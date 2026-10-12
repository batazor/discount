//! The four scenarios of the `evaluate-http-api` spec, through the axum router.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use discount_adapters::http::{AppState, router};
use discount_adapters::telemetry::Instruments;
use discount_adapters::{FixedClock, InMemoryPromotionRepository, NoopEventPublisher};
use discount_core::{OrderCap, PricingInput, PricingPolicy, RoundingPolicy, StackingPolicy, apply};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

fn fixture_repo() -> InMemoryPromotionRepository {
    InMemoryPromotionRepository::from_json_file(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../deploy/promotions.json"
    ))
    .unwrap()
}

fn policy() -> PricingPolicy {
    PricingPolicy {
        rounding: RoundingPolicy::Down,
        stacking: StackingPolicy::Sequential,
        order_cap: OrderCap {
            amount: None,
            bps: None,
        },
    }
}

fn state() -> Arc<AppState> {
    Arc::new(AppState {
        repository: Arc::new(fixture_repo()),
        clock: Arc::new(FixedClock(1_760_227_200)),
        events: Arc::new(NoopEventPublisher),
        policy: policy(),
        instruments: Instruments::noop(),
    })
}

async fn post_json(body: &str) -> (StatusCode, Value) {
    let app = router(state());
    let req = Request::post("/v1/evaluate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn sample_cart() -> Value {
    json!({
        "currency": "EUR",
        "lines": [
            {"line_id": "L1", "unit_price": {"amount": 1999, "currency": "EUR"}, "quantity": 2},
            {"line_id": "L2", "unit_price": {"amount": 501, "currency": "EUR"}, "quantity": 1}
        ]
    })
}

#[tokio::test]
async fn successful_evaluation_matches_core() {
    let body = json!({"cart": sample_cart(), "customer": {}}).to_string();
    let (status, v) = post_json(&body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["final_total"]["amount"], 3750);
    assert_eq!(
        v["original_subtotal"]["amount"].as_i64().unwrap()
            - v["total_discount"]["amount"].as_i64().unwrap(),
        3750
    );
    // identical to discount_core::apply on the same input
    let repo = fixture_repo();
    let input = PricingInput {
        now: 1_760_227_200,
        policy: policy(),
        customer: discount_core::CustomerContext {},
        cart: serde_json::from_value(sample_cart()).unwrap(),
        groups: repo.groups,
        promotions: repo.promotions,
    };
    let expected = serde_json::to_value(apply(&input).unwrap()).unwrap();
    assert_eq!(v, expected);
}

#[tokio::test]
async fn invalid_currency_is_422_with_pricing_error() {
    let mut cart = sample_cart();
    cart["currency"] = json!("eur");
    let (status, v) = post_json(&json!({"cart": cart, "customer": {}}).to_string()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        v,
        json!({"error": {"type": "InvalidCurrency", "field": "cart.currency"}})
    );
}

#[tokio::test]
async fn unknown_key_is_400_decode_error() {
    let (status, v) =
        post_json(&json!({"cart": sample_cart(), "customer": {}, "extra": 1}).to_string()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let msg = v["decode_error"].as_str().expect("decode_error message");
    assert!(msg.contains("extra"), "message names the key: {msg}");
    let (status, v) = post_json("{not json").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(v.get("decode_error").is_some());
}

#[tokio::test]
async fn healthz() {
    let app = router(state());
    let res = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({"status": "ok"})
    );
}
