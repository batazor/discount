//! `validate` (ADR 6): sequential checks in the normative order of
//! `docs/ubiquitous-language.md`; the first failure is reported. Mirrors
//! `spec/Discount/Validate.lean`; Lean's `andThen` is `?` here.

use crate::types::*;

pub fn valid_currency(c: &str) -> bool {
    c.chars().count() == 3 && c.chars().all(|ch| ch.is_ascii_uppercase())
}

// The comparisons below are written as in the Lean model on purpose.
#[allow(clippy::manual_range_contains)]
pub fn amount_in_range(a: i64) -> bool {
    0 <= a && a <= MAX_MINOR_UNITS
}

#[allow(clippy::manual_range_contains)]
pub fn bps_in_range(b: BasisPoints) -> bool {
    0 <= b && b <= BPS_SCALE
}

/// `InvalidCurrency`, `CurrencyMismatch`, `AmountOutOfRange` for one `Money` field.
pub fn check_money(m: &Money, cart_currency: &str, field: &str) -> Result<(), PricingError> {
    if !valid_currency(&m.currency) {
        return Err(PricingError::InvalidCurrency {
            field: field.to_string(),
        });
    }
    if m.currency != cart_currency {
        return Err(PricingError::CurrencyMismatch {
            field: field.to_string(),
        });
    }
    if !amount_in_range(m.amount) {
        return Err(PricingError::AmountOutOfRange {
            field: field.to_string(),
        });
    }
    Ok(())
}

pub fn check_opt_money(
    m: &Option<Money>,
    cart_currency: &str,
    field: &str,
) -> Result<(), PricingError> {
    match m {
        None => Ok(()),
        Some(m) => check_money(m, cart_currency, field),
    }
}

pub fn check_lines(lines: &[CartLine], cur: &str) -> Result<(), PricingError> {
    for (i, l) in lines.iter().enumerate() {
        let field = format!("cart.lines[{i}].unit_price");
        if !valid_currency(&l.unit_price.currency) {
            return Err(PricingError::InvalidCurrency { field });
        }
        if l.unit_price.currency != cur {
            return Err(PricingError::MixedCurrencies { line_index: i });
        }
        if !amount_in_range(l.unit_price.amount) {
            return Err(PricingError::AmountOutOfRange { field });
        }
        if !(1 <= l.quantity && l.quantity <= MAX_QUANTITY) {
            return Err(PricingError::InvalidQuantity { line_index: i });
        }
    }
    Ok(())
}

/// First element seen twice, scanning in order.
pub fn first_duplicate(ids: &[&str]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for x in ids {
        if seen.contains(x) {
            return Some(x.to_string());
        }
        seen.push(x);
    }
    None
}

pub fn check_duplicates(ids: &[&str], mk: fn(String) -> PricingError) -> Result<(), PricingError> {
    match first_duplicate(ids) {
        Some(x) => Err(mk(x)),
        None => Ok(()),
    }
}

pub fn check_order_cap(oc: &OrderCap, cart_currency: &str) -> Result<(), PricingError> {
    check_opt_money(&oc.amount, cart_currency, "policy.order_cap.amount")?;
    match oc.bps {
        None => Ok(()),
        Some(b) => {
            if !bps_in_range(b) {
                Err(PricingError::InvalidBasisPoints {
                    field: "policy.order_cap.bps".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }
}

pub fn check_action(a: &Action, cart_currency: &str, i: usize) -> Result<(), PricingError> {
    match a {
        Action::OrderPercentage { bps } => {
            if !bps_in_range(*bps) {
                Err(PricingError::InvalidBasisPoints {
                    field: format!("promotions[{i}].action.bps"),
                })
            } else {
                Ok(())
            }
        }
        Action::OrderFixedAmount { amount } => check_money(
            amount,
            cart_currency,
            &format!("promotions[{i}].action.amount"),
        ),
    }
}

pub fn check_promotions(ps: &[Promotion], cur: &str, gids: &[&str]) -> Result<(), PricingError> {
    for (i, p) in ps.iter().enumerate() {
        check_action(&p.action, cur, i)?;
        check_opt_money(&p.cap, cur, &format!("promotions[{i}].cap"))?;
        check_opt_money(
            &p.remaining_budget,
            cur,
            &format!("promotions[{i}].remaining_budget"),
        )?;
        if !gids.contains(&p.group_id.as_str()) {
            return Err(PricingError::UnknownGroup { promotion_index: i });
        }
    }
    Ok(())
}

pub fn validate(i: &PricingInput) -> Result<(), PricingError> {
    if !valid_currency(&i.cart.currency) {
        return Err(PricingError::InvalidCurrency {
            field: "cart.currency".to_string(),
        });
    }
    check_lines(&i.cart.lines, &i.cart.currency)?;
    let line_ids: Vec<&str> = i.cart.lines.iter().map(|l| l.line_id.as_str()).collect();
    check_duplicates(&line_ids, |line_id| PricingError::DuplicateLineId {
        line_id,
    })?;
    if order_subtotal(&i.cart) > MAX_MINOR_UNITS as i128 {
        return Err(PricingError::AmountOutOfRange {
            field: "cart.subtotal".to_string(),
        });
    }
    check_order_cap(&i.policy.order_cap, &i.cart.currency)?;
    let gids: Vec<&str> = i.groups.iter().map(|g| g.group_id.as_str()).collect();
    check_duplicates(&gids, |group_id| PricingError::DuplicateGroupId {
        group_id,
    })?;
    check_promotions(&i.promotions, &i.cart.currency, &gids)?;
    let pids: Vec<&str> = i
        .promotions
        .iter()
        .map(|p| p.promotion_id.as_str())
        .collect();
    check_duplicates(&pids, |promotion_id| PricingError::DuplicatePromotionId {
        promotion_id,
    })?;
    Ok(())
}
