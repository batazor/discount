//! Spec `access-control`: bearer JWTs from configured issuers, claim mapping, scopes per route
//! family, tenant isolation and administration, over the axum router with a static JWKS.

mod common;

use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::*;
use discount_infrastructure::OidcConfig;
use discount_infrastructure::auth::{StaticKeySource, Verifier};
use discount_infrastructure::http::{AppState, AuthMode, router};
use discount_infrastructure::telemetry::Instruments;
use discount_infrastructure::{
    FixedClock, InMemoryTenants, NoopEventPublisher, SequentialCodes, default_policy,
};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde_json::{Value, json};
use tower::ServiceExt;

const ISSUER: &str = "https://idp.example/realms/discount";
const OTHER_ISSUER: &str = "https://evil.example";
const KEY_PEM: &str = include_str!("fixtures/jwt_test_key.pem");
const JWKS: &str = include_str!("fixtures/jwt_test_jwks.json");

fn oidc(scopes_claim: &str, autocreate: bool) -> OidcConfig {
    OidcConfig {
        issuers: vec![ISSUER.to_string()],
        audience: Some("discount".to_string()),
        tenant_claim: "tenant_id".to_string(),
        scopes_claim: scopes_claim.to_string(),
        autocreate_tenants: autocreate,
    }
}

fn app_with(config: OidcConfig) -> (Router, Arc<AppState>) {
    let mut keys = HashMap::new();
    keys.insert(ISSUER.to_string(), serde_json::from_str(JWKS).unwrap());
    let verifier = Verifier::new(config.clone(), Arc::new(StaticKeySource(keys)));
    let state = Arc::new(AppState {
        tenancy: Arc::new(InMemoryTenants::new(default_policy())),
        auth: AuthMode::Oidc {
            verifier: Arc::new(verifier),
            autocreate_tenants: config.autocreate_tenants,
        },
        clock: Arc::new(FixedClock(NOW)),
        events: Arc::new(NoopEventPublisher),
        codes: Arc::new(SequentialCodes::default()),
        instruments: Instruments::noop(),
    });
    (router(state.clone()), state)
}

/// A token signed with the fixture key. `claims` is merged over the defaults.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// `exp` is checked against the wall clock by the JWT library, so tokens use real time; the
/// service clock stays fixed at `NOW` for everything else.
fn token(claims: Value) -> String {
    let mut body = json!({
        "iss": ISSUER,
        "sub": "svc-storefront",
        "aud": "discount",
        "exp": unix_now() + 3600,
        "iat": unix_now(),
        "tenant_id": "hotels",
        "scope": "discount:pricing discount:customers",
    });
    for (k, v) in claims.as_object().unwrap() {
        if v.is_null() {
            body.as_object_mut().unwrap().remove(k);
        } else {
            body[k] = v.clone();
        }
    }
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("test-key-1".to_string());
    encode(
        &header,
        &body,
        &EncodingKey::from_rsa_pem(KEY_PEM.as_bytes()).unwrap(),
    )
    .unwrap()
}

async fn call(
    app: &Router,
    method: Method,
    path: &str,
    bearer: Option<&str>,
    extra: &[(&str, &str)],
    body: Option<String>,
) -> (StatusCode, Value, axum::http::HeaderMap) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(t) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let res = app
        .clone()
        .oneshot(
            req.body(body.map(Body::from).unwrap_or_else(Body::empty))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    (status, v, headers)
}

fn evaluate_body() -> String {
    json!({"cart": serde_json::to_value(sample_cart()).unwrap(), "customer": customer_json()})
        .to_string()
}

#[tokio::test]
async fn public_routes_need_no_token() {
    let (app, _) = app_with(oidc("scope", true));
    for path in ["/healthz", "/openapi.yaml", "/docs"] {
        let (status, _, _) = call(&app, Method::GET, path, None, &[], None).await;
        assert_eq!(status, StatusCode::OK, "{path}");
    }
}

