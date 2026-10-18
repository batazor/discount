//! Authentication and authorization (spec `access-control`, ADR 12): bearer JWTs from
//! configured OIDC issuers, claims mapped to a `Principal`, scopes per route family.
//! Nothing here touches pricing.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use discount_application::ports::{
    ALL_SCOPES, DEFAULT_TENANT, Principal, SCOPE_ADMIN, SCOPE_CATALOG_READ, SCOPE_CATALOG_WRITE,
    SCOPE_CUSTOMERS, SCOPE_PRICING,
};
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::config::OidcConfig;

/// Why a token was rejected; the `detail` of the 401 problem body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    MissingToken,
    MalformedToken(String),
    UnknownIssuer(String),
    UnknownKey,
    InvalidSignature(String),
    Expired,
    NotYetValid,
    WrongAudience,
    /// The token verified but names no tenant and is not an admin.
    NoTenant,
    /// JWKS could not be fetched.
    KeysUnavailable(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::MissingToken => write!(f, "missing bearer token"),
            AuthError::MalformedToken(m) => write!(f, "malformed token: {m}"),
            AuthError::UnknownIssuer(i) => write!(f, "issuer not trusted: {i}"),
            AuthError::UnknownKey => write!(f, "no key for the token's kid"),
            AuthError::InvalidSignature(m) => write!(f, "signature or claims invalid: {m}"),
            AuthError::Expired => write!(f, "token expired"),
            AuthError::NotYetValid => write!(f, "token not yet valid"),
            AuthError::WrongAudience => write!(f, "audience mismatch"),
            AuthError::NoTenant => write!(f, "token names no tenant"),
            AuthError::KeysUnavailable(m) => write!(f, "issuer keys unavailable: {m}"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Where an issuer's keys come from: OIDC discovery over HTTP in production, a fixed set in
/// tests.
#[async_trait]
pub trait KeySource: Send + Sync {
    async fn fetch(&self, issuer: &str) -> Result<JwkSet, AuthError>;
}

/// `GET {issuer}/.well-known/openid-configuration` → `jwks_uri` → `GET jwks_uri`.
pub struct HttpKeySource {
    client: reqwest::Client,
}

impl HttpKeySource {
    pub fn new() -> HttpKeySource {
        HttpKeySource {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
        }
    }
}

impl Default for HttpKeySource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeySource for HttpKeySource {
    async fn fetch(&self, issuer: &str) -> Result<JwkSet, AuthError> {
        let discovery = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        let doc: Value = self
            .client
            .get(&discovery)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| AuthError::KeysUnavailable(e.to_string()))?
            .json()
            .await
            .map_err(|e| AuthError::KeysUnavailable(e.to_string()))?;
        let jwks_uri = doc.get("jwks_uri").and_then(Value::as_str).ok_or_else(|| {
            AuthError::KeysUnavailable("discovery document has no jwks_uri".into())
        })?;
        self.client
            .get(jwks_uri)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| AuthError::KeysUnavailable(e.to_string()))?
            .json::<JwkSet>()
            .await
            .map_err(|e| AuthError::KeysUnavailable(e.to_string()))
    }
}

/// A fixed JWKS per issuer, for tests.
pub struct StaticKeySource(pub HashMap<String, JwkSet>);

#[async_trait]
impl KeySource for StaticKeySource {
    async fn fetch(&self, issuer: &str) -> Result<JwkSet, AuthError> {
        self.0
            .get(issuer)
            .cloned()
            .ok_or_else(|| AuthError::UnknownIssuer(issuer.to_string()))
    }
}

struct CachedKeys {
    set: JwkSet,
    fetched_at: Instant,
}

/// Verifies tokens for the configured issuers; caches each issuer's JWKS and refetches once
/// when a token names an unknown `kid` (key rotation).
pub struct Verifier {
    config: OidcConfig,
    keys: Arc<dyn KeySource>,
    cache: RwLock<HashMap<String, CachedKeys>>,
    /// Keys older than this are refetched before use.
    max_age: Duration,
}

/// Leeway on `exp` and `nbf`, seconds.
pub const LEEWAY_SECONDS: u64 = 60;

