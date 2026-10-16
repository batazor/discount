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

/// A string key and value describing the order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attribute {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CartSnapshot {
    pub currency: Currency,
    pub lines: Vec<CartLine>,
    pub tags: Vec<String>,
    pub attributes: Vec<Attribute>,
    pub coupon_codes: Vec<String>,
}

/// How many times the customer already used a promotion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionUse {
    pub promotion_id: PromotionId,
    pub count: i64,
}

/// What eligibility rules may know about the customer (ADR 7).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomerContext {
    pub customer_id: Option<String>,
    pub completed_orders: i64,
    pub uses: Vec<PromotionUse>,
}

/// A code that turns a promotion on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coupon {
    pub code: String,
    pub promotion_id: PromotionId,
    pub starts_at: Option<Timestamp>,
    pub ends_at: Option<Timestamp>,
    pub customer_id: Option<String>,
    /// Absent = unlimited, `1` = single use.
    pub max_uses: Option<i64>,
    /// Snapshot counter: reserved or confirmed redemptions that used the code.
    pub uses: i64,
}

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
    DuplicateCouponCode {
        code: String,
    },
    InvalidCustomerContext {
        field: String,
    },
    InvalidExchangeRate {
        field: String,
    },
    DuplicateExchangeRate {
        from: Currency,
        to: Currency,
    },
    InvalidRule {
        field: String,
    },
    InvalidCoupon {
        field: String,
    },
    InvalidPromotion {
        field: String,
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