#[tokio::test]
async fn missing_or_invalid_tokens_are_401_problems() {
    let (app, _) = app_with(oidc("scope", true));
    let (status, body, headers) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        None,
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["status"], 401);
    assert_eq!(body["title"], "Unauthorized");
    assert_eq!(headers.get("www-authenticate").unwrap(), "Bearer");
    assert_eq!(
        headers.get("content-type").unwrap(),
        "application/problem+json"
    );
    // expired
    let (status, body, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&token(json!({"exp": unix_now() - 3600}))),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(
        (status, body["detail"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("token expired"))
    );
    // unknown issuer
    let (status, body, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&token(json!({"iss": OTHER_ISSUER}))),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        body["detail"]
            .as_str()
            .unwrap()
            .contains("issuer not trusted")
    );
    // wrong audience
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&token(json!({"aud": "other"}))),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // tampered signature
    let mut t = token(json!({}));
    t.push('x');
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&t),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // garbage
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some("not.a.jwt"),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn scopes_gate_route_families() {
    let (app, _) = app_with(oidc("scope", true));
    let storefront = token(json!({}));
    // pricing and customers allowed
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&storefront),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(
        &app,
        Method::GET,
        "/v1/customers/c1/coupons",
        Some(&storefront),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // catalogue write forbidden, naming the scope
    let (status, body, _) = call(
        &app,
        Method::PUT,
        "/v1/groups/g",
        Some(&storefront),
        &[],
        Some(json!({"group_id": "g", "priority": 0, "mode": {"type": "Stackable"}}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["detail"], "missing scope discount:catalog.write");
    // catalogue read forbidden too
    let (status, _, _) = call(
        &app,
        Method::GET,
        "/v1/promotions",
        Some(&storefront),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // tenants are admin only
    let (status, _, _) = call(
        &app,
        Method::GET,
        "/v1/tenants",
        Some(&storefront),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // an operator token writes the catalogue
    let operator = token(json!({"scope": "discount:catalog.read discount:catalog.write"}));
    let (status, _, _) = call(
        &app,
        Method::PUT,
        "/v1/groups/g",
        Some(&operator),
        &[],
        Some(json!({"group_id": "g", "priority": 0, "mode": {"type": "Stackable"}}).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&operator),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn keycloak_style_roles_claim() {
    let (app, _) = app_with(oidc("/realm_access/roles", true));
    let t = token(
        json!({"scope": null, "realm_access": {"roles": ["discount:pricing", "offline_access"]}}),
    );
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&t),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(&app, Method::GET, "/v1/promotions", Some(&t), &[], None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn tenants_are_isolated_and_admin_switches_with_x_tenant() {
    let (app, _) = app_with(oidc("scope", true));
    let promo = |id: &str| {
        json!({"promotion_id": id, "group_id": "g", "priority": 0, "exclusive": false, "cap": null,
        "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [],
        "action": {"type": "OrderPercentage", "bps": 1000}})
        .to_string()
    };
    let group = json!({"group_id": "g", "priority": 0, "mode": {"type": "Stackable"}}).to_string();
    let hotels = token(
        json!({"tenant_id": "hotels", "scope": "discount:catalog.write discount:catalog.read discount:pricing"}),
    );
    let cars = token(
        json!({"tenant_id": "cars", "scope": "discount:catalog.write discount:catalog.read discount:pricing"}),
    );
    call(
        &app,
        Method::PUT,
        "/v1/groups/g",
        Some(&hotels),
        &[],
        Some(group.clone()),
    )
    .await;
    call(
        &app,
        Method::PUT,
        "/v1/promotions/P1",
        Some(&hotels),
        &[],
        Some(promo("P1")),
    )
    .await;
    // cars sees nothing of hotels
    let (status, l, _) = call(&app, Method::GET, "/v1/promotions", Some(&cars), &[], None).await;
    assert_eq!((status, l), (StatusCode::OK, json!([])));
    let (status, _, _) = call(
        &app,
        Method::GET,
        "/v1/promotions/P1",
        Some(&cars),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // the same id in cars is a different promotion; evaluations price per tenant
    call(
        &app,
        Method::PUT,
        "/v1/groups/g",
        Some(&cars),
        &[],
        Some(group),
    )
    .await;
    let mut p1_cars: Value = serde_json::from_str(&promo("P1")).unwrap();
    p1_cars["action"] =
        json!({"type": "OrderFixedAmount", "amount": {"amount": 100, "currency": "EUR"}});
    call(
        &app,
        Method::PUT,
        "/v1/promotions/P1",
        Some(&cars),
        &[],
        Some(p1_cars.to_string()),
    )
    .await;
    let (_, ev_h, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&hotels),
        &[],
        Some(evaluate_body()),
    )
    .await;
    let (_, ev_c, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&cars),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(ev_h["total_discount"]["amount"], 449);
    assert_eq!(ev_c["total_discount"]["amount"], 100);
    // admin without a tenant claim acts on the tenant named in X-Tenant
    let admin = token(json!({"tenant_id": null, "scope": "discount:admin"}));
    let (status, l, _) = call(
        &app,
        Method::GET,
        "/v1/promotions",
        Some(&admin),
        &[("x-tenant", "cars")],
        None,
    )
    .await;
    assert_eq!((status, l.as_array().unwrap().len()), (StatusCode::OK, 1));
    assert_eq!(l[0]["action"]["type"], "OrderFixedAmount");
    // tenant administration: both tenants were autocreated
    let (status, ts, _) = call(&app, Method::GET, "/v1/tenants", Some(&admin), &[], None).await;
    let ids: Vec<&str> = ts
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tenant_id"].as_str().unwrap())
        .collect();
    assert_eq!(
        (status, ids),
        (StatusCode::OK, vec!["cars", "default", "hotels"])
    );
    let (status, t, _) = call(
        &app,
        Method::PUT,
        "/v1/tenants/transfers",
        Some(&admin),
        &[],
        Some(json!({"tenant_id": "ignored", "name": "Transfers", "created_at": 0}).to_string()),
    )
    .await;
    assert_eq!(
        (status, t["tenant_id"].as_str(), t["created_at"].as_i64()),
        (StatusCode::OK, Some("transfers"), Some(NOW))
    );
    assert_eq!(
        call(
            &app,
            Method::PUT,
            "/v1/tenants/Bad!",
            Some(&admin),
            &[],
            Some(json!({"tenant_id": "x", "name": "n", "created_at": 0}).to_string())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            Method::DELETE,
            "/v1/tenants/transfers",
            Some(&admin),
            &[],
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
            "/v1/tenants/transfers",
            Some(&admin),
            &[],
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // a tenant with redemptions cannot be deleted
    call(&app, Method::POST, "/v1/redeem", Some(&hotels), &[], Some(json!({"order_id": "o1", "cart": serde_json::to_value(sample_cart()).unwrap(), "customer": customer_json()}).to_string())).await;
    let (status, b, _) = call(
        &app,
        Method::DELETE,
        "/v1/tenants/hotels",
        Some(&admin),
        &[],
        None,
    )
    .await;
    assert_eq!(
        (status, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("TenantHasRedemptions"))
    );
    // a non-admin token without a tenant is refused
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&token(json!({"tenant_id": null}))),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn unknown_tenant_without_autocreation_is_forbidden() {
    let (app, _) = app_with(oidc("scope", false));
    let (status, body, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&token(json!({"tenant_id": "nope"}))),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["detail"], "unknown tenant nope");
    // the admin creates it, then the token works
    let admin = token(json!({"tenant_id": null, "scope": "discount:admin"}));
    call(
        &app,
        Method::PUT,
        "/v1/tenants/nope",
        Some(&admin),
        &[],
        Some(json!({"tenant_id": "nope", "name": "Nope", "created_at": 0}).to_string()),
    )
    .await;
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        Some(&token(json!({"tenant_id": "nope"}))),
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn local_mode_runs_as_default_tenant() {
    let app = router(state());
    let (status, _, _) = call(
        &app,
        Method::POST,
        "/v1/evaluate",
        None,
        &[],
        Some(evaluate_body()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, ts, _) = call(&app, Method::GET, "/v1/tenants", None, &[], None).await;
    assert_eq!(
        (status, ts[0]["tenant_id"].as_str()),
        (StatusCode::OK, Some("default"))
    );
}
