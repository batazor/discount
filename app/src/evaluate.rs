//! Use cases. `Evaluate` is the pure dry run (ADR 4); `Redeem`, `ConfirmRedemption` and
//! `CancelRedemption` are the lifecycle of spec `redemption`. None of them is in the
//! verified scope: pricing is always `discount_core::apply`.

use discount_core::{
    CartSnapshot, CustomerContext, PricingError, PricingInput, PricingResult, PromotionUse, apply,
};

use crate::ports::{
    CatalogSnapshot, Clock, Event, EventPublisher, PromotionCatalog, Redemption, RedemptionError,
    RedemptionRepository, RedemptionStatus, ReserveOutcome, StorageError, Uses,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluateError {
    Storage(StorageError),
    Pricing(PricingError),
}

impl From<RedemptionError> for EvaluateError {
    fn from(e: RedemptionError) -> Self {
        match e {
            RedemptionError::Storage(s) => EvaluateError::Storage(s),
            RedemptionError::Pricing(p) => EvaluateError::Pricing(p),
            other => EvaluateError::Storage(StorageError::Unavailable(format!("{other:?}"))),
        }
    }
}

/// Add the stored use counts to the caller-supplied ones (spec `evaluate-http-api`).
pub fn merge_uses(mut customer: CustomerContext, stored: Uses) -> CustomerContext {
    for (pid, count) in stored {
        match customer.uses.iter_mut().find(|u| u.promotion_id == pid) {
            Some(u) => u.count += count,
            None => customer.uses.push(PromotionUse {
                promotion_id: pid,
                count,
            }),
        }
    }
    customer
}

/// Build the engine input from a catalogue snapshot: coupons narrowed to the presented
/// codes, stored uses merged into the customer context.
pub fn pricing_input(
    snapshot: CatalogSnapshot,
    now: i64,
    cart: CartSnapshot,
    customer: CustomerContext,
    stored_uses: Uses,
) -> PricingInput {
    let coupons = snapshot
        .coupons
        .into_iter()
        .filter(|c| cart.coupon_codes.iter().any(|code| code == &c.code))
        .collect();
    PricingInput {
        now,
        policy: snapshot.policy,
        customer: merge_uses(customer, stored_uses),
        cart,
        coupons,
        exchange_rates: snapshot.exchange_rates,
        groups: snapshot.groups,
        promotions: snapshot.promotions,
    }
}

async fn stored_uses(
    redemptions: &dyn RedemptionRepository,
    customer: &CustomerContext,
) -> Result<Uses, RedemptionError> {
    match &customer.customer_id {
        Some(cid) => redemptions.uses_for_customer(cid).await,
        None => Ok(Vec::new()),
    }
}

pub struct Evaluate<'a> {
    pub catalog: &'a dyn PromotionCatalog,
    pub redemptions: &'a dyn RedemptionRepository,
    pub clock: &'a dyn Clock,
    pub events: &'a dyn EventPublisher,
}

impl Evaluate<'_> {
    pub async fn run(
        &self,
        cart: CartSnapshot,
        customer: CustomerContext,
    ) -> Result<PricingResult, EvaluateError> {
        let now = self.clock.now();
        let snapshot = self
            .catalog
            .snapshot()
            .await
            .map_err(EvaluateError::Storage)?;
        let uses = stored_uses(self.redemptions, &customer).await?;
        let input = pricing_input(snapshot, now, cart, customer, uses);
        let result = apply(&input).map_err(EvaluateError::Pricing)?;
        self.events
            .publish(Event::Evaluated {
                result: result.clone(),
            })
            .await;
        Ok(result)
    }
}

pub struct Redeem<'a> {
    pub redemptions: &'a dyn RedemptionRepository,
    pub clock: &'a dyn Clock,
    pub events: &'a dyn EventPublisher,
}

impl Redeem<'_> {
    pub async fn run(
        &self,
        order_id: &str,
        cart: CartSnapshot,
        customer: CustomerContext,
    ) -> Result<ReserveOutcome, RedemptionError> {
        let now = self.clock.now();
        let customer_id = customer.customer_id.clone();
        let price = move |snapshot: CatalogSnapshot, uses: Uses| {
            let input = pricing_input(snapshot, now, cart.clone(), customer.clone(), uses);
            let result = apply(&input)?;
            Ok((input, result))
        };
        let outcome = self
            .redemptions
            .reserve(order_id, customer_id.as_deref(), now, &price)
            .await?;
        if let ReserveOutcome::Reserved(r) = &outcome {
            self.events
                .publish(Event::Redeemed {
                    order_id: r.order_id.clone(),
                    result: r.result.clone(),
                })
                .await;
        }
        Ok(outcome)
    }
}

pub struct ConfirmRedemption<'a> {
    pub redemptions: &'a dyn RedemptionRepository,
    pub clock: &'a dyn Clock,
    pub events: &'a dyn EventPublisher,
}

impl ConfirmRedemption<'_> {
    pub async fn run(&self, order_id: &str) -> Result<Redemption, RedemptionError> {
        let r = self
            .redemptions
            .transition(order_id, RedemptionStatus::Confirmed, self.clock.now())
            .await?;
        self.events
            .publish(Event::Confirmed {
                order_id: r.order_id.clone(),
            })
            .await;
        Ok(r)
    }
}

pub struct CancelRedemption<'a> {
    pub redemptions: &'a dyn RedemptionRepository,
    pub clock: &'a dyn Clock,
    pub events: &'a dyn EventPublisher,
}

impl CancelRedemption<'_> {
    pub async fn run(&self, order_id: &str) -> Result<Redemption, RedemptionError> {
        let r = self
            .redemptions
            .transition(order_id, RedemptionStatus::Cancelled, self.clock.now())
            .await?;
        self.events
            .publish(Event::Cancelled {
                order_id: r.order_id.clone(),
            })
            .await;
        Ok(r)
    }
}
