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
        "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [], "action": {"type": "OrderPercentage", "bps": 1000}});
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
        "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [{"type": "RequiresCoupon"}],
        "action": {"type": "OrderFixedAmount", "amount": {"amount": 1000, "currency": "USD"}}});
    let (status, _) = call(
        &app,
        Method::PUT,
        "/v1/promotions/P1",
        Some(promo.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, c) = call(&app, Method::PUT, "/v1/coupons/WELCOME", Some(json!({"code": "WELCOME", "promotion_id": "P1", "starts_at": null, "ends_at": null, "customer_id": null, "max_uses": null, "uses": 5}).to_string())).await;
    assert_eq!(
        (status, c["code"].as_str(), c["uses"].as_i64()),
        (StatusCode::OK, Some("WELCOME"), Some(0))
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
        "/v1/promotions/{promotion_id}/metadata",
        "/v1/promotions/{promotion_id}/state",
        "/v1/coupons/{code}/state",
        "/v1/promotions/{promotion_id}/allowlist:",
        "/v1/promotions/{promotion_id}/allowlist/customers/{customer_id}",
        "/v1/promotion-metadata",
        "/v1/promotions/{promotion_id}:",
        "/v1/promotions/{promotion_id}/budget",
        "/v1/coupons:",
        "/v1/coupons/{code}",
        "/v1/coupons/generate",
        "/v1/coupons/grant",
        "/v1/grant-rules:",
        "/v1/grant-rules/{rule_id}",
        "/v1/customers/{customer_id}/coupons:",
        "/v1/customers/{customer_id}/coupons/{code}",
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

/// Spec `coupon-lifecycle`: bulk generation, grant rules and grants over HTTP.
#[tokio::test]
async fn coupon_lifecycle_routes() {
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
        "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [{"type": "RequiresCoupon"}],
        "action": {"type": "OrderFixedAmount", "amount": {"amount": 300, "currency": "EUR"}}});
    call(
        &app,
        Method::PUT,
        "/v1/promotions/P1",
        Some(promo.to_string()),
    )
    .await;

    // generate: 100 distinct single-use codes with the prefix
    let gen_req = json!({"promotion_id": "P1", "count": 100, "prefix": "SUMMER-", "starts_at": null,
        "ends_at": null, "max_uses": 1, "customer_id": null});
    let (status, g) = call(
        &app,
        Method::POST,
        "/v1/coupons/generate",
        Some(gen_req.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let codes: Vec<String> = serde_json::from_value(g["codes"].clone()).unwrap();
    assert_eq!(codes.len(), 100);
    let distinct: std::collections::BTreeSet<&String> = codes.iter().collect();
    assert_eq!(distinct.len(), 100);
    assert!(
        codes
            .iter()
            .all(|c| c.starts_with("SUMMER-") && c.len() == "SUMMER-".len() + 8)
    );
    let (status, c) = call(
        &app,
        Method::GET,
        &format!("/v1/coupons/{}", codes[0]),
        None,
    )
    .await;
    assert_eq!(
        (status, c["max_uses"].as_i64(), c["uses"].as_i64()),
        (StatusCode::OK, Some(1), Some(0))
    );
    // unknown promotion -> 404, bad count -> 400
    let mut bad = gen_req.clone();
    bad["promotion_id"] = json!("nope");
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/v1/coupons/generate",
            Some(bad.to_string())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let mut bad = gen_req.clone();
    bad["count"] = json!(0);
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/v1/coupons/generate",
            Some(bad.to_string())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    // single use over HTTP: first redeem applies, second with the same code skips, cancel frees it
    let mut cart = cart_json();
    cart["coupon_codes"] = json!([codes[0]]);
    let redeem = |order: &str| {
        json!({"order_id": order, "cart": cart, "customer": customer_json()}).to_string()
    };
    let (status, r1) = call(&app, Method::POST, "/v1/redeem", Some(redeem("o1"))).await;
    assert_eq!(
        (status, r1["result"]["applied"].as_array().unwrap().len()),
        (StatusCode::OK, 1)
    );
    let (_, c) = call(
        &app,
        Method::GET,
        &format!("/v1/coupons/{}", codes[0]),
        None,
    )
    .await;
    assert_eq!(c["uses"].as_i64(), Some(1));
    let (_, r2) = call(&app, Method::POST, "/v1/redeem", Some(redeem("o2"))).await;
    assert_eq!(r2["result"]["skipped"][0]["reason"]["type"], "NotEligible");
    call(&app, Method::POST, "/v1/redemptions/o1/cancel", None).await;
    let (_, r3) = call(&app, Method::POST, "/v1/redeem", Some(redeem("o3"))).await;
    assert_eq!(r3["result"]["applied"].as_array().unwrap().len(), 1);

    // grant rules: no rule -> 404 on grant; PUT, GET, list; grant twice -> same code
    let grant = json!({"trigger": "cancel", "customer_id": "c1", "order_id": "o9"});
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/v1/coupons/grant",
            Some(grant.to_string())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let rule = json!({"rule_id": "r1", "trigger": "cancel", "promotion_id": "P1", "priority": 1, "validity_seconds": 86400, "max_uses": 1});
    let (status, r) = call(
        &app,
        Method::PUT,
        "/v1/grant-rules/r1",
        Some(rule.to_string()),
    )
    .await;
    assert_eq!(
        (status, r["rule_id"].as_str()),
        (StatusCode::OK, Some("r1"))
    );
    let (status, r) = call(&app, Method::GET, "/v1/grant-rules/r1", None).await;
    assert_eq!((status, r), (StatusCode::OK, rule.clone()));
    let (status, rs) = call(&app, Method::GET, "/v1/grant-rules", None).await;
    assert_eq!((status, rs.as_array().unwrap().len()), (StatusCode::OK, 1));
    let mut bad = rule.clone();
    bad["validity_seconds"] = json!(0);
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/grant-rules/r1",
            Some(bad.to_string())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, g1) = call(
        &app,
        Method::POST,
        "/v1/coupons/grant",
        Some(grant.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (
            g1["promotion_id"].as_str(),
            g1["customer_id"].as_str(),
            g1["starts_at"].as_i64(),
            g1["ends_at"].as_i64(),
            g1["max_uses"].as_i64()
        ),
        (
            Some("P1"),
            Some("c1"),
            Some(NOW),
            Some(NOW + 86400),
            Some(1)
        )
    );
    assert!(g1["code"].as_str().unwrap().starts_with("CANCEL-"));
    let (_, g2) = call(
        &app,
        Method::POST,
        "/v1/coupons/grant",
        Some(grant.to_string()),
    )
    .await;
    assert_eq!(g1, g2);
    let (_, cs) = call(&app, Method::GET, "/v1/coupons", None).await;
    assert_eq!(
        cs.as_array()
            .unwrap()
            .iter()
            .filter(|c| c["code"] == g1["code"])
            .count(),
        1
    );
    // a different order gets a different code
    let other = json!({"trigger": "cancel", "customer_id": "c1", "order_id": "o10"});
    let (_, g3) = call(
        &app,
        Method::POST,
        "/v1/coupons/grant",
        Some(other.to_string()),
    )
    .await;
    assert_ne!(g1["code"], g3["code"]);
    let (status, _) = call(&app, Method::DELETE, "/v1/grant-rules/r1", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        call(&app, Method::DELETE, "/v1/grant-rules/r1", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

/// Spec `customer-coupons` over HTTP: list, save (200/404/409), forget (204/404).
#[tokio::test]
async fn customer_coupon_routes() {
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
        "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [{"type": "RequiresCoupon"}],
        "action": {"type": "OrderPercentage", "bps": 1000}});
    call(
        &app,
        Method::PUT,
        "/v1/promotions/P1",
        Some(promo.to_string()),
    )
    .await;
    let coupon = |code: &str, cid: Option<&str>| {
        json!({"code": code, "promotion_id": "P1", "starts_at": null, "ends_at": null, "customer_id": cid, "max_uses": null, "uses": 0}).to_string()
    };
    call(
        &app,
        Method::PUT,
        "/v1/coupons/GRANT-1",
        Some(coupon("GRANT-1", Some("c1"))),
    )
    .await;
    call(
        &app,
        Method::PUT,
        "/v1/coupons/GRANT-2",
        Some(coupon("GRANT-2", Some("c2"))),
    )
    .await;
    call(
        &app,
        Method::PUT,
        "/v1/coupons/SPRING",
        Some(coupon("SPRING", None)),
    )
    .await;
    call(
        &app,
        Method::PUT,
        "/v1/coupons/WELCOME",
        Some(coupon("WELCOME", None)),
    )
    .await;
    let mut cart = cart_json();
    cart["coupon_codes"] = json!(["WELCOME"]);
    let customer = json!({"customer_id": "c1", "completed_orders": 0, "uses": [], "attributes": [], "allowlisted": []});
    let (status, _) = call(
        &app,
        Method::POST,
        "/v1/redeem",
        Some(json!({"order_id": "o1", "cart": cart, "customer": customer}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, l) = call(&app, Method::GET, "/v1/customers/nobody/coupons", None).await;
    assert_eq!((status, l), (StatusCode::OK, json!([])));
    let (status, saved) = call(&app, Method::PUT, "/v1/customers/c1/coupons/SPRING", None).await;
    assert_eq!(
        (
            status,
            saved["status"].as_str(),
            saved["coupon"]["code"].as_str()
        ),
        (StatusCode::OK, Some("active"), Some("SPRING"))
    );
    assert_eq!(
        call(&app, Method::PUT, "/v1/customers/c1/coupons/SPRING", None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, Method::PUT, "/v1/customers/c1/coupons/NOPE", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, b) = call(&app, Method::PUT, "/v1/customers/c1/coupons/GRANT-2", None).await;
    assert_eq!(
        (status, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("BoundToAnotherCustomer"))
    );

    let (status, l) = call(&app, Method::GET, "/v1/customers/c1/coupons", None).await;
    assert_eq!(status, StatusCode::OK);
    let entries = l.as_array().unwrap();
    let summary: Vec<(&str, &str, Option<&str>)> = entries
        .iter()
        .map(|e| {
            (
                e["coupon"]["code"].as_str().unwrap(),
                e["status"].as_str().unwrap(),
                e["order_id"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("GRANT-1", "active", None),
            ("SPRING", "active", None),
            ("WELCOME", "applied", Some("o1"))
        ]
    );
    assert_eq!(
        entries[0]["promotion"],
        json!({"promotion_id": "P1", "action": {"type": "OrderPercentage", "bps": 1000}, "metadata": null})
    );

    assert_eq!(
        call(
            &app,
            Method::DELETE,
            "/v1/customers/c1/coupons/SPRING",
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app,
            Method::DELETE,
            "/v1/customers/c1/coupons/SPRING",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // forgetting a bound coupon is a 404 too and it stays listed
    assert_eq!(
        call(
            &app,
            Method::DELETE,
            "/v1/customers/c1/coupons/GRANT-1",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, l) = call(&app, Method::GET, "/v1/customers/c1/coupons", None).await;
    let codes: Vec<&str> = l
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["coupon"]["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, vec!["GRANT-1", "WELCOME"]);
}

/// Spec `promotion-catalog`, metadata over HTTP: round trip, limits, status codes, listing and
/// the source filter on promotions and customer coupons.
#[tokio::test]
async fn promotion_metadata_routes() {
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
    for id in ["P1", "P2", "P3"] {
        let promo = json!({"promotion_id": id, "group_id": "g", "priority": 0, "exclusive": false, "cap": null,
            "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [{"type": "RequiresCoupon"}],
            "action": {"type": "OrderPercentage", "bps": 1000}});
        call(
            &app,
            Method::PUT,
            &format!("/v1/promotions/{id}"),
            Some(promo.to_string()),
        )
        .await;
    }
    let md = json!({"title": "Summer", "description": "", "source": "hotels", "labels": {"channel": "app"}});
    assert_eq!(
        call(&app, Method::GET, "/v1/promotions/P1/metadata", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, body) = call(
        &app,
        Method::PUT,
        "/v1/promotions/P1/metadata",
        Some(md.to_string()),
    )
    .await;
    assert_eq!((status, &body), (StatusCode::OK, &md));
    let (status, body) = call(&app, Method::GET, "/v1/promotions/P1/metadata", None).await;
    assert_eq!((status, body), (StatusCode::OK, md.clone()));
    let cars = json!({"title": "Roads", "description": "x", "source": "cars", "labels": {}});
    call(
        &app,
        Method::PUT,
        "/v1/promotions/P2/metadata",
        Some(cars.to_string()),
    )
    .await;
    // limits -> 400 decode_error; unknown promotion -> 404; unknown key -> 400
    let mut bad = md.clone();
    bad["title"] = json!("");
    let (status, b) = call(
        &app,
        Method::PUT,
        "/v1/promotions/P1/metadata",
        Some(bad.to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(b["decode_error"].as_str().unwrap().contains("title"));
    let mut bad = md.clone();
    bad["labels"] =
        serde_json::Value::Object((0..21).map(|i| (format!("k{i}"), json!(""))).collect());
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/P1/metadata",
            Some(bad.to_string())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut bad = md.clone();
    bad["source"] = json!("Hotels");
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/P1/metadata",
            Some(bad.to_string())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut bad = md.clone();
    bad["extra"] = json!(1);
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/P1/metadata",
            Some(bad.to_string())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/nope/metadata",
            Some(md.to_string())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // listing and filters
    let (status, l) = call(&app, Method::GET, "/v1/promotion-metadata", None).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = l
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["promotion_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["P1", "P2"]);
    let (_, l) = call(
        &app,
        Method::GET,
        "/v1/promotion-metadata?source=cars",
        None,
    )
    .await;
    assert_eq!(l.as_array().unwrap().len(), 1);
    let (_, l) = call(&app, Method::GET, "/v1/promotions", None).await;
    assert_eq!(l.as_array().unwrap().len(), 3);
    let (_, l) = call(&app, Method::GET, "/v1/promotions?source=hotels", None).await;
    assert_eq!(
        l.as_array()
            .unwrap()
            .iter()
            .map(|p| p["promotion_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["P1"]
    );
    let (_, l) = call(&app, Method::GET, "/v1/promotions?source=transfers", None).await;
    assert_eq!(l.as_array().unwrap().len(), 0);
    // customer coupons: metadata in the entry, source filter
    let coupon = |code: &str, pid: &str| {
        json!({"code": code, "promotion_id": pid, "starts_at": null, "ends_at": null, "customer_id": "c1", "max_uses": null, "uses": 0}).to_string()
    };
    call(
        &app,
        Method::PUT,
        "/v1/coupons/H1",
        Some(coupon("H1", "P1")),
    )
    .await;
    call(
        &app,
        Method::PUT,
        "/v1/coupons/N1",
        Some(coupon("N1", "P3")),
    )
    .await;
    let (_, l) = call(&app, Method::GET, "/v1/customers/c1/coupons", None).await;
    let entries = l.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["promotion"]["metadata"], md);
    assert_eq!(entries[1]["promotion"]["metadata"], serde_json::Value::Null);
    let (_, l) = call(
        &app,
        Method::GET,
        "/v1/customers/c1/coupons?source=hotels",
        None,
    )
    .await;
    assert_eq!(
        l.as_array()
            .unwrap()
            .iter()
            .map(|e| e["coupon"]["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["H1"]
    );
    // archive hides, delete removes
    call(&app, Method::DELETE, "/v1/promotions/P2", None).await;
    assert_eq!(
        call(&app, Method::GET, "/v1/promotions/P2/metadata", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, l) = call(&app, Method::GET, "/v1/promotion-metadata", None).await;
    assert_eq!(l.as_array().unwrap().len(), 1);
    assert_eq!(
        call(&app, Method::DELETE, "/v1/promotions/P1/metadata", None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, Method::DELETE, "/v1/promotions/P1/metadata", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

/// Spec `promotion-catalog`, "Allowlists" over HTTP, and the gating through `/v1/evaluate`.
#[tokio::test]
async fn allowlist_routes() {
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
        "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [{"type": "RequiresAllowlist"}],
        "action": {"type": "OrderPercentage", "bps": 1000}});
    call(
        &app,
        Method::PUT,
        "/v1/promotions/P1",
        Some(promo.to_string()),
    )
    .await;
    let (status, l) = call(&app, Method::GET, "/v1/promotions/P1/allowlist", None).await;
    assert_eq!((status, l), (StatusCode::OK, json!({"customer_ids": []})));
    let (status, l) = call(
        &app,
        Method::PUT,
        "/v1/promotions/P1/allowlist",
        Some(json!({"customer_ids": ["c2", "c1", "c1"]}).to_string()),
    )
    .await;
    assert_eq!(
        (status, l),
        (StatusCode::OK, json!({"customer_ids": ["c1", "c2"]}))
    );
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/P1/allowlist",
            Some(json!({"customer_ids": [""]}).to_string())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/P1/allowlist",
            Some(json!({"ids": []}).to_string())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/nope/allowlist",
            Some(json!({"customer_ids": ["c1"]}).to_string())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, Method::GET, "/v1/promotions/nope/allowlist", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app,
            Method::GET,
            "/v1/promotions/P1/allowlist/customers/c1",
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app,
            Method::GET,
            "/v1/promotions/P1/allowlist/customers/c9",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/P1/allowlist/customers/c3",
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/nope/allowlist/customers/c3",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, l) = call(&app, Method::GET, "/v1/promotions/P1/allowlist", None).await;
    assert_eq!(l, json!({"customer_ids": ["c1", "c2", "c3"]}));
    // gating: listed customer applies, unlisted is NotEligible on rule 0
    let customer = |id: &str| json!({"customer_id": id, "completed_orders": 0, "uses": [], "attributes": [], "allowlisted": []});
    let (status, ev) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": customer("c1")}).to_string()),
    )
    .await;
    assert_eq!(
        (status, ev["applied"].as_array().unwrap().len()),
        (StatusCode::OK, 1)
    );
    let (_, ev) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": customer("c9")}).to_string()),
    )
    .await;
    assert_eq!(
        ev["skipped"][0]["reason"],
        json!({"type": "NotEligible", "rule_index": 0})
    );
    // the customer's attributes travel through the request
    let mut tiered = customer("c9");
    tiered["attributes"] = json!([{"key": "tier", "value": "gold"}]);
    let promo2 = json!({"promotion_id": "P2", "group_id": "g", "priority": 0, "exclusive": false, "cap": null,
        "remaining_budget": null, "uses_total": 0, "rounding": null,
        "rules": [{"type": "CustomerAttributeIn", "key": "tier", "values": ["gold"]}],
        "action": {"type": "OrderFixedAmount", "amount": {"amount": 100, "currency": "EUR"}}});
    call(
        &app,
        Method::PUT,
        "/v1/promotions/P2",
        Some(promo2.to_string()),
    )
    .await;
    let (_, ev) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": tiered}).to_string()),
    )
    .await;
    assert_eq!(
        ev["applied"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["promotion_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["P2"]
    );
    // remove, clear, archive
    assert_eq!(
        call(
            &app,
            Method::DELETE,
            "/v1/promotions/P1/allowlist/customers/c1",
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app,
            Method::DELETE,
            "/v1/promotions/P1/allowlist/customers/c1",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, Method::DELETE, "/v1/promotions/P1/allowlist", None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, l) = call(&app, Method::GET, "/v1/promotions/P1/allowlist", None).await;
    assert_eq!(l, json!({"customer_ids": []}));
    call(&app, Method::DELETE, "/v1/promotions/P1", None).await;
    assert_eq!(
        call(&app, Method::GET, "/v1/promotions/P1/allowlist", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

/// Spec `catalogue-states` over HTTP: the state resources, the listing filter, the effect on
/// evaluations and the 409 on generation.
#[tokio::test]
async fn state_routes() {
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
    for id in ["P1", "P2"] {
        let promo = json!({"promotion_id": id, "group_id": "g", "priority": 0, "exclusive": false, "cap": null,
            "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [],
            "action": {"type": "OrderFixedAmount", "amount": {"amount": 100, "currency": "EUR"}}});
        call(
            &app,
            Method::PUT,
            &format!("/v1/promotions/{id}"),
            Some(promo.to_string()),
        )
        .await;
    }
    let (status, st) = call(&app, Method::GET, "/v1/promotions/P1/state", None).await;
    assert_eq!((status, st), (StatusCode::OK, json!({"state": "active"})));
    let (status, st) = call(
        &app,
        Method::PUT,
        "/v1/promotions/P1/state",
        Some(json!({"state": "paused"}).to_string()),
    )
    .await;
    assert_eq!((status, st), (StatusCode::OK, json!({"state": "paused"})));
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/P1/state",
            Some(json!({"state": "off"}).to_string())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/promotions/nope/state",
            Some(json!({"state": "paused"}).to_string())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, l) = call(&app, Method::GET, "/v1/promotions", None).await;
    assert_eq!(l.as_array().unwrap().len(), 2);
    let (_, l) = call(&app, Method::GET, "/v1/promotions?state=paused", None).await;
    assert_eq!(
        l.as_array()
            .unwrap()
            .iter()
            .map(|p| p["promotion_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["P1"]
    );
    let (_, ev) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(json!({"cart": cart_json(), "customer": customer_json()}).to_string()),
    )
    .await;
    assert_eq!(
        ev["applied"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["promotion_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["P2"]
    );
    assert!(ev["skipped"].as_array().unwrap().is_empty());
    let gen_req = json!({"promotion_id": "P1", "count": 1, "prefix": "X-", "starts_at": null, "ends_at": null, "max_uses": null, "customer_id": null});
    let (status, b) = call(
        &app,
        Method::POST,
        "/v1/coupons/generate",
        Some(gen_req.to_string()),
    )
    .await;
    assert_eq!(
        (status, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("PromotionPaused"))
    );
    call(&app, Method::PUT, "/v1/coupons/W", Some(json!({"code": "W", "promotion_id": "P2", "starts_at": null, "ends_at": null, "customer_id": null, "max_uses": null, "uses": 0}).to_string())).await;
    let (status, st) = call(
        &app,
        Method::PUT,
        "/v1/coupons/W/state",
        Some(json!({"state": "paused"}).to_string()),
    )
    .await;
    assert_eq!((status, st), (StatusCode::OK, json!({"state": "paused"})));
    let (_, l) = call(&app, Method::GET, "/v1/coupons?state=paused", None).await;
    assert_eq!(l.as_array().unwrap().len(), 1);
    let (_, l) = call(&app, Method::GET, "/v1/coupons?state=active", None).await;
    assert_eq!(l.as_array().unwrap().len(), 0);
    assert_eq!(
        call(&app, Method::GET, "/v1/coupons/nope/state", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, _) = call(
        &app,
        Method::PUT,
        "/v1/promotions/P1/state",
        Some(json!({"state": "active"}).to_string()),
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
    assert_eq!(ev["applied"].as_array().unwrap().len(), 2);
}
