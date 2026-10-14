//! Repository implementations of the application ports `PromotionCatalog` and
//! `RedemptionRepository`: Postgres (sqlx) for the service, in-memory for tests and
//! database-less runs. Both behave identically; `tests/ports.rs` runs the same scenarios
//! against each.

pub mod memory;
pub mod postgres;
