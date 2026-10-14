//! HTTP scenarios of `evaluate-http-api`, `redemption` and `promotion-catalog` through
//! the axum router on the in-memory store.

mod common;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::*;
use discount_domain::{CustomerContext, PricingInput, apply};
use discount_infrastructure::http::{AppState, router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn call(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<String>,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.map(Body::from).unwrap_or_else(Body::empty))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, v)
}

fn cart_json() -> Value {
    serde_json::to_value(sample_cart()).unwrap()
}

#[tokio::test]
async fn successful_evaluation_matches_core() {
    let st: Arc<AppState> = state();
    let app = router(st.clone());
    let (status, v) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": customer_json()}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["final_total"]["amount"], 3750);
    let f = fixture();
    let input = PricingInput {
        now: NOW,
        policy: discount_infrastructure::default_policy(),
        customer: CustomerContext::default(),
        cart: sample_cart(),
        coupons: vec![],
        exchange_rates: vec![],
        groups: f.groups,
        promotions: f.promotions,
    };
    assert_eq!(v, serde_json::to_value(apply(&input).unwrap()).unwrap());
}

#[tokio::test]
async fn invalid_currency_is_422_with_pricing_error() {
    let app = router(state());
    let mut cart = cart_json();
    cart["currency"] = json!("eur");
    let (status, v) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart, "customer": customer_json()}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        v,
        json!({"error": {"type": "InvalidCurrency", "field": "cart.currency"}})
    );
}

