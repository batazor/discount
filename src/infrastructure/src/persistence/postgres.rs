//! Postgres implementation of `PromotionCatalog` and `RedemptionRepository` (sqlx).
//! `reserve` is one transaction: `FOR UPDATE` on every budget row, snapshot, pricing,
//! insert, decrement (design decisions 2 and 3 of the redeem-and-persistence change).

use std::collections::BTreeMap;

use async_trait::async_trait;
use discount_application::ports::{
    CatalogSnapshot, PricingFn, PromotionCatalog, Redemption, RedemptionError,
    RedemptionRepository, RedemptionStatus, ReserveOutcome, StorageError, Uses, transition_allowed,
};
use discount_domain::{
    Coupon, ExchangeRate, Money, PricingInput, PricingPolicy, PricingResult, Promotion,
    PromotionId, StackingGroup, Timestamp,
};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::types::Json;
use sqlx::{Postgres, Row, Transaction};

use super::memory::budget_moves;

#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

fn storage(e: sqlx::Error) -> StorageError {
    StorageError::Unavailable(e.to_string())
}

fn redemption_storage(e: sqlx::Error) -> RedemptionError {
    RedemptionError::Storage(storage(e))
}

fn epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn parse_status(s: &str) -> RedemptionStatus {
    match s {
        "confirmed" => RedemptionStatus::Confirmed,
        "cancelled" => RedemptionStatus::Cancelled,
        _ => RedemptionStatus::Reserved,
    }
}

