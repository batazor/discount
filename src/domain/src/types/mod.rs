//! Domain types. Names follow `docs/ubiquitous-language.md` exactly (ADR 8); the JSON
//! shape is the documented contract: enums as `{"type": "<Variant>", ...}`, optional
//! fields as `null`, unknown keys rejected. One module per file of the Lean model's
//! `Discount/Types/`; this module re-exports them all.

use serde::{Deserialize, Deserializer};

pub mod cart;
pub mod customer;
pub mod money;
pub mod policy;
pub mod promotion;
pub mod result;

pub use cart::*;
pub use customer::*;
pub use money::*;
pub use policy::*;
pub use promotion::*;
pub use result::*;

/// `{"type": "<Variant>"}` with no other keys. serde's derive does not enforce
/// `deny_unknown_fields` on unit variants of internally tagged enums, so the enums with
/// only unit variants read their tag through this strict shape.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UnitVariant {
    #[serde(rename = "type")]
    variant: String,
}

pub(super) fn unit_variant<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    UnitVariant::deserialize(d).map(|u| u.variant)
}
