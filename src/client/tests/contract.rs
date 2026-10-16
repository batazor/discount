//! Contract test (ADR 10): the generated client drives a running service through the
//! documented flows. Requests are built from JSON in the ubiquitous-language shape and
//! decoded into the generated types, so a mismatch between `api/openapi.yaml` and the
//! server surfaces here.

use std::sync::Arc;

use discount_client::{Client, types};
use discount_infrastructure::http::{AppState, router};
use discount_infrastructure::telemetry::Instruments;
use discount_infrastructure::{FixedClock, InMemoryStore, NoopEventPublisher, default_policy};
use serde_json::json;

async fn start_server() -> String {
    let store = InMemoryStore::new(default_policy());
    let state = Arc::new(AppState {
        catalog: Arc::new(store.clone()),
        redemptions: Arc::new(store),
        clock: Arc::new(FixedClock(1_760_227_200)),
        events: Arc::new(NoopEventPublisher),
        codes: Arc::new(discount_infrastructure::RandomCodes::default()),
        instruments: Instruments::noop(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    format!("http://{addr}")
}

fn from_json<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
    serde_json::from_value(v).expect("value matches the generated type")
}

fn cart() -> serde_json::Value {
    json!({
        "currency": "EUR",
        "lines": [
            {"line_id": "L1", "unit_price": {"amount": 1999, "currency": "EUR"}, "quantity": 2},
            {"line_id": "L2", "unit_price": {"amount": 501, "currency": "EUR"}, "quantity": 1}
        ],
        "tags": [], "attributes": [], "coupon_codes": []
    })
}

fn customer() -> serde_json::Value {
    json!({"customer_id": "c1", "completed_orders": 0, "uses": []})
}

#[tokio::test]
async fn generated_client_drives_the_documented_flows() {
    let base = start_server().await;
    let client = Client::new(&base);

    // health
    let h = client.healthz().await.expect("healthz");
    assert_eq!(h.status(), 200);

    // catalogue: group, promotion, budget, policy
    let group: types::StackingGroup =
        from_json(json!({"group_id": "g", "priority": 0, "mode": {"type": "Stackable"}}));
    let g = client.put_group("g", &group).await.expect("put group");
    assert_eq!(g.group_id, "g");
    let promo: types::Promotion = from_json(json!({
        "promotion_id": "P1", "group_id": "g", "priority": 10, "exclusive": false, "cap": null,
        "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [{"type": "DateWindow", "starts_at": null, "ends_at": null}],
        "action": {"type": "OrderPercentage", "bps": 1000}
    }));
    client
        .put_promotion("P1", &promo)
        .await
        .expect("put promotion");
    let budget: types::Budget =
        from_json(json!({"remaining": {"amount": 100000, "currency": "EUR"}}));
    client.put_budget("P1", &budget).await.expect("put budget");
    let listed = client.list_promotions().await.expect("list promotions");
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].remaining_budget.as_ref().map(|m| m.amount),
        Some(100000)
    );
    let policy = client.get_policy().await.expect("get policy");
    assert!(matches!(
        policy.rounding.type_,
        types::RoundingPolicyType::Down
    ));

    // evaluate
    let req: types::EvaluateRequest = from_json(json!({"cart": cart(), "customer": customer()}));
    let result = client.evaluate(&req).await.expect("evaluate").into_inner();
    assert_eq!(result.original_subtotal.amount, 4499);
    assert_eq!(result.total_discount.amount, 449);
    assert_eq!(result.applied.len(), 1);
    assert_eq!(result.applied[0].promotion_id, "P1");

    // redeem -> idempotent repeat -> confirm -> get -> cancel
    let req: types::RedeemRequest =
        from_json(json!({"order_id": "o-1", "cart": cart(), "customer": customer()}));
    let r1 = client.redeem(&req).await.expect("redeem").into_inner();
    assert!(matches!(r1.status, types::RedemptionStatus::Reserved));
    assert_eq!(r1.result.total_discount.amount, 449);
    let r2 = client
        .redeem(&req)
        .await
        .expect("redeem again")
        .into_inner();
    assert_eq!(
        serde_json::to_value(&r2.result).unwrap(),
        serde_json::to_value(&r1.result).unwrap()
    );
    let budget = client
        .get_budget("P1")
        .await
        .expect("get budget")
        .into_inner();
    assert_eq!(budget.remaining.map(|m| m.amount), Some(100000 - 449));
    let c = client
        .confirm_redemption("o-1")
        .await
        .expect("confirm")
        .into_inner();
    assert!(matches!(c.status, types::RedemptionStatus::Confirmed));
    let stored = client
        .get_redemption("o-1")
        .await
        .expect("get redemption")
        .into_inner();
    assert!(matches!(stored.status, types::RedemptionStatus::Confirmed));
    assert_eq!(stored.customer_id.as_deref(), Some("c1"));
    let again = client.confirm_redemption("o-1").await;
    match again {
        Err(discount_client::Error::ErrorResponse(e)) => assert_eq!(e.status(), 409),
        other => panic!("expected 409, got {other:?}"),
    }
    let cancelled = client
        .cancel_redemption("o-1")
        .await
        .expect("cancel")
        .into_inner();
    assert!(matches!(
        cancelled.status,
        types::RedemptionStatus::Cancelled
    ));
    let budget = client
        .get_budget("P1")
        .await
        .expect("get budget")
        .into_inner();
    assert_eq!(budget.remaining.map(|m| m.amount), Some(100000));

    // 404 and 422 are typed error responses
    let missing = client.get_promotion("nope").await;
    match missing {
        Err(discount_client::Error::ErrorResponse(e)) => assert_eq!(e.status(), 404),
        other => panic!("expected 404, got {other:?}"),
    }
    // a body the schema accepts but `validate` rejects: duplicate line ids
    let mut bad = cart();
    bad["lines"][1]["line_id"] = json!("L1");
    let req: types::EvaluateRequest = from_json(json!({"cart": bad, "customer": customer()}));
    match client.evaluate(&req).await {
        Err(discount_client::Error::ErrorResponse(e)) => {
            assert_eq!(e.status(), 422);
            let body = serde_json::to_value(e.into_inner()).unwrap();
            assert_eq!(body["error"]["type"], "DuplicateLineId");
        }
        other => panic!("expected 422, got {other:?}"),
    }
    // and one the generated type refuses before any request: a malformed currency
    let mut bad = cart();
    bad["currency"] = json!("eur");
    assert!(
        serde_json::from_value::<types::EvaluateRequest>(
            json!({"cart": bad, "customer": customer()})
        )
        .is_err()
    );

    // coupon lifecycle: generate, grant rules, idempotent grant
    let gen_req: types::GenerateCouponsRequest = from_json(json!({
        "promotion_id": "P1", "count": 3, "prefix": "SUMMER-", "starts_at": null, "ends_at": null,
        "max_uses": 1, "customer_id": null
    }));
    let generated = client
        .generate_coupons(&gen_req)
        .await
        .expect("generate")
        .into_inner();
    assert_eq!(generated.codes.len(), 3);
    assert!(generated.codes.iter().all(|c| c.starts_with("SUMMER-")));
    let c = client
        .get_coupon(&generated.codes[0])
        .await
        .expect("get coupon")
        .into_inner();
    assert_eq!((c.max_uses, c.uses), (Some(1), 0));
    let rule: types::GrantRule = from_json(json!({
        "rule_id": "r1", "trigger": "cancel", "promotion_id": "P1", "priority": 1,
        "validity_seconds": 86400, "max_uses": 1
    }));
    client.put_grant_rule("r1", &rule).await.expect("put rule");
    assert_eq!(
        client.list_grant_rules().await.expect("list rules").len(),
        1
    );
    let grant: types::GrantRequest =
        from_json(json!({"trigger": "cancel", "customer_id": "c1", "order_id": "o9"}));
    let g1 = client
        .grant_coupon(&grant)
        .await
        .expect("grant")
        .into_inner();
    let g2 = client
        .grant_coupon(&grant)
        .await
        .expect("grant again")
        .into_inner();
    assert_eq!(g1.code, g2.code);
    assert_eq!(g1.customer_id.as_deref(), Some("c1"));
    assert_eq!(g1.ends_at, Some(1_760_227_200 + 86400));
    client.delete_grant_rule("r1").await.expect("delete rule");
    match client.get_grant_rule("r1").await {
        Err(discount_client::Error::ErrorResponse(e)) => assert_eq!(e.status(), 404),
        other => panic!("expected 404, got {other:?}"),
    }

    // the service serves the very document the client was generated from
    let doc = client.openapi_document().await.expect("openapi document");
    assert_eq!(doc.status(), 200);
}
