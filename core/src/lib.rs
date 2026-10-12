//! # discount-core
//!
//! The pure pricing engine. No IO, no async, no clock, no randomness (ADR 7). Every
//! module mirrors a module of the Lean specification in `spec/Discount/`, function by
//! function, with the names of `docs/ubiquitous-language.md`. The Lean model is the source
//! of truth (ADR 2); this crate is checked against it by differential random testing in
//! `drt/`.
//!
//! Arithmetic: the Lean model works on unbounded `Int`. Here amounts are `i64`, the
//! products `D * s_i` of the allocation and the line subtotals before validation are
//! computed in `i128`, and the two products that stay in `i64` after validation use
//! checked multiplication mapped to [`PricingError::Overflow`] (ADR 3). Division and
//! remainder use `div_euclid` / `rem_euclid` to match Lean's `/` and `%` on `Int`.

pub mod allocate;
pub mod apply;
pub mod eligibility;
pub mod rounding;
pub mod stacking;
pub mod types;
pub mod validate;

pub use allocate::allocate;
pub use apply::{apply, compute};
pub use rounding::round_div;
pub use types::*;
pub use validate::validate;
