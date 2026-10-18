//! Service configuration from environment variables (spec `evaluate-http-api`,
//! requirement "Configuration").

use discount_domain::{OrderCap, PricingPolicy, RoundingPolicy, StackingPolicy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub listen: String,
    pub otlp_endpoint: String,
    pub database_url: Option<String>,
    pub promotions_path: String,
    pub environment: String,
    pub policy: PricingPolicy,
    pub auth: AuthConfig,
}

/// Authentication settings (spec `access-control`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthConfig {
    /// No authentication: every request runs as `default` with every scope. Local use only.
    Off,
    Oidc(OidcConfig),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcConfig {
    /// Trusted issuer URLs, as they appear in `iss`.
    pub issuers: Vec<String>,
    /// Required `aud` member, if any.
    pub audience: Option<String>,
    /// Claim name or JSON pointer (`/realm_access/tenant`) holding the tenant id.
    pub tenant_claim: String,
    /// Claim name or JSON pointer holding the scopes (space-separated string or array).
    pub scopes_claim: String,
    /// Create a tenant row on first sight of a token naming an unknown tenant.
    pub autocreate_tenants: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    InvalidRounding(String),
    InvalidStacking(String),
    InvalidAuthMode(String),
    MissingIssuers,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidRounding(v) => write!(
                f,
                "DISCOUNT_ROUNDING: unknown policy '{v}' (HalfUp | HalfEven | Down)"
            ),
            ConfigError::InvalidStacking(v) => write!(
                f,
                "DISCOUNT_STACKING: unknown policy '{v}' (Sequential | Additive)"
            ),
            ConfigError::InvalidAuthMode(v) => {
                write!(f, "DISCOUNT_AUTH: unknown mode '{v}' (oidc | off)")
            }
            ConfigError::MissingIssuers => write!(
                f,
                "DISCOUNT_AUTH=oidc needs DISCOUNT_AUTH_ISSUERS (comma-separated issuer URLs); set DISCOUNT_AUTH=off for local use"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

pub const DEFAULT_LISTEN: &str = "0.0.0.0:8080";
pub const DEFAULT_OTLP_ENDPOINT: &str = "http://localhost:4317";
pub const DEFAULT_PROMOTIONS: &str = "ops/promotions.json";
pub const DEFAULT_ENV: &str = "local";
pub const DEFAULT_TENANT_CLAIM: &str = "tenant_id";
pub const DEFAULT_SCOPES_CLAIM: &str = "scope";

impl Config {
    /// Read from the process environment.
    pub fn from_env() -> Result<Config, ConfigError> {
        Config::from_lookup(|k| std::env::var(k).ok())
    }

    /// Read through a lookup function (tests pass a map).
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Config, ConfigError> {
        let rounding = match get("DISCOUNT_ROUNDING").as_deref() {
            None | Some("Down") => RoundingPolicy::Down,
            Some("HalfUp") => RoundingPolicy::HalfUp,
            Some("HalfEven") => RoundingPolicy::HalfEven,
            Some(v) => return Err(ConfigError::InvalidRounding(v.to_string())),
        };
        let stacking = match get("DISCOUNT_STACKING").as_deref() {
            None | Some("Sequential") => StackingPolicy::Sequential,
            Some("Additive") => StackingPolicy::Additive,
            Some(v) => return Err(ConfigError::InvalidStacking(v.to_string())),
        };
        let auth = match get("DISCOUNT_AUTH").as_deref() {
            Some("off") => AuthConfig::Off,
            None | Some("oidc") => {
                let issuers: Vec<String> = get("DISCOUNT_AUTH_ISSUERS")
                    .unwrap_or_default()
                    .split(',')
                    .map(|s| s.trim().trim_end_matches('/').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if issuers.is_empty() {
                    return Err(ConfigError::MissingIssuers);
                }
                AuthConfig::Oidc(OidcConfig {
                    issuers,
                    audience: get("DISCOUNT_AUTH_AUDIENCE").filter(|s| !s.is_empty()),
                    tenant_claim: get("DISCOUNT_AUTH_TENANT_CLAIM")
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| DEFAULT_TENANT_CLAIM.to_string()),
                    scopes_claim: get("DISCOUNT_AUTH_SCOPES_CLAIM")
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| DEFAULT_SCOPES_CLAIM.to_string()),
                    autocreate_tenants: matches!(
                        get("DISCOUNT_AUTH_AUTOCREATE_TENANTS").as_deref(),
                        Some("true") | Some("1")
                    ),
                })
            }
            Some(v) => return Err(ConfigError::InvalidAuthMode(v.to_string())),
        };
        Ok(Config {
            auth,
            listen: get("DISCOUNT_LISTEN").unwrap_or_else(|| DEFAULT_LISTEN.to_string()),
            otlp_endpoint: get("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|| DEFAULT_OTLP_ENDPOINT.to_string()),
            database_url: get("DATABASE_URL").filter(|s| !s.is_empty()),
            promotions_path: get("DISCOUNT_PROMOTIONS")
                .unwrap_or_else(|| DEFAULT_PROMOTIONS.to_string()),
            environment: get("DISCOUNT_ENV").unwrap_or_else(|| DEFAULT_ENV.to_string()),
            policy: PricingPolicy {
                rounding,
                stacking,
                order_cap: OrderCap {
                    amount: None,
                    bps: None,
                },
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_environment_refuses_to_start_without_issuers() {
        assert_eq!(
            Config::from_lookup(|_| None),
            Err(ConfigError::MissingIssuers)
        );
    }

    #[test]
    fn oidc_settings() {
        let c = Config::from_lookup(|k| match k {
            "DISCOUNT_AUTH_ISSUERS" => {
                Some("https://kc.example/realms/a/, https://ory.example".into())
            }
            "DISCOUNT_AUTH_SCOPES_CLAIM" => Some("/realm_access/roles".into()),
            "DISCOUNT_AUTH_AUTOCREATE_TENANTS" => Some("true".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            c.auth,
            AuthConfig::Oidc(OidcConfig {
                issuers: vec![
                    "https://kc.example/realms/a".into(),
                    "https://ory.example".into()
                ],
                audience: None,
                tenant_claim: "tenant_id".into(),
                scopes_claim: "/realm_access/roles".into(),
                autocreate_tenants: true,
            })
        );
        assert_eq!(
            Config::from_lookup(|k| if k == "DISCOUNT_AUTH" {
                Some("basic".into())
            } else {
                None
            }),
            Err(ConfigError::InvalidAuthMode("basic".into()))
        );
    }

    #[test]
    fn empty_environment_yields_defaults() {
        let c = Config::from_lookup(|k| {
            if k == "DISCOUNT_AUTH" {
                Some("off".into())
            } else {
                None
            }
        })
        .unwrap();
        assert_eq!(c.auth, AuthConfig::Off);
        assert_eq!(c.listen, "0.0.0.0:8080");
        assert_eq!(c.otlp_endpoint, "http://localhost:4317");
        assert_eq!(c.database_url, None);
        assert_eq!(c.promotions_path, "ops/promotions.json");
        assert_eq!(c.environment, "local");
        assert_eq!(c.policy.rounding, RoundingPolicy::Down);
        assert_eq!(c.policy.stacking, StackingPolicy::Sequential);
        assert_eq!(
            c.policy.order_cap,
            OrderCap {
                amount: None,
                bps: None
            }
        );
    }

    #[test]
    fn overrides_and_errors() {
        let c = Config::from_lookup(|k| match k {
            "DISCOUNT_AUTH" => Some("off".into()),
            "DISCOUNT_ROUNDING" => Some("HalfEven".into()),
            "DISCOUNT_STACKING" => Some("Additive".into()),
            "DISCOUNT_LISTEN" => Some("127.0.0.1:9000".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(c.policy.rounding, RoundingPolicy::HalfEven);
        assert_eq!(c.policy.stacking, StackingPolicy::Additive);
        assert_eq!(c.listen, "127.0.0.1:9000");
        let e = Config::from_lookup(|k| match k {
            "DISCOUNT_AUTH" => Some("off".into()),
            "DISCOUNT_ROUNDING" => Some("Floor".into()),
            _ => None,
        });
        assert_eq!(e, Err(ConfigError::InvalidRounding("Floor".into())));
    }
}
