//! Cart snapshot: `CartLine`, `Attribute`, `CartSnapshot` and the two subtotals. Mirrors `Discount/Types/Cart.lean`.

use serde::{Deserialize, Serialize};

use super::money::{Currency, LineId, Money, Quantity};
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

/// `unit_price.amount * quantity` (ADR 8). Wide, because it is computed before validation
/// bounds the result; after validation it fits `i64`.
pub fn line_subtotal(l: &CartLine) -> i128 {
    (l.unit_price.amount as i128) * (l.quantity as i128)
}

/// Sum of line subtotals.
pub fn order_subtotal(c: &CartSnapshot) -> i128 {
    c.lines.iter().map(line_subtotal).sum()
}
