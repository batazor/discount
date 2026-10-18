//! Contract test (ADR 10): the generated client drives a running service through the
//! documented flows. Requests are built from JSON in the ubiquitous-language shape and
//! decoded into the generated types, so a mismatch between `api/openapi.yaml` and the
//! server surfaces here.

use std::sync::Arc;

use discount_client::{Client, types};
use discount_infrastructure::http::{AppState, AuthMode, router};
use discount_infrastructure::telemetry::Instruments;
use discount_infrastructure::{
    FixedClock, InMemoryStore, InMemoryTenants, NoopEventPublisher, default_policy,
};
use serde_json::json;

async fn start_server() -> String {
    let store = InMemoryStore::new(default_policy());
    let state = Arc::new(AppState {
        tenancy: Arc::new(InMemoryTenants::single(store, default_policy())),
        auth: AuthMode::Off,
        clock: Arc::new(FixedClock(1_760_227_200)),
        events: Arc::new(NoopEventPublisher),
        codes: Arc::new(discount_infrastructure::RandomCodes::default()),
        fetcher: Arc::new(discount_infrastructure::CannedFetcher::default()),
        instruments: Instruments::noop(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    format!("http://{addr}")
}

/// A server with OIDC authentication against the fixture JWKS, and a token for it.
async fn start_secured_server() -> (String, String) {
    use discount_infrastructure::OidcConfig;
    use discount_infrastructure::auth::{StaticKeySource, Verifier};
    const ISSUER: &str = "https://idp.example/realms/discount";
    let jwks: jsonwebtoken::jwk::JwkSet = serde_json::from_str(include_str!(
        "../../infrastructure/tests/fixtures/jwt_test_jwks.json"
    ))
    .unwrap();
    let mut keys = std::collections::HashMap::new();
    keys.insert(ISSUER.to_string(), jwks);
    let config = OidcConfig {
        issuers: vec![ISSUER.to_string()],
        audience: None,
        tenant_claim: "tenant_id".into(),
        scopes_claim: "scope".into(),
        autocreate_tenants: true,
    };
    let verifier = Verifier::new(config, Arc::new(StaticKeySource(keys)));
    let store = InMemoryStore::new(default_policy());
    let state = Arc::new(AppState {
        tenancy: Arc::new(InMemoryTenants::single(store, default_policy())),
        auth: AuthMode::Oidc {
            verifier: Arc::new(verifier),
            autocreate_tenants: true,
        },
        clock: Arc::new(FixedClock(1_760_227_200)),
        events: Arc::new(NoopEventPublisher),
        codes: Arc::new(discount_infrastructure::RandomCodes::default()),
        fetcher: Arc::new(discount_infrastructure::CannedFetcher::default()),
        instruments: Instruments::noop(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("test-key-1".into());
    let claims = json!({"iss": ISSUER, "sub": "svc", "exp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() + 3600, "tenant_id": "hotels", "scope": "discount:pricing"});
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(include_bytes!(
        "../../infrastructure/tests/fixtures/jwt_test_key.pem"
    ))
    .unwrap();
    let token = jsonwebtoken::encode(&header, &claims, &key).unwrap();
    (format!("http://{addr}"), token)
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
    json!({"customer_id": "c1", "completed_orders": 0, "uses": [], "attributes": [], "allowlisted": []})
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
    let listed = client
        .list_promotions(None, None)
        .await
        .expect("list promotions");
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

    // customer coupons: the granted code is listed for c1, a public code can be saved and forgotten
    let listed = client
        .list_customer_coupons("c1", None)
        .await
        .expect("list customer coupons")
        .into_inner();
    assert!(
        listed
            .iter()
            .any(|e| e.coupon.code == g1.code && matches!(e.status, types::CouponStatus::Active))
    );
    let saved = client
        .save_customer_coupon("c1", &generated.codes[0])
        .await
        .expect("save coupon")
        .into_inner();
    assert_eq!(saved.coupon.code, generated.codes[0]);
    assert!(
        saved
            .promotion
            .as_ref()
            .is_some_and(|p| p.promotion_id == "P1")
    );
    match client.save_customer_coupon("c1", "NOPE").await {
        Err(discount_client::Error::ErrorResponse(e)) => assert_eq!(e.status(), 404),
        other => panic!("expected 404, got {other:?}"),
    }
    client
        .forget_customer_coupon("c1", &generated.codes[0])
        .await
        .expect("forget coupon");
    let listed = client
        .list_customer_coupons("c1", None)
        .await
        .expect("list again")
        .into_inner();
    assert!(listed.iter().all(|e| e.coupon.code != generated.codes[0]));

    // promotion metadata: round trip, listing, source filter on promotions and customer coupons
    let md: types::PromotionMetadata = from_json(json!({
        "title": "Summer sale", "description": "10 % off", "source": "hotels", "labels": {"channel": "app"}
    }));
    let stored = client
        .put_promotion_metadata("P1", &md)
        .await
        .expect("put metadata")
        .into_inner();
    assert_eq!(stored.title.as_str(), "Summer sale");
    let got = client
        .get_promotion_metadata("P1")
        .await
        .expect("get metadata")
        .into_inner();
    assert_eq!(got.source.as_ref().map(|s| s.as_str()), Some("hotels"));
    let entries = client
        .list_promotion_metadata(None)
        .await
        .expect("list metadata")
        .into_inner();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].promotion_id, "P1");
    let hotels: types::ListPromotionsSource = "hotels".parse().unwrap();
    assert_eq!(
        client
            .list_promotions(Some(&hotels), None)
            .await
            .expect("filtered")
            .len(),
        1
    );
    let cars: types::ListPromotionsSource = "cars".parse().unwrap();
    assert_eq!(
        client
            .list_promotions(Some(&cars), None)
            .await
            .expect("filtered")
            .len(),
        0
    );
    let cc_hotels: types::ListCustomerCouponsSource = "hotels".parse().unwrap();
    let listed = client
        .list_customer_coupons("c1", Some(&cc_hotels))
        .await
        .expect("filtered coupons")
        .into_inner();
    assert!(!listed.is_empty());
    assert!(listed.iter().all(|e| {
        e.promotion
            .as_ref()
            .and_then(|p| p.metadata.as_ref())
            .and_then(|m| m.source.as_ref())
            .map(|s| s.as_str())
            == Some("hotels")
    }));
    match client.put_promotion_metadata("nope", &md).await {
        Err(discount_client::Error::ErrorResponse(e)) => assert_eq!(e.status(), 404),
        other => panic!("expected 404, got {other:?}"),
    }
    client
        .delete_promotion_metadata("P1")
        .await
        .expect("delete metadata");
    match client.get_promotion_metadata("P1").await {
        Err(discount_client::Error::ErrorResponse(e)) => assert_eq!(e.status(), 404),
        other => panic!("expected 404, got {other:?}"),
    }

    // allowlists: replace, read back sorted, add one, membership, remove
    let al: types::Allowlist = from_json(json!({"customer_ids": ["c2", "c1"]}));
    let stored = client
        .put_allowlist("P1", &al)
        .await
        .expect("put allowlist")
        .into_inner();
    assert_eq!(
        stored
            .customer_ids
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>(),
        vec!["c1", "c2"]
    );
    assert_eq!(
        client
            .add_to_allowlist("P1", "c3")
            .await
            .expect("add")
            .status(),
        204
    );
    let got = client
        .get_allowlist("P1")
        .await
        .expect("get allowlist")
        .into_inner();
    assert_eq!(got.customer_ids.len(), 3);
    assert_eq!(
        client
            .is_allowlisted("P1", "c3")
            .await
            .expect("member")
            .status(),
        204
    );
    client
        .remove_from_allowlist("P1", "c3")
        .await
        .expect("remove");
    match client.is_allowlisted("P1", "c3").await {
        Err(discount_client::Error::ErrorResponse(e)) => assert_eq!(e.status(), 404),
        other => panic!("expected 404, got {other:?}"),
    }
    client.delete_allowlist("P1").await.expect("clear");

    // states: pause P1, the listing filter and the evaluation follow, resume
    let paused: types::StateBody = from_json(json!({"state": "paused"}));
    let st = client
        .put_promotion_state("P1", &paused)
        .await
        .expect("pause")
        .into_inner();
    assert!(matches!(st.state, types::CatalogState::Paused));
    let got = client
        .get_promotion_state("P1")
        .await
        .expect("get state")
        .into_inner();
    assert!(matches!(got.state, types::CatalogState::Paused));
    let only_paused = client
        .list_promotions(None, Some(types::CatalogState::Paused))
        .await
        .expect("filtered by state");
    assert_eq!(only_paused.len(), 1);
    let req: types::EvaluateRequest = from_json(json!({"cart": cart(), "customer": customer()}));
    let r = client
        .evaluate(&req)
        .await
        .expect("evaluate while paused")
        .into_inner();
    assert!(r.applied.is_empty() && r.skipped.is_empty());
    let active: types::StateBody = from_json(json!({"state": "active"}));
    client
        .put_promotion_state("P1", &active)
        .await
        .expect("resume");

    // a bearer token travels with every call when the service authenticates (spec `access-control`)
    let (secured, token) = start_secured_server().await;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    let authed = Client::new_with_client(&secured, http);
    let req: types::EvaluateRequest = from_json(json!({"cart": cart(), "customer": customer()}));
    let r = authed.evaluate(&req).await.expect("evaluate with token");
    assert_eq!(r.status(), 200);
    match Client::new(&secured).evaluate(&req).await {
        Err(discount_client::Error::UnexpectedResponse(res)) => assert_eq!(res.status(), 401),
        other => panic!("expected 401 without a token, got {other:?}"),
    }

    // the service serves the very document the client was generated from
    let doc = client.openapi_document().await.expect("openapi document");
    assert_eq!(doc.status(), 200);
}
