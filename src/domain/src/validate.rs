//! `validate` (ADR 6): sequential checks in the normative order of
//! `docs/ubiquitous-language.md`; the first failure is reported. Mirrors
//! `spec/Discount/Validate.lean`; Lean's `andThen` is `?` here. Money in another currency
//! than the cart's is checked through `convert_amount` (OpenSpec `multi-currency`).

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

#[allow(clippy::manual_range_contains)]
pub fn rate_part_in_range(x: i64) -> bool {
    1 <= x && x <= MAX_RATE_PART
}

/// The rate converting `from` into `to`, if listed.
pub fn find_rate<'a>(rates: &'a [ExchangeRate], from: &str, to: &str) -> Option<&'a ExchangeRate> {
    rates.iter().find(|r| r.from == from && r.to == to)
}

/// `amount` of currency `from` in currency `to`: unchanged for the same currency, else
/// `roundDiv Down (amount * numerator) denominator`; `None` without a rate. The product can
/// reach `10^26`, so it is computed in `i128`; validation keeps the result within `i64`.
pub fn convert_amount(rates: &[ExchangeRate], amount: i64, from: &str, to: &str) -> Option<i128> {
    if from == to {
        return Some(amount as i128);
    }
    let r = find_rate(rates, from, to)?;
    Some(round_div_i128(
        RoundingPolicy::Down,
        (amount as i128) * (r.numerator as i128),
        r.denominator as i128,
    ))
}

/// `round_div` on `i128`, used only by the conversion before validation bounds the result.
/// Same branch structure as the Lean definition.
#[allow(clippy::if_same_then_else)]
pub fn round_div_i128(p: RoundingPolicy, num: i128, den: i128) -> i128 {
    let q = num.div_euclid(den);
    let r = num.rem_euclid(den);
    match p {
        RoundingPolicy::Down => q,
        RoundingPolicy::HalfUp => {
            if 2 * r >= den {
                q + 1
            } else {
                q
            }
        }
        RoundingPolicy::HalfEven => {
            if 2 * r > den {
                q + 1
            } else if 2 * r < den {
                q
            } else if q.rem_euclid(2) == 0 {
                q
            } else {
                q + 1
            }
        }
    }
}

/// `convert_amount` narrowed to `i64`; only valid after `validate` bounded the result.
pub fn convert_amount_i64(
    rates: &[ExchangeRate],
    amount: i64,
    from: &str,
    to: &str,
) -> Option<i64> {
    convert_amount(rates, amount, from, to)
        .map(|c| c.clamp(i64::MIN as i128, i64::MAX as i128) as i64)
}

fn amount_in_range_i128(a: i128) -> bool {
    0 <= a && a <= MAX_MINOR_UNITS as i128
}

/// `InvalidCurrency`, `CurrencyMismatch`, `AmountOutOfRange` (original, then converted).
/// The two range checks are kept separate to mirror the Lean check order.
#[allow(clippy::if_same_then_else)]
pub fn check_money(
    m: &Money,
    cart_currency: &str,
    rates: &[ExchangeRate],
    field: &str,
) -> Result<(), PricingError> {
    if !valid_currency(&m.currency) {
        return Err(PricingError::InvalidCurrency {
            field: field.to_string(),
        });
    }
    match convert_amount(rates, m.amount, &m.currency, cart_currency) {
        None => Err(PricingError::CurrencyMismatch {
            field: field.to_string(),
        }),
        Some(c) => {
            if !amount_in_range(m.amount) {
                Err(PricingError::AmountOutOfRange {
                    field: field.to_string(),
                })
            } else if !amount_in_range_i128(c) {
                Err(PricingError::AmountOutOfRange {
                    field: field.to_string(),
                })
            } else {
                Ok(())
            }
        }
    }
}