impl Verifier {
    pub fn new(config: OidcConfig, keys: Arc<dyn KeySource>) -> Verifier {
        Verifier {
            config,
            keys,
            cache: RwLock::new(HashMap::new()),
            max_age: Duration::from_secs(3600),
        }
    }

    pub fn config(&self) -> &OidcConfig {
        &self.config
    }

    /// Fetch every issuer's keys once, so a misconfigured issuer fails at start-up.
    pub async fn warm_up(&self) -> Result<(), AuthError> {
        for issuer in &self.config.issuers {
            let set = self.keys.fetch(issuer).await?;
            self.cache.write().await.insert(
                issuer.clone(),
                CachedKeys {
                    set,
                    fetched_at: Instant::now(),
                },
            );
        }
        Ok(())
    }

    async fn key_for(
        &self,
        issuer: &str,
        kid: &str,
        force: bool,
    ) -> Result<Option<DecodingKey>, AuthError> {
        let stale = {
            let cache = self.cache.read().await;
            match cache.get(issuer) {
                Some(c) if !force && c.fetched_at.elapsed() < self.max_age => {
                    return Ok(c
                        .set
                        .find(kid)
                        .and_then(|jwk| DecodingKey::from_jwk(jwk).ok()));
                }
                _ => true,
            }
        };
        if stale || force {
            let set = self.keys.fetch(issuer).await?;
            let key = set
                .find(kid)
                .and_then(|jwk| DecodingKey::from_jwk(jwk).ok());
            self.cache.write().await.insert(
                issuer.to_string(),
                CachedKeys {
                    set,
                    fetched_at: Instant::now(),
                },
            );
            return Ok(key);
        }
        Ok(None)
    }

    /// Verify the token and resolve the principal. `x_tenant` is the `X-Tenant` header, used
    /// only by admins.
    pub async fn verify(
        &self,
        token: &str,
        x_tenant: Option<&str>,
    ) -> Result<Principal, AuthError> {
        let header = decode_header(token).map_err(|e| AuthError::MalformedToken(e.to_string()))?;
        let kid = header.kid.clone().unwrap_or_default();
        // The issuer is needed before the signature can be checked: read the payload
        // without verifying, then verify against that issuer's keys.
        let issuer = unverified_issuer(token)?;
        let issuer = issuer.trim_end_matches('/').to_string();
        if !self.config.issuers.contains(&issuer) {
            return Err(AuthError::UnknownIssuer(issuer));
        }
        let key = match self.key_for(&issuer, &kid, false).await? {
            Some(k) => k,
            None => self
                .key_for(&issuer, &kid, true)
                .await?
                .ok_or(AuthError::UnknownKey)?,
        };
        let mut validation = Validation::new(header.alg);
        validation.algorithms = vec![header.alg];
        if !matches!(
            header.alg,
            Algorithm::RS256
                | Algorithm::RS384
                | Algorithm::RS512
                | Algorithm::ES256
                | Algorithm::ES384
                | Algorithm::PS256
                | Algorithm::PS384
                | Algorithm::PS512
                | Algorithm::EdDSA
        ) {
            return Err(AuthError::MalformedToken(format!(
                "algorithm {:?} not accepted",
                header.alg
            )));
        }
        validation.leeway = LEEWAY_SECONDS;
        validation.set_issuer(&[issuer.as_str()]);
        match &self.config.audience {
            Some(aud) => validation.set_audience(&[aud.as_str()]),
            None => validation.validate_aud = false,
        }
        validation.set_required_spec_claims(&["exp", "iss"]);
        let data = decode::<Value>(token, &key, &validation).map_err(|e| {
            use jsonwebtoken::errors::ErrorKind::*;
            match e.kind() {
                ExpiredSignature => AuthError::Expired,
                ImmatureSignature => AuthError::NotYetValid,
                InvalidAudience => AuthError::WrongAudience,
                InvalidIssuer => AuthError::UnknownIssuer(issuer.clone()),
                _ => AuthError::InvalidSignature(e.to_string()),
            }
        })?;
        let claims = data.claims;
        let subject = claims
            .get("sub")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let scopes = scopes_from(claim(&claims, &self.config.scopes_claim));
        let tenant = claim(&claims, &self.config.tenant_claim)
            .and_then(Value::as_str)
            .map(str::to_string);
        let is_admin = scopes.iter().any(|s| s == SCOPE_ADMIN);
        let tenant_id = match (tenant, x_tenant, is_admin) {
            (_, Some(x), true) => x.to_string(),
            (Some(t), _, _) => t,
            (None, _, true) => DEFAULT_TENANT.to_string(),
            (None, _, false) => return Err(AuthError::NoTenant),
        };
        Ok(Principal {
            tenant_id,
            scopes,
            subject,
        })
    }
}

