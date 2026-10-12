//! `Evaluate`: the pure dry run (ADR 4). `Redeem`, which commits budgets and usage, is
//! iteration 2.

use discount_core::{
    CartSnapshot, CustomerContext, PricingError, PricingInput, PricingPolicy, PricingResult, apply,
};

use crate::ports::{Clock, Event, EventPublisher, PromotionRepository, RepositoryError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluateError {
    Repository(RepositoryError),
    Pricing(PricingError),
}

pub struct Evaluate<'a, R: PromotionRepository, C: Clock, E: EventPublisher> {
    pub repository: &'a R,
    pub clock: &'a C,
    pub events: &'a E,
    pub policy: PricingPolicy,
}

impl<R: PromotionRepository, C: Clock, E: EventPublisher> Evaluate<'_, R, C, E> {
    pub fn run(
        &self,
        cart: CartSnapshot,
        customer: CustomerContext,
    ) -> Result<PricingResult, EvaluateError> {
        let now = self.clock.now();
        let (groups, promotions) = self
            .repository
            .snapshot(&cart, now)
            .map_err(EvaluateError::Repository)?;
        let input = PricingInput {
            now,
            policy: self.policy.clone(),
            customer,
            cart,
            groups,
            promotions,
        };
        let result = apply(&input).map_err(EvaluateError::Pricing)?;
        self.events.publish(Event::Evaluated {
            result: result.clone(),
        });
        Ok(result)
    }
}
