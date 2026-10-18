//! Postgres implementation of `PromotionCatalog` and `RedemptionRepository` (sqlx).
//! `reserve` is one transaction: `FOR UPDATE` on every budget row, snapshot, pricing,
//! insert, decrement (design decisions 2 and 3 of the redeem-and-persistence change).

use std::collections::BTreeMap;

use async_trait::async_trait;
use discount_application::ports::{
    CatalogSnapshot, Counters, ExchangeRateSource, ExchangeRateSourceRecord, FetchOutcome,
    GrantRule, GrantTrigger, PricingFn, PromotionCatalog, PromotionMetadata, Redemption,
    RedemptionError, RedemptionRepository, RedemptionStatus, ReserveOutcome, SourcePair,
    StorageError, Uses, transition_allowed,
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

/// Every sqlx failure passes through here: mark the enclosing `db.<method>` span as failed
/// (spec `telemetry`, "Database spans") and log it. Domain outcomes such as `NotFound` or
/// `InvalidTransition` do not come this way and leave the span status unset.
fn storage(e: sqlx::Error) -> StorageError {
    tracing::Span::current().record("otel.status_code", "ERROR");
    tracing::error!(error = %e, "postgres failure");
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
        sqlx::query("TRUNCATE redemptions, promotion_budgets, promotions, stacking_groups, coupons, exchange_rates, grant_rules, grants, saved_coupons, promotion_metadata, promotion_allowlist, exchange_rate_source")
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
    #[tracing::instrument(name = "db.snapshot", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "snapshot"))]
    async fn snapshot(&self) -> Result<CatalogSnapshot, StorageError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (snapshot, _) = PgStore::snapshot_in(&mut tx, false)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(snapshot)
    }

    #[tracing::instrument(name = "db.get_policy", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get_policy"))]
    async fn get_policy(&self) -> Result<PricingPolicy, StorageError> {
        let j: Json<PricingPolicy> = sqlx::query("SELECT body FROM pricing_policy WHERE id = 1")
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?
            .try_get("body")
            .map_err(storage)?;
        Ok(j.0)
    }

    #[tracing::instrument(name = "db.put_policy", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_policy"))]
    async fn put_policy(&self, policy: PricingPolicy) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO pricing_policy (id, body, updated_at) VALUES (1, $1, $2) ON CONFLICT (id) DO UPDATE SET body = $1, updated_at = $2")
            .bind(Json(policy))
            .bind(epoch())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    #[tracing::instrument(name = "db.list_groups", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "list_groups"))]
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

    #[tracing::instrument(name = "db.get_group", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get_group"))]
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

    #[tracing::instrument(name = "db.put_group", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_group"))]
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

    #[tracing::instrument(name = "db.delete_group", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "delete_group"))]
    async fn delete_group(&self, group_id: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM stacking_groups WHERE group_id = $1")
            .bind(group_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    #[tracing::instrument(name = "db.list_promotions", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "list_promotions"))]
    async fn list_promotions(&self) -> Result<Vec<Promotion>, StorageError> {
        Ok(self.snapshot().await?.promotions)
    }

    #[tracing::instrument(name = "db.get_promotion", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get_promotion"))]
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

    #[tracing::instrument(name = "db.put_promotion", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_promotion"))]
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

    #[tracing::instrument(name = "db.archive_promotion", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "archive_promotion"))]
    async fn archive_promotion(&self, promotion_id: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("UPDATE promotions SET archived = TRUE, updated_at = $2 WHERE promotion_id = $1 AND NOT archived")
            .bind(promotion_id)
            .bind(epoch())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    #[tracing::instrument(name = "db.get_budget", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get_budget"))]
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

    #[tracing::instrument(name = "db.put_budget", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_budget"))]
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

    #[tracing::instrument(name = "db.list_budgets", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "list_budgets"))]
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

    #[tracing::instrument(name = "db.list_coupons", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "list_coupons"))]
    async fn list_coupons(&self) -> Result<Vec<Coupon>, StorageError> {
        Ok(self.snapshot().await?.coupons)
    }

    #[tracing::instrument(name = "db.get_coupon", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get_coupon"))]
    async fn get_coupon(&self, code: &str) -> Result<Option<Coupon>, StorageError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let c = PgStore::coupon_in(&mut tx, code).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(c)
    }

    #[tracing::instrument(name = "db.put_coupon", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_coupon"))]
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

    #[tracing::instrument(name = "db.delete_coupon", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "delete_coupon"))]
    async fn delete_coupon(&self, code: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM coupons WHERE code = $1")
            .bind(code)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    #[tracing::instrument(name = "db.list_rates", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "list_rates"))]
    async fn list_rates(&self) -> Result<Vec<ExchangeRate>, StorageError> {
        Ok(self.snapshot().await?.exchange_rates)
    }

    #[tracing::instrument(name = "db.put_rate", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_rate"))]
    async fn put_rate(&self, rate: ExchangeRate) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO exchange_rates (from_currency, to_currency, numerator, denominator, updated_at, origin, fetched_at) VALUES ($1, $2, $3, $4, $5, 'manual', NULL) \
             ON CONFLICT (from_currency, to_currency) DO UPDATE SET numerator = $3, denominator = $4, updated_at = $5, origin = 'manual', fetched_at = NULL",
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

    #[tracing::instrument(name = "db.delete_rate", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "delete_rate"))]
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

    #[tracing::instrument(name = "db.get_rate_source", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get_rate_source"))]
    async fn get_rate_source(&self) -> Result<Option<ExchangeRateSourceRecord>, StorageError> {
        let row = sqlx::query("SELECT body, updated_at, last_attempt_at, last_success_at, last_error, last_count FROM exchange_rate_source WHERE id = 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        row.map(|r| {
            let body: Json<ExchangeRateSource> = r.try_get("body").map_err(storage)?;
            Ok(ExchangeRateSourceRecord {
                source: body.0,
                updated_at: r.try_get("updated_at").map_err(storage)?,
                last_attempt_at: r.try_get("last_attempt_at").map_err(storage)?,
                last_success_at: r.try_get("last_success_at").map_err(storage)?,
                last_error: r.try_get("last_error").map_err(storage)?,
                last_count: r.try_get("last_count").map_err(storage)?,
            })
        })
        .transpose()
    }

    #[tracing::instrument(name = "db.put_rate_source", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_rate_source"))]
    async fn put_rate_source(
        &self,
        source: ExchangeRateSource,
        now: Timestamp,
    ) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO exchange_rate_source (id, body, updated_at) VALUES (1, $1, $2) ON CONFLICT (id) DO UPDATE SET body = $1, updated_at = $2")
            .bind(Json(source))
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }

    #[tracing::instrument(name = "db.delete_rate_source", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "delete_rate_source"))]
    async fn delete_rate_source(&self) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query("UPDATE exchange_rates SET origin = 'manual', fetched_at = NULL WHERE origin = 'source'")
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let r = sqlx::query("DELETE FROM exchange_rate_source WHERE id = 1")
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    #[tracing::instrument(name = "db.record_rate_fetch", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "record_rate_fetch"))]
    async fn record_rate_fetch(
        &self,
        now: Timestamp,
        outcome: FetchOutcome,
    ) -> Result<(), StorageError> {
        let q = match outcome {
            Ok(count) => sqlx::query("UPDATE exchange_rate_source SET last_attempt_at = $1, last_success_at = $1, last_count = $2, last_error = NULL WHERE id = 1")
                .bind(now)
                .bind(count),
            Err(message) => sqlx::query("UPDATE exchange_rate_source SET last_attempt_at = $1, last_error = $2 WHERE id = 1")
                .bind(now)
                .bind(message),
        };
        q.execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }

    #[tracing::instrument(name = "db.put_fetched_rates", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_fetched_rates"))]
    async fn put_fetched_rates(
        &self,
        rates: Vec<ExchangeRate>,
        now: Timestamp,
    ) -> Result<i64, StorageError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let mut stored = 0;
        for rate in rates {
            let r = sqlx::query(
                "INSERT INTO exchange_rates (from_currency, to_currency, numerator, denominator, updated_at, origin, fetched_at) VALUES ($1, $2, $3, $4, $5, 'source', $5) \
                 ON CONFLICT (from_currency, to_currency) DO UPDATE SET numerator = $3, denominator = $4, updated_at = $5, fetched_at = $5 WHERE exchange_rates.origin = 'source'",
            )
            .bind(&rate.from)
            .bind(&rate.to)
            .bind(rate.numerator)
            .bind(rate.denominator)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            stored += r.rows_affected() as i64;
        }
        tx.commit().await.map_err(storage)?;
        Ok(stored)
    }

    #[tracing::instrument(name = "db.list_source_pairs", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "list_source_pairs"))]
    async fn list_source_pairs(&self) -> Result<Vec<SourcePair>, StorageError> {
        sqlx::query("SELECT from_currency, to_currency, fetched_at FROM exchange_rates WHERE origin = 'source' ORDER BY from_currency, to_currency")
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
            .iter()
            .map(|r| {
                Ok(SourcePair {
                    from: r.try_get("from_currency").map_err(storage)?,
                    to: r.try_get("to_currency").map_err(storage)?,
                    fetched_at: r.try_get::<Option<i64>, _>("fetched_at").map_err(storage)?.unwrap_or(0),
                })
            })
            .collect()
    }

    #[tracing::instrument(name = "db.list_grant_rules", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "list_grant_rules"))]
    async fn list_grant_rules(&self) -> Result<Vec<GrantRule>, StorageError> {
        sqlx::query("SELECT body FROM grant_rules ORDER BY rule_id")
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
            .iter()
            .map(|r| PgStore::row_to_grant_rule(r).map_err(storage))
            .collect()
    }

    #[tracing::instrument(name = "db.get_grant_rule", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get_grant_rule"))]
    async fn get_grant_rule(&self, rule_id: &str) -> Result<Option<GrantRule>, StorageError> {
        sqlx::query("SELECT body FROM grant_rules WHERE rule_id = $1")
            .bind(rule_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .map(|r| PgStore::row_to_grant_rule(&r).map_err(storage))
            .transpose()
    }

    #[tracing::instrument(name = "db.put_grant_rule", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_grant_rule"))]
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

    #[tracing::instrument(name = "db.delete_grant_rule", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "delete_grant_rule"))]
    async fn delete_grant_rule(&self, rule_id: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM grant_rules WHERE rule_id = $1")
            .bind(rule_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    #[tracing::instrument(name = "db.put_coupons", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_coupons"))]
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

    #[tracing::instrument(name = "db.get_grant", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get_grant"))]
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

    #[tracing::instrument(name = "db.record_grant", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "record_grant"))]
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

    #[tracing::instrument(name = "db.coupons_bound_to", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "coupons_bound_to"))]
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

    #[tracing::instrument(name = "db.saved_codes", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "saved_codes"))]
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

    #[tracing::instrument(name = "db.save_coupon", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "save_coupon"))]
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

    #[tracing::instrument(name = "db.forget_coupon", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "forget_coupon"))]
    async fn forget_coupon(&self, customer_id: &str, code: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM saved_coupons WHERE customer_id = $1 AND code = $2")
            .bind(customer_id)
            .bind(code)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    #[tracing::instrument(name = "db.get_metadata", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get_metadata"))]
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

    #[tracing::instrument(name = "db.put_metadata", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_metadata"))]
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

    #[tracing::instrument(name = "db.delete_metadata", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "delete_metadata"))]
    async fn delete_metadata(&self, promotion_id: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM promotion_metadata WHERE promotion_id = $1")
            .bind(promotion_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    #[tracing::instrument(name = "db.list_metadata", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "list_metadata"))]
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
    #[tracing::instrument(name = "db.get_allowlist", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get_allowlist"))]
    async fn get_allowlist(&self, promotion_id: &str) -> Result<Option<Vec<String>>, StorageError> {
        let live: Option<i64> = sqlx::query_scalar(
            "SELECT 1::BIGINT FROM promotions WHERE promotion_id = $1 AND NOT archived",
        )
        .bind(promotion_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        if live.is_none() {
            return Ok(None);
        }
        let ids: Vec<String> = sqlx::query_scalar("SELECT customer_id FROM promotion_allowlist WHERE promotion_id = $1 ORDER BY customer_id")
            .bind(promotion_id)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(Some(ids))
    }

    #[tracing::instrument(name = "db.put_allowlist", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "put_allowlist"))]
    async fn put_allowlist(
        &self,
        promotion_id: &str,
        customer_ids: Vec<String>,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let live: Option<i64> = sqlx::query_scalar(
            "SELECT 1::BIGINT FROM promotions WHERE promotion_id = $1 AND NOT archived FOR UPDATE",
        )
        .bind(promotion_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        if live.is_none() {
            return Ok(false);
        }
        sqlx::query("DELETE FROM promotion_allowlist WHERE promotion_id = $1")
            .bind(promotion_id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let mut ids = customer_ids;
        ids.sort();
        ids.dedup();
        sqlx::query("INSERT INTO promotion_allowlist (promotion_id, customer_id, created_at) SELECT $1, unnest($2::TEXT[]), $3")
            .bind(promotion_id)
            .bind(&ids)
            .bind(epoch())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(true)
    }

    #[tracing::instrument(name = "db.delete_allowlist", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "delete_allowlist"))]
    async fn delete_allowlist(&self, promotion_id: &str) -> Result<bool, StorageError> {
        let live: Option<i64> = sqlx::query_scalar(
            "SELECT 1::BIGINT FROM promotions WHERE promotion_id = $1 AND NOT archived",
        )
        .bind(promotion_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        if live.is_none() {
            return Ok(false);
        }
        sqlx::query("DELETE FROM promotion_allowlist WHERE promotion_id = $1")
            .bind(promotion_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(true)
    }

    #[tracing::instrument(name = "db.add_to_allowlist", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "add_to_allowlist"))]
    async fn add_to_allowlist(
        &self,
        promotion_id: &str,
        customer_id: &str,
    ) -> Result<bool, StorageError> {
        let r = sqlx::query(
            "INSERT INTO promotion_allowlist (promotion_id, customer_id, created_at) \
             SELECT promotion_id, $2, $3 FROM promotions WHERE promotion_id = $1 AND NOT archived \
             ON CONFLICT (promotion_id, customer_id) DO UPDATE SET created_at = promotion_allowlist.created_at",
        )
        .bind(promotion_id)
        .bind(customer_id)
        .bind(epoch())
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    #[tracing::instrument(name = "db.remove_from_allowlist", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "remove_from_allowlist"))]
    async fn remove_from_allowlist(
        &self,
        promotion_id: &str,
        customer_id: &str,
    ) -> Result<bool, StorageError> {
        let r = sqlx::query(
            "DELETE FROM promotion_allowlist WHERE promotion_id = $1 AND customer_id = $2",
        )
        .bind(promotion_id)
        .bind(customer_id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(r.rows_affected() > 0)
    }

    #[tracing::instrument(name = "db.is_allowlisted", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "is_allowlisted"))]
    async fn is_allowlisted(
        &self,
        promotion_id: &str,
        customer_id: &str,
    ) -> Result<bool, StorageError> {
        let hit: Option<i64> = sqlx::query_scalar(
            "SELECT 1::BIGINT FROM promotion_allowlist a JOIN promotions p USING (promotion_id) \
             WHERE a.promotion_id = $1 AND a.customer_id = $2 AND NOT p.archived",
        )
        .bind(promotion_id)
        .bind(customer_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        Ok(hit.is_some())
    }

    #[tracing::instrument(name = "db.allowlisted_for", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "allowlisted_for"))]
    async fn allowlisted_for(&self, customer_id: &str) -> Result<Vec<PromotionId>, StorageError> {
        sqlx::query_scalar(
            "SELECT a.promotion_id FROM promotion_allowlist a JOIN promotions p USING (promotion_id) \
             WHERE a.customer_id = $1 AND NOT p.archived ORDER BY a.promotion_id",
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)
    }
}

#[async_trait]
impl RedemptionRepository for PgStore {
    #[tracing::instrument(name = "db.reserve", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "reserve"))]
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

    #[tracing::instrument(name = "db.transition", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "transition"))]
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

    #[tracing::instrument(name = "db.uses_for_customer", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "uses_for_customer"))]
    async fn uses_for_customer(&self, customer_id: &str) -> Result<Uses, RedemptionError> {
        let mut tx = self.pool.begin().await.map_err(redemption_storage)?;
        let uses = PgStore::uses_in(&mut tx, customer_id)
            .await
            .map_err(redemption_storage)?;
        tx.commit().await.map_err(redemption_storage)?;
        Ok(uses)
    }

    #[tracing::instrument(name = "db.counters", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "counters"))]
    async fn counters(&self, codes: &[String]) -> Result<Counters, RedemptionError> {
        let mut tx = self.pool.begin().await.map_err(redemption_storage)?;
        let c = PgStore::counters_in(&mut tx, Some(codes))
            .await
            .map_err(redemption_storage)?;
        tx.commit().await.map_err(redemption_storage)?;
        Ok(c)
    }

    #[tracing::instrument(name = "db.coupon_uses_of_customer", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "coupon_uses_of_customer"))]
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

    #[tracing::instrument(name = "db.get", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "get"))]
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
