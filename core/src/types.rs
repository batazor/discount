//! Domain types. Names follow `docs/ubiquitous-language.md` exactly (ADR 8); the JSON
//! shape is the documented contract: enums as `{"type": "<Variant>", ...}`, optional
//! fields as `null`, unknown keys rejected.

use serde::{Deserialize, Deserializer, Serialize};

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

/// `{"type": "<Variant>"}` with no other keys. serde's derive does not enforce
/// `deny_unknown_fields` on unit variants of internally tagged enums, so the enums with
/// only unit variants read their tag through this strict shape.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitVariant {
    #[serde(rename = "type")]
    variant: String,
}

fn unit_variant<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    UnitVariant::deserialize(d).map(|u| u.variant)
}

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
pub struct CartLine {
    pub line_id: LineId,
    pub unit_price: Money,
    pub quantity: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CartSnapshot {
    pub currency: Currency,
    pub lines: Vec<CartLine>,
}

/// Empty in the first iteration; exists so the input shape is stable (ADR 7).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomerContext {}

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
    pub rounding: Option<RoundingPolicy>,
    pub rules: Vec<Rule>,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingInput {
    pub now: Timestamp,
    pub policy: PricingPolicy,
    pub customer: CustomerContext,
    pub cart: CartSnapshot,
    pub groups: Vec<StackingGroup>,
    pub promotions: Vec<Promotion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum PricingError {
    InvalidCurrency {
        field: String,
    },
    MixedCurrencies {
        line_index: usize,
    },
    CurrencyMismatch {
        field: String,
    },
    AmountOutOfRange {
        field: String,
    },
    InvalidBasisPoints {
        field: String,
    },
    InvalidQuantity {
        line_index: usize,
    },
    DuplicateLineId {
        line_id: LineId,
    },
    DuplicateGroupId {
        group_id: GroupId,
    },
    DuplicatePromotionId {
        promotion_id: PromotionId,
    },
    UnknownGroup {
        promotion_index: usize,
    },
    /// Rust only; the Lean model never produces it (ADR 3). Unreachable for validated input.
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum ClampReason {
    Cap,
    Budget,
    OrderCap,
    Running,
}

impl<'de> Deserialize<'de> for ClampReason {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match unit_variant(d)?.as_str() {
            "Cap" => Ok(ClampReason::Cap),
            "Budget" => Ok(ClampReason::Budget),
            "OrderCap" => Ok(ClampReason::OrderCap),
            "Running" => Ok(ClampReason::Running),
            v => Err(serde::de::Error::unknown_variant(
                v,
                &["Cap", "Budget", "OrderCap", "Running"],
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum SkipReason {
    NotEligible { rule_index: usize },
    ExclusiveApplied { winner: PromotionId },
    FirstMatchTaken { winner: PromotionId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkippedPromotion {
    pub promotion_id: PromotionId,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineAllocation {
    pub line_id: LineId,
    pub amount: Money,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedPromotion {
    pub promotion_id: PromotionId,
    pub discount: Money,
    pub clamped_by: Option<ClampReason>,
    pub allocation: Vec<LineAllocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineResult {
    pub line_id: LineId,
    pub subtotal: Money,
    pub discount: Money,
    pub total: Money,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingResult {
    pub currency: Currency,
    pub original_subtotal: Money,
    pub total_discount: Money,
    pub final_total: Money,
    pub lines: Vec<LineResult>,
    pub applied: Vec<AppliedPromotion>,
    pub skipped: Vec<SkippedPromotion>,
}

/// `unit_price.amount * quantity` (ADR 8). Wide, because it is computed before validation
/// bounds the result; after validation it fits `i64`.
pub fn line_subtotal(l: &CartLine) -> i128 {
    (l.unit_price.amount as i128) * (l.quantity as i128)
}

/// Sum of line subtotals.
pub fn order_subtotal(c: &CartSnapshot) -> i128 {
    c.lines.iter().map(line_subtotal).sum()
}
