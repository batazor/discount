//! In-memory port implementations: a fixed promotion catalogue (optionally loaded from a
//! JSON file in the ubiquitous-language encoding), the system clock, a no-op publisher.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use discount_app::ports::{Clock, Event, EventPublisher, PromotionRepository, RepositoryError};
use discount_core::{CartSnapshot, Promotion, StackingGroup, Timestamp};
use serde::{Deserialize, Serialize};

/// Fixed set of groups and promotions; good for tests and local runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InMemoryPromotionRepository {
    pub groups: Vec<StackingGroup>,
    pub promotions: Vec<Promotion>,
}

impl InMemoryPromotionRepository {
    /// Load `{"groups": [...], "promotions": [...]}` from a JSON file.
    pub fn from_json_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("{}: {e}", path.as_ref().display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.as_ref().display()))
    }
}

impl PromotionRepository for InMemoryPromotionRepository {
    fn snapshot(
        &self,
        _cart: &CartSnapshot,
        _now: Timestamp,
    ) -> Result<(Vec<StackingGroup>, Vec<Promotion>), RepositoryError> {
        Ok((self.groups.clone(), self.promotions.clone()))
    }
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

/// A clock that always answers the same instant; for tests.
pub struct FixedClock(pub Timestamp);

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}

pub struct NoopEventPublisher;

impl EventPublisher for NoopEventPublisher {
    fn publish(&self, _event: Event) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use discount_app::Evaluate;
    use discount_core::*;

    fn eur(a: i64) -> Money {
        Money::new(a, "EUR")
    }

    #[test]
    fn fixture_file_prices_sample_cart() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../deploy/promotions.json");
        let repo = InMemoryPromotionRepository::from_json_file(path).expect("fixture loads");
        assert_eq!(repo.groups.len(), 1);
        assert_eq!(repo.promotions.len(), 3);
        let policy = PricingPolicy {
            rounding: RoundingPolicy::Down,
            stacking: StackingPolicy::Sequential,
            order_cap: OrderCap {
                amount: None,
                bps: None,
            },
        };
        let clock = FixedClock(1_760_227_200);
        let use_case = Evaluate {
            repository: &repo,
            clock: &clock,
            events: &NoopEventPublisher,
            policy,
        };
        let cart = CartSnapshot {
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
        };
        let r = use_case.run(cart, CustomerContext {}).unwrap();
        assert_eq!(r.total_discount.amount, 749);
        assert_eq!(r.final_total.amount, 3750);
    }
}
