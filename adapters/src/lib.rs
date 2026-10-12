//! # discount-adapters
//!
//! Implementations of the `discount-app` ports and the service edge: an axum HTTP API
//! (`http`), OpenTelemetry instrumentation (`telemetry`), configuration from the
//! environment (`config`) and in-memory ports (`memory`). Postgres arrives with the CRUD
//! iteration. Nothing here is in the verified scope; telemetry only reads the
//! `PricingResult` and never changes pricing.

pub mod config;
pub mod http;
pub mod memory;
pub mod telemetry;

pub use config::Config;
pub use memory::{FixedClock, InMemoryPromotionRepository, NoopEventPublisher, SystemClock};
