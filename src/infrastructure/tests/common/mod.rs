//! Shared fixtures for the adapter tests.
#![allow(dead_code)]

use std::sync::Arc;

use discount_domain::*;
use discount_infrastructure::http::{AppState, AuthMode};
use discount_infrastructure::telemetry::Instruments;
use discount_infrastructure::{
    CatalogFixture, FixedClock, InMemoryStore, InMemoryTenants, NoopEventPublisher, PgStore,
    default_policy,
};

pub const NOW: i64 = 1_760_227_200;

pub fn eur(a: i64) -> Money {
    Money::new(a, "EUR")
}

pub fn fixture() -> CatalogFixture {
    CatalogFixture::from_json_file(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../ops/promotions.json"
    ))
    .unwrap()
}

pub fn memory_store() -> InMemoryStore {
    InMemoryStore::new(default_policy()).with_fixture(fixture())
}

pub fn sample_cart() -> CartSnapshot {
    CartSnapshot {
        currency: "EUR".into(),
        lines: vec![
            CartLine {
                line_id: "L1".into(),
                unit_price: eur(1999),
                quantity: 2,
            },
            CartLine {
                line_id: "L2".into(),
                unit_price: eur(501),
                quantity: 1,
            },
        ],
        tags: vec![],
        attributes: vec![],
        coupon_codes: vec![],
    }
}

pub fn state_with(store: InMemoryStore, instruments: Instruments) -> Arc<AppState> {
    Arc::new(AppState {
        tenancy: Arc::new(InMemoryTenants::single(store, default_policy())),
        auth: AuthMode::Off,
        clock: Arc::new(FixedClock(NOW)),
        events: Arc::new(NoopEventPublisher),
        codes: Arc::new(discount_infrastructure::SequentialCodes::default()),
        fetcher: Arc::new(discount_infrastructure::CannedFetcher::default()),
        instruments,
    })
}

pub fn state() -> Arc<AppState> {
    state_with(memory_store(), Instruments::noop())
}

/// A promotion granting a fixed 300 on group `g`.
pub fn fixed_300(id: &str) -> Promotion {
    Promotion {
        promotion_id: id.into(),
        group_id: "g".into(),
        priority: 0,
        exclusive: false,
        cap: None,
        remaining_budget: None,
        uses_total: 0,
        rounding: None,
        rules: vec![],
        action: Action::OrderFixedAmount { amount: eur(300) },
    }
}

pub fn group_g() -> StackingGroup {
    StackingGroup {
        group_id: "g".into(),
        priority: 0,
        mode: StackingMode::Stackable,
    }
}

pub fn customer_json() -> serde_json::Value {
    serde_json::json!({"customer_id": null, "completed_orders": 0, "uses": [], "attributes": [], "allowlisted": []})
}

/// The Postgres the integration tests run against: `DATABASE_URL`, or the `discount_test`
/// database of the local stack (`docker compose -f ops/compose.yaml up -d postgres`).
/// Postgres scenarios never skip (ADR 11): without a reachable server the suite fails.
pub const DEFAULT_TEST_DATABASE_URL: &str =
    "postgres://discount:discount@localhost:5432/discount_test";

pub fn test_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_TEST_DATABASE_URL.to_string())
}

/// Connect to the test database, creating it when it does not exist yet (fresh local
/// stack), and wipe it. Panics with the remedy when no Postgres answers.
pub async fn pg_store() -> PgStore {
    let url = test_database_url();
    let store = match PgStore::connect(&url, &default_policy()).await {
        Ok(s) => s,
        Err(e) if e.to_string().contains("does not exist") => {
            create_database(&url).await;
            PgStore::connect(&url, &default_policy())
                .await
                .unwrap_or_else(|e| panic!("connect to {url} after creating it: {e}"))
        }
        Err(e) => panic!(
            "Postgres scenarios need a server at {url} ({e}). Start the local one with \
             `docker compose -f ops/compose.yaml up -d postgres` or set DATABASE_URL; \
             tests do not skip."
        ),
    };
    store.reset_for_tests(&default_policy()).await.unwrap();
    store
}

async fn create_database(url: &str) {
    use sqlx::Connection;
    let (base, name) = url.rsplit_once('/').expect("database name in DATABASE_URL");
    let name = name.split('?').next().unwrap();
    assert!(
        name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "database name {name:?} must be a plain identifier"
    );
    let mut admin = sqlx::PgConnection::connect(&format!("{base}/postgres"))
        .await
        .unwrap_or_else(|e| panic!("connect to {base}/postgres to create {name}: {e}"));
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&mut admin)
        .await
        .unwrap_or_else(|e| panic!("CREATE DATABASE {name}: {e}"));
    eprintln!("created test database {name}");
}
