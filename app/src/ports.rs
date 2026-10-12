//! Ports: what the application needs from the outside world (ADR 7).

use discount_core::{CartSnapshot, PricingResult, Promotion, StackingGroup, Timestamp};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    Unavailable(String),
}

/// Snapshot of the promotions and groups relevant to a cart at `now`, with the remaining
/// budgets as of the snapshot.
pub trait PromotionRepository {
    fn snapshot(
        &self,
        cart: &CartSnapshot,
        now: Timestamp,
    ) -> Result<(Vec<StackingGroup>, Vec<Promotion>), RepositoryError>;
}

pub trait Clock {
    fn now(&self) -> Timestamp;
}

/// Domain events emitted after a use case completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Evaluated { result: PricingResult },
}

pub trait EventPublisher {
    fn publish(&self, event: Event);
}
