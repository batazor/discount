//! Service configuration from environment variables (spec `evaluate-http-api`,
//! requirement "Configuration").

use discount_core::{OrderCap, PricingPolicy, RoundingPolicy, StackingPolicy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub listen: String,
    pub otlp_endpoint: String,
    pub database_url: Option<String>,
    pub promotions_path: String,
    pub environment: String,
    pub policy: PricingPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    InvalidRounding(String),
    InvalidStacking(String),
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
        }
    }
}

impl std::error::Error for ConfigError {}

pub const DEFAULT_LISTEN: &str = "0.0.0.0:8080";
pub const DEFAULT_OTLP_ENDPOINT: &str = "http://localhost:4317";
pub const DEFAULT_PROMOTIONS: &str = "deploy/promotions.json";
pub const DEFAULT_ENV: &str = "local";

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
        Ok(Config {
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
    fn empty_environment_yields_defaults() {
        let c = Config::from_lookup(|_| None).unwrap();
        assert_eq!(c.listen, "0.0.0.0:8080");
        assert_eq!(c.otlp_endpoint, "http://localhost:4317");
        assert_eq!(c.database_url, None);
        assert_eq!(c.promotions_path, "deploy/promotions.json");
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
            "DISCOUNT_ROUNDING" => Some("HalfEven".into()),
            "DISCOUNT_STACKING" => Some("Additive".into()),
            "DISCOUNT_LISTEN" => Some("127.0.0.1:9000".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(c.policy.rounding, RoundingPolicy::HalfEven);
        assert_eq!(c.policy.stacking, StackingPolicy::Additive);
        assert_eq!(c.listen, "127.0.0.1:9000");
        let e = Config::from_lookup(|k| {
            if k == "DISCOUNT_ROUNDING" {
                Some("Floor".into())
            } else {
                None
            }
        });
        assert_eq!(e, Err(ConfigError::InvalidRounding("Floor".into())));
    }
}
