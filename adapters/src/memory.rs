use std::time::{SystemTime, UNIX_EPOCH};

use discount_app::ports::{Clock, Event, EventPublisher, PromotionRepository, RepositoryError};
use discount_core::{CartSnapshot, Promotion, StackingGroup, Timestamp};

/// Fixed set of groups and promotions; good for tests and local runs.
pub struct InMemoryPromotionRepository {
    pub groups: Vec<StackingGroup>,
    pub promotions: Vec<Promotion>,
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

pub struct NoopEventPublisher;

impl EventPublisher for NoopEventPublisher {
    fn publish(&self, _event: Event) {}
}
