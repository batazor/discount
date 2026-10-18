//! Customer context and coupons (ADR 7, OpenSpec `coupons`). Mirrors `Discount/Types/Customer.lean`.

use serde::{Deserialize, Serialize};

use super::cart::Attribute;
use super::money::{PromotionId, Timestamp};
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
    /// Caller-supplied facts about the customer (`tier = "gold"`, `completed_app_orders = "0"`).
    pub attributes: Vec<Attribute>,
    /// Promotions whose allowlist names this customer; filled by the service like `uses`.
    pub allowlisted: Vec<PromotionId>,
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