#[tokio::test]
async fn unknown_key_is_400_decode_error() {
    let app = router(state());
    let (status, v) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": customer_json(), "extra": 1}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(v["decode_error"].as_str().unwrap().contains("extra"));
    let (status, v) = call(&app, Method::POST, "/v1/evaluate", Some("{not json".into())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(v.get("decode_error").is_some());
}

#[tokio::test]
async fn healthz() {
    let app = router(state());
    let (status, v) = call(&app, Method::GET, "/healthz", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v, json!({"status": "ok"}));
}

#[tokio::test]
async fn redeem_confirm_cancel_over_http() {
    let app = router(state());
    let body =
        json!({"order_id": "o-1", "cart": cart_json(), "customer": customer_json()}).to_string();
    let (status, v) = call(&app, Method::POST, "/v1/redeem", Some(body.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "reserved");
    assert_eq!(v["order_id"], "o-1");
    let (_, ev) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": customer_json()}).to_string()),
    )
    .await;
    assert_eq!(
        v["result"], ev,
        "redeem result equals evaluate for the same cart"
    );
    // idempotent repeat
    let (status, v2) = call(&app, Method::POST, "/v1/redeem", Some(body)).await;
    assert_eq!((status, &v2), (StatusCode::OK, &v));
    // get
    let (status, g) = call(&app, Method::GET, "/v1/redemptions/o-1", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(g["status"], "reserved");
    // confirm, then confirm again -> 409
    let (status, c) = call(&app, Method::POST, "/v1/redemptions/o-1/confirm", None).await;
    assert_eq!(
        (status, c["status"].as_str()),
        (StatusCode::OK, Some("confirmed"))
    );
    let (status, c) = call(&app, Method::POST, "/v1/redemptions/o-1/confirm", None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        c,
        json!({"error": "InvalidTransition", "from": "confirmed", "to": "confirmed"})
    );
    // cancel
    let (status, c) = call(&app, Method::POST, "/v1/redemptions/o-1/cancel", None).await;
    assert_eq!(
        (status, c["status"].as_str()),
        (StatusCode::OK, Some("cancelled"))
    );
    // 404s
    let (status, n) = call(&app, Method::POST, "/v1/redemptions/nope/confirm", None).await;
    assert_eq!(
        (status, n),
        (StatusCode::NOT_FOUND, json!({"error": "NotFound"}))
    );
    let (status, _) = call(&app, Method::GET, "/v1/redemptions/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // 422 stores nothing
    let mut bad = cart_json();
    bad["currency"] = json!("eur");
    let (status, e) = call(
        &app,
        Method::POST,
        "/v1/redeem",
        Some(json!({"order_id": "o-2", "cart": bad, "customer": customer_json()}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(e["error"]["type"], "InvalidCurrency");
    let (status, _) = call(&app, Method::GET, "/v1/redemptions/o-2", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn catalogue_routes() {
    let app = router(state_with(
        discount_infrastructure::InMemoryStore::new(discount_infrastructure::default_policy()),
        discount_infrastructure::telemetry::Instruments::noop(),
    ));
    let (status, v) = call(&app, Method::GET, "/v1/promotions", None).await;
    assert_eq!((status, v), (StatusCode::OK, json!([])));
    // create group and promotion, then price
    let (status, _) = call(
        &app,
        Method::PUT,
        "/v1/groups/g",
        Some(json!({"group_id": "g", "priority": 0, "mode": {"type": "Stackable"}}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let promo = json!({"promotion_id": "P10", "group_id": "g", "priority": 0, "exclusive": false, "cap": null,
        "remaining_budget": null, "rounding": null, "rules": [], "action": {"type": "OrderPercentage", "bps": 1000}});
    let (status, _) = call(
        &app,
        Method::PUT,
        "/v1/promotions/P10",
        Some(promo.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, ev) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": customer_json()}).to_string()),
    )
    .await;
    assert_eq!(ev["total_discount"]["amount"], 449);
    // budget flows into pricing
    let (status, b) = call(
        &app,
        Method::PUT,
        "/v1/promotions/P10/budget",
        Some(json!({"remaining": {"amount": 250, "currency": "EUR"}}).to_string()),
    )
    .await;
    assert_eq!(
        (status, b),
        (
            StatusCode::OK,
            json!({"remaining": {"amount": 250, "currency": "EUR"}})
        )
    );
    let (_, ev) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": customer_json()}).to_string()),
    )
    .await;
    assert_eq!(ev["applied"][0]["clamped_by"], json!({"type": "Budget"}));
    let (status, p) = call(&app, Method::GET, "/v1/promotions/P10", None).await;
    assert_eq!(
        (status, &p["remaining_budget"]),
        (StatusCode::OK, &json!({"amount": 250, "currency": "EUR"}))
    );
    let (status, _) = call(&app, Method::GET, "/v1/promotions/P10/budget", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &app,
        Method::PUT,
        "/v1/promotions/nope/budget",
        Some(json!({"remaining": null}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // policy
    let (status, pol) = call(&app, Method::GET, "/v1/policy", None).await;
    assert_eq!(
        (status, pol["rounding"].clone()),
        (StatusCode::OK, json!({"type": "Down"}))
    );
    let (status, _) = call(&app, Method::PUT, "/v1/policy", Some(json!({"rounding": {"type": "HalfUp"}, "stacking": {"type": "Additive"}, "order_cap": {"amount": null, "bps": null}}).to_string())).await;
    assert_eq!(status, StatusCode::OK);
    let (_, pol) = call(&app, Method::GET, "/v1/policy", None).await;
    assert_eq!(pol["stacking"], json!({"type": "Additive"}));
    // unknown key -> 400
    let (status, _) = call(
        &app,
        Method::PUT,
        "/v1/groups/h",
        Some(
            json!({"group_id": "h", "priority": 0, "mode": {"type": "Stackable"}, "x": 1})
                .to_string(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // archive hides
    let (status, _) = call(&app, Method::DELETE, "/v1/promotions/P10", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, v) = call(&app, Method::GET, "/v1/promotions", None).await;
    assert_eq!((status, v), (StatusCode::OK, json!([])));
    let (_, ev) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": customer_json()}).to_string()),
    )
    .await;
    assert_eq!(ev["total_discount"]["amount"], 0);
    let (status, _) = call(&app, Method::DELETE, "/v1/promotions/P10", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&app, Method::DELETE, "/v1/groups/g", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, Method::GET, "/v1/groups/g", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn coupon_and_rate_routes() {
    let app = router(state_with(
        discount_infrastructure::InMemoryStore::new(discount_infrastructure::default_policy()),
        discount_infrastructure::telemetry::Instruments::noop(),
    ));
    call(
        &app,
        Method::PUT,
        "/v1/groups/g",
        Some(json!({"group_id": "g", "priority": 0, "mode": {"type": "Stackable"}}).to_string()),
    )
    .await;
    let promo = json!({"promotion_id": "P1", "group_id": "g", "priority": 0, "exclusive": false, "cap": null,
        "remaining_budget": null, "rounding": null, "rules": [{"type": "RequiresCoupon"}],
        "action": {"type": "OrderFixedAmount", "amount": {"amount": 1000, "currency": "USD"}}});
    let (status, _) = call(
        &app,
        Method::PUT,
        "/v1/promotions/P1",
        Some(promo.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, c) = call(&app, Method::PUT, "/v1/coupons/WELCOME", Some(json!({"code": "WELCOME", "promotion_id": "P1", "starts_at": null, "ends_at": null, "customer_id": null}).to_string())).await;
    assert_eq!(
        (status, c["code"].as_str()),
        (StatusCode::OK, Some("WELCOME"))
    );
    let (status, r) = call(
        &app,
        Method::PUT,
        "/v1/exchange-rates/USD/EUR",
        Some(json!({"numerator": 92, "denominator": 100}).to_string()),
    )
    .await;
    assert_eq!((status, r["from"].as_str()), (StatusCode::OK, Some("USD")));
    let (status, cs) = call(&app, Method::GET, "/v1/coupons", None).await;
    assert_eq!((status, cs.as_array().unwrap().len()), (StatusCode::OK, 1));
    let (status, rs) = call(&app, Method::GET, "/v1/exchange-rates", None).await;
    assert_eq!((status, rs.as_array().unwrap().len()), (StatusCode::OK, 1));
    // evaluate with the code: converted USD amount applies
    let mut cart = cart_json();
    cart["coupon_codes"] = json!(["WELCOME"]);
    let (status, ev) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart, "customer": customer_json()}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ev["applied"][0]["discount"]["amount"], 920);
    // without the code: skipped
    let (_, ev) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": customer_json()}).to_string()),
    )
    .await;
    assert_eq!(ev["skipped"][0]["reason"]["type"], "NotEligible");
    let (status, _) = call(&app, Method::DELETE, "/v1/coupons/WELCOME", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, Method::DELETE, "/v1/exchange-rates/USD/EUR", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, Method::GET, "/v1/coupons/WELCOME", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Every route the router serves is documented in `docs/openapi.yaml`, and the document is
/// served by the service itself.
#[tokio::test]
async fn openapi_covers_every_route() {
    let app = router(state());
    let (status, _) = call(&app, Method::GET, "/healthz", None).await;
    assert_eq!(status, StatusCode::OK);
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/openapi.yaml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let doc = String::from_utf8(bytes.to_vec()).unwrap();
    assert_eq!(doc, discount_infrastructure::http::OPENAPI_YAML);
    for path in [
        "/v1/evaluate",
        "/v1/redeem",
        "/v1/redemptions/{order_id}:",
        "/v1/redemptions/{order_id}/confirm",
        "/v1/redemptions/{order_id}/cancel",
        "/v1/policy",
        "/v1/groups:",
        "/v1/groups/{group_id}",
        "/v1/promotions:",
        "/v1/promotions/{promotion_id}:",
        "/v1/promotions/{promotion_id}/budget",
        "/v1/coupons:",
        "/v1/coupons/{code}",
        "/v1/exchange-rates:",
        "/v1/exchange-rates/{from}/{to}",
        "/healthz",
        "/openapi.yaml",
        "/docs",
    ] {
        assert!(
            doc.contains(&format!("  {path}")),
            "openapi.yaml lacks path {path}"
        );
    }
    let res = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/docs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
