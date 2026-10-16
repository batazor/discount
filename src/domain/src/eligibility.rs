//! `eligible` (ADR 7, OpenSpec `eligibility-rules`, `coupons`). Mirrors
//! `spec/Discount/Eligibility.lean`. Rules read `(now, cart, customer, coupons)` and the
//! promotion; amounts are already in the cart currency (`normalize` ran before).

use crate::types::*;

/// Half-open window `[starts_at, ends_at)` on `t`, each bound optional.
pub fn in_window(starts_at: Option<Timestamp>, ends_at: Option<Timestamp>, t: i64) -> bool {
    let after_start = match starts_at {
        None => true,
        Some(s) => s <= t,
    };
    let before_end = match ends_at {
        None => true,
        Some(e) => t < e,
    };
    after_start && before_end
}

/// Optional leading `-`, then 1 to 18 ASCII digits. Anything else is `None`. Same as Lean.
pub fn parse_decimal(s: &str) -> Option<i64> {
    let (neg, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    if digits.is_empty()
        || digits.chars().count() > 18
        || !digits.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let n: i64 = digits
        .chars()
        .fold(0i64, |acc, c| acc * 10 + (c as i64 - '0' as i64));
    Some(if neg { -n } else { n })
}

pub fn uses_of(c: &CustomerContext, pid: &str) -> i64 {
    match c.uses.iter().find(|u| u.promotion_id == pid) {
        Some(u) => u.count,
        None => 0,
    }
}

/// Remaining uses: `max_uses` absent, or `uses < max_uses`.
pub fn coupon_has_uses(c: &Coupon) -> bool {
    match c.max_uses {
        None => true,
        Some(m) => c.uses < m,
    }
}

pub fn coupon_valid(i: &PricingInput, pid: &str, c: &Coupon) -> bool {
    c.promotion_id == pid
        && i.cart.coupon_codes.iter().any(|code| code == &c.code)
        && in_window(c.starts_at, c.ends_at, i.now)
        && match &c.customer_id {
            None => true,
            Some(cid) => i.customer.customer_id.as_deref() == Some(cid.as_str()),
        }
        && coupon_has_uses(c)
}

pub fn rule_holds(r: &Rule, p: &Promotion, i: &PricingInput) -> bool {
    match r {
        Rule::DateWindow { starts_at, ends_at } => in_window(*starts_at, *ends_at, i.now),
        Rule::OrderTotalRange { min, max } => {
            // Valid input: the subtotal fits i64 and the bounds are normalized.
            let s = order_subtotal(&i.cart) as i64;
            let lo = match min {
                None => true,
                Some(m) => m.amount <= s,
            };
            let hi = match max {
                None => true,
                Some(m) => s <= m.amount,
            };
            lo && hi
        }
        Rule::TagsMatch { all, any, none } => {
            let has = |t: &String| i.cart.tags.iter().any(|x| x == t);
            all.iter().all(has)
                && (any.is_empty() || any.iter().any(has))
                && none.iter().all(|t| !has(t))
        }
        Rule::AttributeIn { key, values } => i
            .cart
            .attributes
            .iter()
            .any(|a| a.key == *key && values.iter().any(|v| v == &a.value)),
        Rule::AttributeDateWindow {
            key,
            starts_at,
            ends_at,
        } => match i.cart.attributes.iter().find(|a| a.key == *key) {
            None => false,
            Some(a) => match parse_decimal(&a.value) {
                None => false,
                Some(t) => in_window(*starts_at, *ends_at, t),
            },
        },
        Rule::MaxCompletedOrders { max } => i.customer.completed_orders <= *max,
        Rule::MaxUsesPerCustomer { max } => uses_of(&i.customer, &p.promotion_id) < *max,
        Rule::MaxUsesTotal { max } => p.uses_total < *max,
        Rule::RequiresCoupon => i
            .coupons
            .iter()
            .any(|c| coupon_valid(i, &p.promotion_id, c)),
    }
}

/// Index of the first rule that does not hold, for `SkipReason::NotEligible`.
pub fn first_failing_rule(rules: &[Rule], p: &Promotion, i: &PricingInput) -> Option<usize> {
    for (k, r) in rules.iter().enumerate() {
        if !rule_holds(r, p, i) {
            return Some(k);
        }
    }
    None
}

pub fn eligible(p: &Promotion, i: &PricingInput) -> bool {
    first_failing_rule(&p.rules, p, i).is_none()
}