impl PgStore {
    /// Connect, run the migrations, seed the policy row if missing.
    pub async fn connect(
        url: &str,
        default_policy: &PricingPolicy,
    ) -> Result<PgStore, StorageError> {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(url)
            .await
            .map_err(storage)?;
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| StorageError::Unavailable(e.to_string()))?;
        sqlx::query("INSERT INTO pricing_policy (id, body, updated_at) VALUES (1, $1, $2) ON CONFLICT (id) DO NOTHING")
            .bind(Json(default_policy))
            .bind(epoch())
            .execute(&pool)
            .await
            .map_err(storage)?;
        Ok(PgStore { pool })
    }

    /// Empty every table and reset the policy; for integration tests only. Tests wipe the
    /// database they point at, so give them a dedicated one.
    pub async fn reset_for_tests(&self, policy: &PricingPolicy) -> Result<(), StorageError> {
        sqlx::query("TRUNCATE redemptions, promotion_budgets, promotions, stacking_groups, coupons, exchange_rates")
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        self.put_policy(policy.clone()).await
    }

    async fn snapshot_in(
        tx: &mut Transaction<'_, Postgres>,
        lock_budgets: bool,
    ) -> Result<(CatalogSnapshot, BTreeMap<String, Money>), sqlx::Error> {
        let policy: Json<PricingPolicy> =
            sqlx::query("SELECT body FROM pricing_policy WHERE id = 1")
                .fetch_one(&mut **tx)
                .await?
                .try_get("body")?;
        let groups = sqlx::query("SELECT body FROM stacking_groups ORDER BY group_id")
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .map(|r| r.try_get::<Json<StackingGroup>, _>("body").map(|j| j.0))
            .collect::<Result<Vec<_>, _>>()?;
        let budget_sql = if lock_budgets {
            "SELECT promotion_id, remaining, currency FROM promotion_budgets ORDER BY promotion_id FOR UPDATE"
        } else {
            "SELECT promotion_id, remaining, currency FROM promotion_budgets ORDER BY promotion_id"
        };
        let mut budgets = BTreeMap::new();
        for r in sqlx::query(budget_sql).fetch_all(&mut **tx).await? {
            let pid: String = r.try_get("promotion_id")?;
            let remaining: i64 = r.try_get("remaining")?;
            let currency: String = r.try_get("currency")?;
            budgets.insert(
                pid,
                Money {
                    amount: remaining,
                    currency,
                },
            );
        }
        let promotions =
            sqlx::query("SELECT body FROM promotions WHERE NOT archived ORDER BY promotion_id")
                .fetch_all(&mut **tx)
                .await?
                .into_iter()
                .map(|r| {
                    r.try_get::<Json<Promotion>, _>("body").map(|j| {
                        let mut p = j.0;
                        p.remaining_budget = budgets.get(&p.promotion_id).cloned();
                        p
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
        let coupons = sqlx::query("SELECT body FROM coupons ORDER BY code")
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .map(|r| r.try_get::<Json<Coupon>, _>("body").map(|j| j.0))
            .collect::<Result<Vec<_>, _>>()?;
        let exchange_rates = sqlx::query("SELECT from_currency, to_currency, numerator, denominator FROM exchange_rates ORDER BY from_currency, to_currency")
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .map(|r| {
                Ok(ExchangeRate {
                    from: r.try_get("from_currency")?,
                    to: r.try_get("to_currency")?,
                    numerator: r.try_get("numerator")?,
                    denominator: r.try_get("denominator")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;
        Ok((
            CatalogSnapshot {
                policy: policy.0,
                groups,
                promotions,
                coupons,
                exchange_rates,
            },
            budgets,
        ))
    }

    async fn uses_in(
        tx: &mut Transaction<'_, Postgres>,
        customer_id: &str,
    ) -> Result<Uses, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT a->>'promotion_id' AS pid, COUNT(*)::BIGINT AS n FROM redemptions r, jsonb_array_elements(r.result->'applied') a \
             WHERE r.customer_id = $1 AND r.status <> 'cancelled' GROUP BY 1 ORDER BY 1",
        )
        .bind(customer_id)
        .fetch_all(&mut **tx)
        .await?;
        rows.into_iter()
            .map(|r| Ok((r.try_get::<String, _>("pid")?, r.try_get::<i64, _>("n")?)))
            .collect()
    }

    fn row_to_redemption(r: &sqlx::postgres::PgRow) -> Result<Redemption, sqlx::Error> {
        let status: String = r.try_get("status")?;
        let input: Json<PricingInput> = r.try_get("input")?;
        let result: Json<PricingResult> = r.try_get("result")?;
        Ok(Redemption {
            order_id: r.try_get("order_id")?,
            customer_id: r.try_get("customer_id")?,
            status: parse_status(&status),
            input: input.0,
            result: result.0,
            created_at: r.try_get("created_at")?,
            updated_at: r.try_get("updated_at")?,
        })
    }
}

#[async_trait]
impl PromotionCatalog for PgStore {
    async fn snapshot(&self) -> Result<CatalogSnapshot, StorageError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (snapshot, _) = PgStore::snapshot_in(&mut tx, false)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(snapshot)
    }

    async fn get_policy(&self) -> Result<PricingPolicy, StorageError> {
        let j: Json<PricingPolicy> = sqlx::query("SELECT body FROM pricing_policy WHERE id = 1")
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?
            .try_get("body")
            .map_err(storage)?;
        Ok(j.0)
    }

    async fn put_policy(&self, policy: PricingPolicy) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO pricing_policy (id, body, updated_at) VALUES (1, $1, $2) ON CONFLICT (id) DO UPDATE SET body = $1, updated_at = $2")
            .bind(Json(policy))
            .bind(epoch())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn list_groups(&self) -> Result<Vec<StackingGroup>, StorageError> {
        sqlx::query("SELECT body FROM stacking_groups ORDER BY group_id")
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
            .into_iter()
            .map(|r| {
                r.try_get::<Json<StackingGroup>, _>("body")
                    .map(|j| j.0)
                    .map_err(storage)
            })
            .collect()
    }

    async fn get_group(&self, group_id: &str) -> Result<Option<StackingGroup>, StorageError> {
        let row = sqlx::query("SELECT body FROM stacking_groups WHERE group_id = $1")
            .bind(group_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        row.map(|r| {
            r.try_get::<Json<StackingGroup>, _>("body")
                .map(|j| j.0)
                .map_err(storage)
        })
        .transpose()
    }

    async fn put_group(&self, group: StackingGroup) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO stacking_groups (group_id, body, updated_at) VALUES ($1, $2, $3) ON CONFLICT (group_id) DO UPDATE SET body = $2, updated_at = $3")
            .bind(&group.group_id)
            .bind(Json(&group))
            .bind(epoch())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_group(&self, group_id: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM stacking_groups WHERE group_id = $1")
            .bind(group_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    async fn list_promotions(&self) -> Result<Vec<Promotion>, StorageError> {
        Ok(self.snapshot().await?.promotions)
    }

    async fn get_promotion(&self, promotion_id: &str) -> Result<Option<Promotion>, StorageError> {
        let row = sqlx::query(
            "SELECT p.body, b.remaining, b.currency FROM promotions p LEFT JOIN promotion_budgets b USING (promotion_id) WHERE p.promotion_id = $1 AND NOT p.archived",
        )
        .bind(promotion_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        match row {
            None => Ok(None),
            Some(r) => {
                let mut p = r.try_get::<Json<Promotion>, _>("body").map_err(storage)?.0;
                let remaining: Option<i64> = r.try_get("remaining").map_err(storage)?;
                let currency: Option<String> = r.try_get("currency").map_err(storage)?;
                p.remaining_budget = match (remaining, currency) {
                    (Some(a), Some(c)) => Some(Money {
                        amount: a,
                        currency: c,
                    }),
                    _ => None,
                };
                Ok(Some(p))
            }
        }
    }

    async fn put_promotion(&self, mut promotion: Promotion) -> Result<(), StorageError> {
        promotion.remaining_budget = None;
        let now = epoch();
        sqlx::query(
            "INSERT INTO promotions (promotion_id, body, archived, created_at, updated_at) VALUES ($1, $2, FALSE, $3, $3) \
             ON CONFLICT (promotion_id) DO UPDATE SET body = $2, archived = FALSE, updated_at = $3",
        )
        .bind(&promotion.promotion_id)
        .bind(Json(&promotion))
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(())
    }

    async fn archive_promotion(&self, promotion_id: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("UPDATE promotions SET archived = TRUE, updated_at = $2 WHERE promotion_id = $1 AND NOT archived")
            .bind(promotion_id)
            .bind(epoch())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    async fn get_budget(&self, promotion_id: &str) -> Result<Option<Option<Money>>, StorageError> {
        let row = sqlx::query("SELECT b.remaining, b.currency FROM promotions p LEFT JOIN promotion_budgets b USING (promotion_id) WHERE p.promotion_id = $1")
            .bind(promotion_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        match row {
            None => Ok(None),
            Some(r) => {
                let remaining: Option<i64> = r.try_get("remaining").map_err(storage)?;
                let currency: Option<String> = r.try_get("currency").map_err(storage)?;
                Ok(Some(match (remaining, currency) {
                    (Some(a), Some(c)) => Some(Money {
                        amount: a,
                        currency: c,
                    }),
                    _ => None,
                }))
            }
        }
    }

    async fn put_budget(
        &self,
        promotion_id: &str,
        remaining: Option<Money>,
    ) -> Result<bool, StorageError> {
        let exists: Option<sqlx::postgres::PgRow> =
            sqlx::query("SELECT 1 FROM promotions WHERE promotion_id = $1")
                .bind(promotion_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        if exists.is_none() {
            return Ok(false);
        }
        match remaining {
            Some(m) => {
                sqlx::query("INSERT INTO promotion_budgets (promotion_id, remaining, currency, updated_at) VALUES ($1, $2, $3, $4) ON CONFLICT (promotion_id) DO UPDATE SET remaining = $2, currency = $3, updated_at = $4")
                    .bind(promotion_id)
                    .bind(m.amount)
                    .bind(&m.currency)
                    .bind(epoch())
                    .execute(&self.pool)
                    .await
                    .map_err(storage)?;
            }
            None => {
                sqlx::query("DELETE FROM promotion_budgets WHERE promotion_id = $1")
                    .bind(promotion_id)
                    .execute(&self.pool)
                    .await
                    .map_err(storage)?;
            }
        }
        Ok(true)
    }

    async fn list_budgets(&self) -> Result<Vec<(PromotionId, Money)>, StorageError> {
        sqlx::query(
            "SELECT promotion_id, remaining, currency FROM promotion_budgets ORDER BY promotion_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?
        .into_iter()
        .map(|r| {
            Ok((
                r.try_get::<String, _>("promotion_id").map_err(storage)?,
                Money {
                    amount: r.try_get("remaining").map_err(storage)?,
                    currency: r.try_get("currency").map_err(storage)?,
                },
            ))
        })
        .collect()
    }

    async fn list_coupons(&self) -> Result<Vec<Coupon>, StorageError> {
        Ok(self.snapshot().await?.coupons)
    }

    async fn get_coupon(&self, code: &str) -> Result<Option<Coupon>, StorageError> {
        let row = sqlx::query("SELECT body FROM coupons WHERE code = $1")
            .bind(code)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        row.map(|r| {
            r.try_get::<Json<Coupon>, _>("body")
                .map(|j| j.0)
                .map_err(storage)
        })
        .transpose()
    }

    async fn put_coupon(&self, coupon: Coupon) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO coupons (code, body, updated_at) VALUES ($1, $2, $3) ON CONFLICT (code) DO UPDATE SET body = $2, updated_at = $3")
            .bind(&coupon.code)
            .bind(Json(&coupon))
            .bind(epoch())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_coupon(&self, code: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM coupons WHERE code = $1")
            .bind(code)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    async fn list_rates(&self) -> Result<Vec<ExchangeRate>, StorageError> {
        Ok(self.snapshot().await?.exchange_rates)
    }

    async fn put_rate(&self, rate: ExchangeRate) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO exchange_rates (from_currency, to_currency, numerator, denominator, updated_at) VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (from_currency, to_currency) DO UPDATE SET numerator = $3, denominator = $4, updated_at = $5",
        )
        .bind(&rate.from)
        .bind(&rate.to)
        .bind(rate.numerator)
        .bind(rate.denominator)
        .bind(epoch())
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(())
    }

    async fn delete_rate(&self, from: &str, to: &str) -> Result<bool, StorageError> {
        let r =
            sqlx::query("DELETE FROM exchange_rates WHERE from_currency = $1 AND to_currency = $2")
                .bind(from)
                .bind(to)
                .execute(&self.pool)
                .await
                .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }
}

#[async_trait]
impl RedemptionRepository for PgStore {
    async fn reserve(
        &self,
        order_id: &str,
        customer_id: Option<&str>,
        now: Timestamp,
        price: &PricingFn<'_>,
    ) -> Result<ReserveOutcome, RedemptionError> {
        let mut tx = self.pool.begin().await.map_err(redemption_storage)?;
        let existing = sqlx::query("SELECT order_id, customer_id, status, input, result, created_at, updated_at FROM redemptions WHERE order_id = $1 FOR UPDATE")
            .bind(order_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(redemption_storage)?;
        if let Some(r) = existing {
            let r = PgStore::row_to_redemption(&r).map_err(redemption_storage)?;
            if r.status != RedemptionStatus::Cancelled {
                tx.rollback().await.map_err(redemption_storage)?;
                return Ok(ReserveOutcome::Idempotent(r));
            }
        }
        let (snapshot, budgets) = PgStore::snapshot_in(&mut tx, true)
            .await
            .map_err(redemption_storage)?;
        let uses = match customer_id {
            Some(c) => PgStore::uses_in(&mut tx, c)
                .await
                .map_err(redemption_storage)?,
            None => Vec::new(),
        };
        let (input, result) = price(snapshot, uses).map_err(RedemptionError::Pricing)?;
        for (pid, amount) in budget_moves(&result, &budgets) {
            sqlx::query("UPDATE promotion_budgets SET remaining = remaining - $2, updated_at = $3 WHERE promotion_id = $1")
                .bind(&pid)
                .bind(amount)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(redemption_storage)?;
        }
        sqlx::query(
            "INSERT INTO redemptions (order_id, customer_id, status, input, result, created_at, updated_at) VALUES ($1, $5, 'reserved', $2, $3, $4, $4) \
             ON CONFLICT (order_id) DO UPDATE SET customer_id = $5, status = 'reserved', input = $2, result = $3, created_at = $4, updated_at = $4",
        )
        .bind(order_id)
        .bind(Json(&input))
        .bind(Json(&result))
        .bind(now)
        .bind(customer_id)
        .execute(&mut *tx)
        .await
        .map_err(redemption_storage)?;
        tx.commit().await.map_err(redemption_storage)?;
        Ok(ReserveOutcome::Reserved(Redemption {
            order_id: order_id.to_string(),
            customer_id: customer_id.map(str::to_string),
            status: RedemptionStatus::Reserved,
            input,
            result,
            created_at: now,
            updated_at: now,
        }))
    }

    async fn transition(
        &self,
        order_id: &str,
        to: RedemptionStatus,
        now: Timestamp,
    ) -> Result<Redemption, RedemptionError> {
        let mut tx = self.pool.begin().await.map_err(redemption_storage)?;
        let row = sqlx::query("SELECT order_id, customer_id, status, input, result, created_at, updated_at FROM redemptions WHERE order_id = $1 FOR UPDATE")
            .bind(order_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(redemption_storage)?
            .ok_or(RedemptionError::NotFound)?;
        let mut current = PgStore::row_to_redemption(&row).map_err(redemption_storage)?;
        if !transition_allowed(current.status, to) {
            return Err(RedemptionError::InvalidTransition {
                from: current.status,
                to,
            });
        }
        if to == RedemptionStatus::Cancelled {
            // Restore what the result granted to promotions that still have a budget row.
            for a in &current.result.applied {
                sqlx::query("UPDATE promotion_budgets SET remaining = remaining + $2, updated_at = $3 WHERE promotion_id = $1")
                    .bind(&a.promotion_id)
                    .bind(a.discount.amount)
                    .bind(now)
                    .execute(&mut *tx)
                    .await
                    .map_err(redemption_storage)?;
            }
        }
        sqlx::query("UPDATE redemptions SET status = $2, updated_at = $3 WHERE order_id = $1")
            .bind(order_id)
            .bind(to.as_str())
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(redemption_storage)?;
        tx.commit().await.map_err(redemption_storage)?;
        current.status = to;
        current.updated_at = now;
        Ok(current)
    }

    async fn uses_for_customer(&self, customer_id: &str) -> Result<Uses, RedemptionError> {
        let mut tx = self.pool.begin().await.map_err(redemption_storage)?;
        let uses = PgStore::uses_in(&mut tx, customer_id)
            .await
            .map_err(redemption_storage)?;
        tx.commit().await.map_err(redemption_storage)?;
        Ok(uses)
    }

    async fn get(&self, order_id: &str) -> Result<Option<Redemption>, RedemptionError> {
        let row = sqlx::query("SELECT order_id, customer_id, status, input, result, created_at, updated_at FROM redemptions WHERE order_id = $1")
            .bind(order_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(redemption_storage)?;
        row.map(|r| PgStore::row_to_redemption(&r).map_err(redemption_storage))
            .transpose()
    }
}
