//! Postgres implementation of `PromotionCatalog` and `RedemptionRepository` (sqlx).
//! `reserve` is one transaction: `FOR UPDATE` on every budget row, snapshot, pricing,
//! insert, decrement (design decisions 2 and 3 of the redeem-and-persistence change).

use std::collections::BTreeMap;

use async_trait::async_trait;
use discount_application::ports::{
    CatalogSnapshot, Counters, GrantRule, GrantTrigger, PricingFn, PromotionCatalog,
    PromotionMetadata, Redemption, RedemptionError, RedemptionRepository, RedemptionStatus,
    ReserveOutcome, StorageError, Uses, transition_allowed,
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
        sqlx::query("TRUNCATE redemptions, promotion_budgets, promotions, stacking_groups, coupons, exchange_rates, grant_rules, grants, saved_coupons, promotion_metadata")
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        self.put_policy(policy.clone()).await
    }

    /// With `lock`, the budget and promotion rows are taken `FOR UPDATE`, so concurrent
    /// reservations serialize and each one counts the uses the previous one committed.
    async fn snapshot_in(
        tx: &mut Transaction<'_, Postgres>,
        lock: bool,
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
        let budget_sql = if lock {
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
        let promotion_sql = if lock {
            "SELECT body FROM promotions WHERE NOT archived ORDER BY promotion_id FOR UPDATE"
        } else {
            "SELECT body FROM promotions WHERE NOT archived ORDER BY promotion_id"
        };
        let promotions = sqlx::query(promotion_sql)
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
        let mut snapshot = CatalogSnapshot {
            policy: policy.0,
            groups,
            promotions,
            coupons,
            exchange_rates,
        };
        PgStore::counters_in(tx, None)
            .await?
            .apply_to(&mut snapshot);
        Ok((snapshot, budgets))
    }

    /// Spec `redemption`, "Counting uses per coupon and per promotion", over reserved and
    /// confirmed redemptions. `codes = None` counts every stored coupon.
    async fn counters_in(
        tx: &mut Transaction<'_, Postgres>,
        codes: Option<&[String]>,
    ) -> Result<Counters, sqlx::Error> {
        let promotion_uses = sqlx::query(
            "SELECT a->>'promotion_id' AS pid, COUNT(*)::BIGINT AS n FROM redemptions r, jsonb_array_elements(r.result->'applied') a \
             WHERE r.status <> 'cancelled' GROUP BY 1 ORDER BY 1",
        )
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|r| Ok((r.try_get::<String, _>("pid")?, r.try_get::<i64, _>("n")?)))
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
        let all: Vec<String> = Vec::new();
        let coupon_uses = sqlx::query(
            "SELECT c.code AS code, COUNT(*)::BIGINT AS n FROM redemptions r \
             JOIN LATERAL jsonb_array_elements_text(r.input->'cart'->'coupon_codes') AS presented(code) ON TRUE \
             JOIN coupons c ON c.code = presented.code \
             WHERE r.status <> 'cancelled' AND ($1 OR c.code = ANY($2)) \
               AND EXISTS (SELECT 1 FROM jsonb_array_elements(r.result->'applied') a WHERE a->>'promotion_id' = c.body->>'promotion_id') \
             GROUP BY 1 ORDER BY 1",
        )
        .bind(codes.is_none())
        .bind(codes.unwrap_or(&all))
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|r| Ok((r.try_get::<String, _>("code")?, r.try_get::<i64, _>("n")?)))
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
        Ok(Counters {
            promotion_uses,
            coupon_uses,
        })
    }

    async fn coupon_in(
        tx: &mut Transaction<'_, Postgres>,
        code: &str,
    ) -> Result<Option<Coupon>, sqlx::Error> {
        let row = sqlx::query("SELECT body FROM coupons WHERE code = $1")
            .bind(code)
            .fetch_optional(&mut **tx)
            .await?;
        let Some(row) = row else { return Ok(None) };
        let mut c = row.try_get::<Json<Coupon>, _>("body")?.0;
        c.uses = PgStore::counters_in(tx, Some(&[code.to_string()]))
            .await?
            .coupon(code);
        Ok(Some(c))
    }

    fn row_to_grant_rule(r: &sqlx::postgres::PgRow) -> Result<GrantRule, sqlx::Error> {
        r.try_get::<Json<GrantRule>, _>("body").map(|j| j.0)
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
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let c = PgStore::coupon_in(&mut tx, code).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(c)
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

    async fn list_grant_rules(&self) -> Result<Vec<GrantRule>, StorageError> {
        sqlx::query("SELECT body FROM grant_rules ORDER BY rule_id")
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
            .iter()
            .map(|r| PgStore::row_to_grant_rule(r).map_err(storage))
            .collect()
    }

    async fn get_grant_rule(&self, rule_id: &str) -> Result<Option<GrantRule>, StorageError> {
        sqlx::query("SELECT body FROM grant_rules WHERE rule_id = $1")
            .bind(rule_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| PgStore::row_to_grant_rule(&r).map_err(storage))
            .transpose()
    }

    async fn put_grant_rule(&self, rule: GrantRule) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO grant_rules (rule_id, body, updated_at) VALUES ($1, $2, $3) ON CONFLICT (rule_id) DO UPDATE SET body = $2, updated_at = $3")
            .bind(&rule.rule_id)
            .bind(Json(&rule))
            .bind(epoch())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn delete_grant_rule(&self, rule_id: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM grant_rules WHERE rule_id = $1")
            .bind(rule_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    async fn put_coupons(&self, coupons: Vec<Coupon>) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let now = epoch();
        for c in &coupons {
            // A plain INSERT: an existing code aborts the whole batch.
            sqlx::query("INSERT INTO coupons (code, body, updated_at) VALUES ($1, $2, $3)")
                .bind(&c.code)
                .bind(Json(c))
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(())
    }

    async fn get_grant(
        &self,
        trigger: GrantTrigger,
        order_id: &str,
    ) -> Result<Option<Coupon>, StorageError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let code: Option<String> =
            sqlx::query("SELECT code FROM grants WHERE trigger = $1 AND order_id = $2")
                .bind(trigger.as_str())
                .bind(order_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
                .map(|r| r.try_get("code"))
                .transpose()
                .map_err(storage)?;
        let c = match code {
            Some(code) => PgStore::coupon_in(&mut tx, &code).await.map_err(storage)?,
            None => None,
        };
        tx.commit().await.map_err(storage)?;
        Ok(c)
    }

    async fn record_grant(
        &self,
        trigger: GrantTrigger,
        order_id: &str,
        coupon: Coupon,
        now: Timestamp,
    ) -> Result<Coupon, StorageError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let inserted = sqlx::query("INSERT INTO grants (trigger, order_id, code, created_at) VALUES ($1, $2, $3, $4) ON CONFLICT (trigger, order_id) DO NOTHING")
            .bind(trigger.as_str())
            .bind(order_id)
            .bind(&coupon.code)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(storage)?
            .rows_affected();
        if inserted == 0 {
            // Another grant for the same key won: hand back what it issued.
            let code: String =
                sqlx::query("SELECT code FROM grants WHERE trigger = $1 AND order_id = $2")
                    .bind(trigger.as_str())
                    .bind(order_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(storage)?
                    .try_get("code")
                    .map_err(storage)?;
            let existing = PgStore::coupon_in(&mut tx, &code).await.map_err(storage)?;
            tx.commit().await.map_err(storage)?;
            return Ok(existing.unwrap_or(coupon));
        }
        sqlx::query("INSERT INTO coupons (code, body, updated_at) VALUES ($1, $2, $3)")
            .bind(&coupon.code)
            .bind(Json(&coupon))
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(coupon)
    }

    async fn coupons_bound_to(&self, customer_id: &str) -> Result<Vec<Coupon>, StorageError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let mut coupons =
            sqlx::query("SELECT body FROM coupons WHERE body ->> 'customer_id' = $1 ORDER BY code")
                .bind(customer_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(storage)?
                .into_iter()
                .map(|r| r.try_get::<Json<Coupon>, _>("body").map(|j| j.0))
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage)?;
        let codes: Vec<String> = coupons.iter().map(|c| c.code.clone()).collect();
        let counters = PgStore::counters_in(&mut tx, Some(&codes))
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        for c in &mut coupons {
            c.uses = counters.coupon(&c.code);
        }
        Ok(coupons)
    }

    async fn saved_codes(&self, customer_id: &str) -> Result<Vec<String>, StorageError> {
        sqlx::query("SELECT code FROM saved_coupons WHERE customer_id = $1 ORDER BY code")
            .bind(customer_id)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
            .iter()
            .map(|r| r.try_get::<String, _>("code").map_err(storage))
            .collect()
    }

    async fn save_coupon(&self, customer_id: &str, code: &str) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO saved_coupons (customer_id, code, created_at) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind(customer_id)
            .bind(code)
            .bind(epoch())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    async fn forget_coupon(&self, customer_id: &str, code: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM saved_coupons WHERE customer_id = $1 AND code = $2")
            .bind(customer_id)
            .bind(code)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    async fn get_metadata(
        &self,
        promotion_id: &str,
    ) -> Result<Option<PromotionMetadata>, StorageError> {
        sqlx::query("SELECT m.body FROM promotion_metadata m JOIN promotions p USING (promotion_id) WHERE m.promotion_id = $1 AND NOT p.archived")
            .bind(promotion_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| r.try_get::<Json<PromotionMetadata>, _>("body").map(|j| j.0).map_err(storage))
            .transpose()
    }

    async fn put_metadata(
        &self,
        promotion_id: &str,
        metadata: PromotionMetadata,
    ) -> Result<bool, StorageError> {
        let r = sqlx::query(
            "INSERT INTO promotion_metadata (promotion_id, body, updated_at) \
             SELECT promotion_id, $2, $3 FROM promotions WHERE promotion_id = $1 AND NOT archived \
             ON CONFLICT (promotion_id) DO UPDATE SET body = $2, updated_at = $3",
        )
        .bind(promotion_id)
        .bind(Json(&metadata))
        .bind(epoch())
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    async fn delete_metadata(&self, promotion_id: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM promotion_metadata WHERE promotion_id = $1")
            .bind(promotion_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    async fn list_metadata(&self) -> Result<Vec<(PromotionId, PromotionMetadata)>, StorageError> {
        sqlx::query("SELECT m.promotion_id, m.body FROM promotion_metadata m JOIN promotions p USING (promotion_id) WHERE NOT p.archived ORDER BY m.promotion_id")
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
            .iter()
            .map(|r| {
                Ok((
                    r.try_get::<String, _>("promotion_id").map_err(storage)?,
                    r.try_get::<Json<PromotionMetadata>, _>("body").map(|j| j.0).map_err(storage)?,
                ))
            })
            .collect()
    }
}

#[async_trait]
impl RedemptionRepository for PgStore {
    async fn reserve(
        &self,
        order_id: &str,
        customer_id: Option<&str>,
        _codes: &[String],
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

    async fn counters(&self, codes: &[String]) -> Result<Counters, RedemptionError> {
        let mut tx = self.pool.begin().await.map_err(redemption_storage)?;
        let c = PgStore::counters_in(&mut tx, Some(codes))
            .await
            .map_err(redemption_storage)?;
        tx.commit().await.map_err(redemption_storage)?;
        Ok(c)
    }

    async fn coupon_uses_of_customer(
        &self,
        customer_id: &str,
    ) -> Result<Vec<(String, String)>, RedemptionError> {
        sqlx::query(
            "SELECT presented.code AS code, r.order_id AS order_id FROM redemptions r \
             JOIN LATERAL jsonb_array_elements_text(r.input->'cart'->'coupon_codes') AS presented(code) ON TRUE \
             JOIN coupons c ON c.code = presented.code \
             WHERE r.customer_id = $1 AND r.status <> 'cancelled' \
               AND EXISTS (SELECT 1 FROM jsonb_array_elements(r.result->'applied') a WHERE a->>'promotion_id' = c.body->>'promotion_id') \
             ORDER BY r.created_at, r.order_id, presented.code",
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await
        .map_err(redemption_storage)?
        .iter()
        .map(|r| Ok((r.try_get::<String, _>("code")?, r.try_get::<String, _>("order_id")?)))
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(redemption_storage)
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
