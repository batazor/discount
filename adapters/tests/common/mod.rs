//! Shared fixtures for the adapter tests.
#![allow(dead_code)]

use std::sync::Arc;

use discount_adapters::http::AppState;
use discount_adapters::telemetry::Instruments;
use discount_adapters::{
    CatalogFixture, FixedClock, InMemoryStore, NoopEventPublisher, default_policy,
};
use discount_core::*;

pub const NOW: i64 = 1_760_227_200;

pub fn eur(a: i64) -> Money {
    Money::new(a, "EUR")
}

pub fn fixture() -> CatalogFixture {
    CatalogFixture::from_json_file(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../deploy/promotions.json"
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
        catalog: Arc::new(store.clone()),
        redemptions: Arc::new(store),
        clock: Arc::new(FixedClock(NOW)),
        events: Arc::new(NoopEventPublisher),
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
    serde_json::json!({"customer_id": null, "completed_orders": 0, "uses": []})
}
