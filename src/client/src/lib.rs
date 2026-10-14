//! # discount-client
//!
//! Rust client for the discount service, generated at build time from `api/openapi.yaml`
//! (the contract; see ADR 10). The service is checked against the same document, so a
//! change to the API starts in the spec, then the client follows automatically and the
//! server's contract test fails until it is implemented.
//!
//! ```no_run
//! use discount_client::Client;
//! let client = Client::new("http://localhost:8080");
//! ```

include!(concat!(env!("OUT_DIR"), "/codegen.rs"));
