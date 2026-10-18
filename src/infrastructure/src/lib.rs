//! # discount-infrastructure
//!
//! Infrastructure layer: implementations of the `discount-application` ports and the
//! service edge. `http` is the axum API, `telemetry` the OpenTelemetry instrumentation,
//! `config` the environment configuration, and `persistence` the repository
//! implementations (Postgres and in-memory). Nothing here is in the verified scope;
//! telemetry only reads the `PricingResult` and never changes pricing.

pub mod config;
pub mod http;
pub mod persistence;
pub mod rates;
pub mod telemetry;

pub use config::Config;
pub use persistence::memory::{
    CatalogFixture, FixedClock, InMemoryStore, NoopEventPublisher, RandomCodes, SequentialCodes,
    SystemClock, default_policy,
};
pub use persistence::postgres::PgStore;
pub use rates::{CannedFetcher, ReqwestFetcher};
