//! Errors and the pricing result (ADR 6, ADR 9). Mirrors `Discount/Types/Result.lean`.

use serde::{Deserialize, Deserializer, Serialize};

use super::money::{Currency, GroupId, LineId, Money, PromotionId};
use super::unit_variant;
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
