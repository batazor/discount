//! Promotions, groups, rules and exchange rates (ADR 4). Mirrors `Discount/Types/Promotion.lean`.

use serde::{Deserialize, Deserializer, Serialize};

use super::money::{BasisPoints, Currency, GroupId, Money, PromotionId, RoundingPolicy, Timestamp};
use super::unit_variant;
/// One minor unit of `from` is `numerator / denominator` minor units of `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeRate {
    pub from: Currency,
    pub to: Currency,
    pub numerator: i64,
    pub denominator: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum StackingMode {
    Stackable,
    FirstMatch,
}

impl<'de> Deserialize<'de> for StackingMode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match unit_variant(d)?.as_str() {
            "Stackable" => Ok(StackingMode::Stackable),
            "FirstMatch" => Ok(StackingMode::FirstMatch),
            v => Err(serde::de::Error::unknown_variant(
                v,
                &["Stackable", "FirstMatch"],
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackingGroup {
    pub group_id: GroupId,
    pub priority: i64,
    pub mode: StackingMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Rule {
    /// Holds when `starts_at <= now` (or absent) and `now < ends_at` (or absent).
    DateWindow {
        starts_at: Option<Timestamp>,
        ends_at: Option<Timestamp>,
    },
    /// `min <= orderSubtotal <= max` for the present bounds (cart currency after normalization).
    OrderTotalRange {
        min: Option<Money>,
        max: Option<Money>,
    },
    /// All of `all`, at least one of `any` (if non-empty), none of `none` in `cart.tags`.
    TagsMatch {
        all: Vec<String>,
        any: Vec<String>,
        none: Vec<String>,
    },
    /// Some attribute `key` has a value in `values`.
    AttributeIn { key: String, values: Vec<String> },
    /// Attribute `key` parses as an integer `t` with `starts_at <= t < ends_at`.
    AttributeDateWindow {
        key: String,
        starts_at: Option<Timestamp>,
        ends_at: Option<Timestamp>,
    },
    /// `customer.completed_orders <= max`.
    MaxCompletedOrders { max: i64 },
    /// The customer's use count of this promotion is `< max`.
    MaxUsesPerCustomer { max: i64 },
    /// The promotion's `uses_total` is `< max`.
    MaxUsesTotal { max: i64 },
    /// A presented, valid coupon of this promotion exists.
    RequiresCoupon,
    /// Some customer attribute `key` has a value in `values`.
    CustomerAttributeIn { key: String, values: Vec<String> },
    /// Customer attribute `key` parses as an integer `t` with `min <= t <= max` for the present bounds.
    CustomerAttributeRange {
        key: String,
        min: Option<i64>,
        max: Option<i64>,
    },
    /// This promotion's id is in `customer.allowlisted`.
    RequiresAllowlist,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Action {
    OrderPercentage { bps: BasisPoints },
    OrderFixedAmount { amount: Money },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Promotion {
    pub promotion_id: PromotionId,
    pub group_id: GroupId,
    pub priority: i64,
    pub exclusive: bool,
    pub cap: Option<Money>,
    pub remaining_budget: Option<Money>,
    /// Snapshot counter: reserved or confirmed redemptions that applied the promotion.
    pub uses_total: i64,
    pub rounding: Option<RoundingPolicy>,
    pub rules: Vec<Rule>,
    pub action: Action,
}
