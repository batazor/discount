//! # discount-app
//!
//! Use cases and ports (hexagonal architecture). The domain is `discount-core`; adapters
//! implement the ports in `discount-adapters`. Nothing here is in the verified scope:
//! `Evaluate` only assembles a `PricingInput` and calls `discount_core::apply`.

pub mod evaluate;
pub mod ports;

pub use evaluate::{Evaluate, EvaluateError};
pub use ports::{Clock, EventPublisher, PromotionRepository, RepositoryError};
