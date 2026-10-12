//! # discount-adapters
//!
//! Implementations of the `discount-app` ports. Axum HTTP and Postgres (sqlx) adapters
//! arrive with the CRUD iteration; the first iteration ships an in-memory repository and a
//! system clock so that `Evaluate` can run end to end.

pub mod memory;

pub use memory::{InMemoryPromotionRepository, NoopEventPublisher, SystemClock};
