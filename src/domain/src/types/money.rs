//! Money, identifiers and bounds (ADR 3). Mirrors `Discount/Types/Money.lean`.

use serde::{Deserialize, Deserializer, Serialize};

use super::unit_variant;
pub type Currency = String;
pub type LineId = String;
pub type GroupId = String;
pub type PromotionId = String;
pub type BasisPoints = i64;
pub type Timestamp = i64;
pub type Quantity = i64;

/// Upper bound for every amount and for the order subtotal (ADR 3).
pub const MAX_MINOR_UNITS: i64 = 100_000_000_000_000;
/// Upper bound for `quantity`.
pub const MAX_QUANTITY: i64 = 1_000_000_000;
/// Denominator of `BasisPoints`.
pub const BPS_SCALE: i64 = 10_000;
/// Upper bound for `ExchangeRate.numerator` and `denominator`.
pub const MAX_RATE_PART: i64 = 1_000_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Money {
    pub amount: i64,
    pub currency: Currency,
}

impl Money {
    pub fn new(amount: i64, currency: &str) -> Money {
        Money {
            amount,
            currency: currency.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum RoundingPolicy {
    HalfUp,
    HalfEven,
    Down,
}

impl<'de> Deserialize<'de> for RoundingPolicy {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match unit_variant(d)?.as_str() {
            "HalfUp" => Ok(RoundingPolicy::HalfUp),
            "HalfEven" => Ok(RoundingPolicy::HalfEven),
            "Down" => Ok(RoundingPolicy::Down),
            v => Err(serde::de::Error::unknown_variant(
                v,
                &["HalfUp", "HalfEven", "Down"],
            )),
        }
    }
}