/// The payload's `iss`, read before verification (the issuer selects the keys).
fn unverified_issuer(token: &str) -> Result<String, AuthError> {
    use base64::Engine;
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| AuthError::MalformedToken("not three segments".into()))?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| AuthError::MalformedToken(e.to_string()))?;
    let v: Value =
        serde_json::from_slice(&bytes).map_err(|e| AuthError::MalformedToken(e.to_string()))?;
    v.get("iss")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AuthError::MalformedToken("no iss claim".into()))
}

/// A claim by name or by JSON pointer (`/realm_access/roles`).
pub fn claim<'a>(claims: &'a Value, name: &str) -> Option<&'a Value> {
    if name.starts_with('/') {
        claims.pointer(name)
    } else {
        claims.get(name)
    }
}

/// Scopes from a space-separated string or an array of strings.
pub fn scopes_from(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => s.split_whitespace().map(str::to_string).collect(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// The scope a route family requires; `None` for public routes.
pub fn required_scope(method: &axum::http::Method, path: &str) -> Option<&'static str> {
    use axum::http::Method;
    if path == "/healthz" || path == "/openapi.yaml" || path == "/docs" {
        return None;
    }
    if path.starts_with("/v1/tenants") {
        return Some(SCOPE_ADMIN);
    }
    if path == "/v1/evaluate" || path == "/v1/redeem" || path.starts_with("/v1/redemptions") {
        return Some(SCOPE_PRICING);
    }
    if path.starts_with("/v1/customers") {
        return Some(SCOPE_CUSTOMERS);
    }
    if *method == Method::GET || *method == Method::HEAD {
        Some(SCOPE_CATALOG_READ)
    } else {
        Some(SCOPE_CATALOG_WRITE)
    }
}

/// The principal of unauthenticated local runs (`DISCOUNT_AUTH=off`).
pub fn local_principal() -> Principal {
    Principal {
        tenant_id: DEFAULT_TENANT.to_string(),
        scopes: ALL_SCOPES.iter().map(|s| s.to_string()).collect(),
        subject: "local".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Method;

    #[test]
    fn scope_families() {
        assert_eq!(required_scope(&Method::GET, "/healthz"), None);
        assert_eq!(
            required_scope(&Method::POST, "/v1/evaluate"),
            Some(SCOPE_PRICING)
        );
        assert_eq!(
            required_scope(&Method::GET, "/v1/redemptions/o1"),
            Some(SCOPE_PRICING)
        );
        assert_eq!(
            required_scope(&Method::GET, "/v1/customers/c1/coupons"),
            Some(SCOPE_CUSTOMERS)
        );
        assert_eq!(
            required_scope(&Method::GET, "/v1/promotions"),
            Some(SCOPE_CATALOG_READ)
        );
        assert_eq!(
            required_scope(&Method::PUT, "/v1/promotions/P1"),
            Some(SCOPE_CATALOG_WRITE)
        );
        assert_eq!(
            required_scope(&Method::POST, "/v1/coupons/grant"),
            Some(SCOPE_CATALOG_WRITE)
        );
        assert_eq!(
            required_scope(&Method::GET, "/v1/tenants"),
            Some(SCOPE_ADMIN)
        );
    }

    #[test]
    fn claims_by_name_and_pointer() {
        let c: Value = serde_json::json!({"scope": "a b", "realm_access": {"roles": ["x", "y"]}, "tenant_id": "t"});
        assert_eq!(scopes_from(claim(&c, "scope")), vec!["a", "b"]);
        assert_eq!(
            scopes_from(claim(&c, "/realm_access/roles")),
            vec!["x", "y"]
        );
        assert_eq!(claim(&c, "tenant_id").and_then(Value::as_str), Some("t"));
        assert!(scopes_from(claim(&c, "missing")).is_empty());
    }
}