pub fn check_opt_money(
    m: &Option<Money>,
    cart_currency: &str,
    rates: &[ExchangeRate],
    field: &str,
) -> Result<(), PricingError> {
    match m {
        None => Ok(()),
        Some(m) => check_money(m, cart_currency, rates, field),
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

/// Index of the second occurrence of a repeated element, if any.
pub fn second_occurrence(ids: &[&str]) -> Option<usize> {
    let mut seen: Vec<&str> = Vec::new();
    for (k, x) in ids.iter().enumerate() {
        if seen.contains(x) {
            return Some(k);
        }
        seen.push(x);
    }
    None
}

pub fn check_customer(c: &CustomerContext) -> Result<(), PricingError> {
    if c.completed_orders < 0 {
        return Err(PricingError::InvalidCustomerContext {
            field: "customer.completed_orders".to_string(),
        });
    }
    for (k, u) in c.uses.iter().enumerate() {
        if u.count < 0 {
            return Err(PricingError::InvalidCustomerContext {
                field: format!("customer.uses[{k}].count"),
            });
        }
    }
    let ids: Vec<&str> = c.uses.iter().map(|u| u.promotion_id.as_str()).collect();
    match second_occurrence(&ids) {
        Some(k) => Err(PricingError::InvalidCustomerContext {
            field: format!("customer.uses[{k}]"),
        }),
        None => Ok(()),
    }
}

pub fn check_rate(r: &ExchangeRate, k: usize) -> Result<(), PricingError> {
    if !valid_currency(&r.from) {
        Err(PricingError::InvalidCurrency {
            field: format!("exchange_rates[{k}].from"),
        })
    } else if !valid_currency(&r.to) {
        Err(PricingError::InvalidCurrency {
            field: format!("exchange_rates[{k}].to"),
        })
    } else if !rate_part_in_range(r.numerator) {
        Err(PricingError::InvalidExchangeRate {
            field: format!("exchange_rates[{k}].numerator"),
        })
    } else if !rate_part_in_range(r.denominator) {
        Err(PricingError::InvalidExchangeRate {
            field: format!("exchange_rates[{k}].denominator"),
        })
    } else {
        Ok(())
    }
}

/// First `(from, to)` pair seen twice.
pub fn first_duplicate_pair(rates: &[ExchangeRate]) -> Option<(Currency, Currency)> {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for r in rates {
        let pair = (r.from.as_str(), r.to.as_str());
        if seen.contains(&pair) {
            return Some((r.from.clone(), r.to.clone()));
        }
        seen.push(pair);
    }
    None
}

pub fn check_rates(rates: &[ExchangeRate]) -> Result<(), PricingError> {
    for (k, r) in rates.iter().enumerate() {
        check_rate(r, k)?;
    }
    match first_duplicate_pair(rates) {
        Some((from, to)) => Err(PricingError::DuplicateExchangeRate { from, to }),
        None => Ok(()),
    }
}

pub fn check_order_cap(
    oc: &OrderCap,
    cart_currency: &str,
    rates: &[ExchangeRate],
) -> Result<(), PricingError> {
    check_opt_money(&oc.amount, cart_currency, rates, "policy.order_cap.amount")?;
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

pub fn check_action(
    a: &Action,
    cart_currency: &str,
    rates: &[ExchangeRate],
    i: usize,
) -> Result<(), PricingError> {
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
            rates,
            &format!("promotions[{i}].action.amount"),
        ),
    }
}

pub fn check_rule(
    r: &Rule,
    cart_currency: &str,
    rates: &[ExchangeRate],
    i: usize,
    k: usize,
) -> Result<(), PricingError> {
    match r {
        Rule::OrderTotalRange { min, max } => {
            check_opt_money(
                min,
                cart_currency,
                rates,
                &format!("promotions[{i}].rules[{k}].min"),
            )?;
            check_opt_money(
                max,
                cart_currency,
                rates,
                &format!("promotions[{i}].rules[{k}].max"),
            )
        }
        Rule::MaxCompletedOrders { max } | Rule::MaxUsesPerCustomer { max } => {
            if *max < 0 {
                Err(PricingError::InvalidRule {
                    field: format!("promotions[{i}].rules[{k}].max"),
                })
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

pub fn check_rules(
    rules: &[Rule],
    cart_currency: &str,
    rates: &[ExchangeRate],
    i: usize,
) -> Result<(), PricingError> {
    for (k, r) in rules.iter().enumerate() {
        check_rule(r, cart_currency, rates, i, k)?;
    }
    Ok(())
}

pub fn check_promotions(
    ps: &[Promotion],
    cur: &str,
    rates: &[ExchangeRate],
    gids: &[&str],
) -> Result<(), PricingError> {
    for (i, p) in ps.iter().enumerate() {
        check_action(&p.action, cur, rates, i)?;
        check_opt_money(&p.cap, cur, rates, &format!("promotions[{i}].cap"))?;
        check_opt_money(
            &p.remaining_budget,
            cur,
            rates,
            &format!("promotions[{i}].remaining_budget"),
        )?;
        check_rules(&p.rules, cur, rates, i)?;
        if !gids.contains(&p.group_id.as_str()) {
            return Err(PricingError::UnknownGroup { promotion_index: i });
        }
    }
    Ok(())
}

pub fn validate(i: &PricingInput) -> Result<(), PricingError> {
    let cur = i.cart.currency.as_str();
    let rates = &i.exchange_rates;
    if !valid_currency(cur) {
        return Err(PricingError::InvalidCurrency {
            field: "cart.currency".to_string(),
        });
    }
    check_lines(&i.cart.lines, cur)?;
    let line_ids: Vec<&str> = i.cart.lines.iter().map(|l| l.line_id.as_str()).collect();
    check_duplicates(&line_ids, |line_id| PricingError::DuplicateLineId {
        line_id,
    })?;
    let cart_codes: Vec<&str> = i.cart.coupon_codes.iter().map(|c| c.as_str()).collect();
    check_duplicates(&cart_codes, |code| PricingError::DuplicateCouponCode {
        code,
    })?;
    if order_subtotal(&i.cart) > MAX_MINOR_UNITS as i128 {
        return Err(PricingError::AmountOutOfRange {
            field: "cart.subtotal".to_string(),
        });
    }
    check_customer(&i.customer)?;
    check_rates(rates)?;
    let codes: Vec<&str> = i.coupons.iter().map(|c| c.code.as_str()).collect();
    check_duplicates(&codes, |code| PricingError::DuplicateCouponCode { code })?;
    check_order_cap(&i.policy.order_cap, cur, rates)?;
    let gids: Vec<&str> = i.groups.iter().map(|g| g.group_id.as_str()).collect();
    check_duplicates(&gids, |group_id| PricingError::DuplicateGroupId {
        group_id,
    })?;
    check_promotions(&i.promotions, cur, rates, &gids)?;
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
