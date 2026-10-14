//! # discount-application
//!
//! Use cases and ports (hexagonal architecture). The domain is `discount-domain`; adapters
//! implement the ports in `discount-infrastructure`. Nothing here is in the verified scope:
//! every use case prices with `discount_domain::apply` and only adds bookkeeping.

pub mod evaluate;
pub mod ports;

pub use evaluate::{
    CancelRedemption, ConfirmRedemption, Evaluate, EvaluateError, Redeem, pricing_input,
};
pub use ports::{
    CatalogSnapshot, Clock, Event, EventPublisher, PricingFn, PromotionCatalog, Redemption,
    RedemptionError, RedemptionRepository, RedemptionStatus, ReserveOutcome, StorageError,
    transition_allowed,
};
