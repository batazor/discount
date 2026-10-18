//! Pricing policy and the engine input (ADR 3, ADR 4). Mirrors `Discount/Types/Policy.lean`.

use serde::{Deserialize, Deserializer, Serialize};

use super::cart::CartSnapshot;
use super::customer::{Coupon, CustomerContext};
use super::money::{BasisPoints, Money, RoundingPolicy, Timestamp};
use super::promotion::{ExchangeRate, Promotion, StackingGroup};
use super::unit_variant;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum StackingPolicy {
    Sequential,
    Additive,
}

impl<'de> Deserialize<'de> for StackingPolicy {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match unit_variant(d)?.as_str() {
            "Sequential" => Ok(StackingPolicy::Sequential),
            "Additive" => Ok(StackingPolicy::Additive),
            v => Err(serde::de::Error::unknown_variant(
                v,
                &["Sequential", "Additive"],
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderCap {
    pub amount: Option<Money>,
    pub bps: Option<BasisPoints>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingPolicy {
    pub rounding: RoundingPolicy,
    pub stacking: StackingPolicy,
    pub order_cap: OrderCap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingInput {
    pub now: Timestamp,
    pub policy: PricingPolicy,
    pub customer: CustomerContext,
    pub cart: CartSnapshot,
    pub coupons: Vec<Coupon>,
    pub exchange_rates: Vec<ExchangeRate>,
    pub groups: Vec<StackingGroup>,
    pub promotions: Vec<Promotion>,
}
