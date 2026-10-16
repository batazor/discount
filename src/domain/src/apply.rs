//! `apply` (ADR 4, ADR 6, ADR 9):
//! `validate → sortPromotions → eligible → selectExclusive → selectFirstMatch → fold applyOne`.
//! Mirrors `spec/Discount/Apply.lean` stage by stage.

use crate::allocate::allocate;
use crate::eligibility::{eligible, first_failing_rule};
use crate::rounding::round_div;
use crate::stacking::{lookup, select_exclusive, select_first_match, sort_promotions};
use crate::types::*;
use crate::validate::{convert_amount_i64, validate};

/// Intermediate state of the fold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub line_running: Vec<i64>,
    pub order_cap_remaining: Option<i64>,
    pub applied: Vec<AppliedPromotion>,
}

pub fn effective_rounding(policy: &PricingPolicy, p: &Promotion) -> RoundingPolicy {
    match p.rounding {
        Some(r) => r,
        None => policy.rounding,
    }
}

pub fn raw_discount(
    policy: &PricingPolicy,
    original: i64,
    running: i64,
    p: &Promotion,
) -> Result<i64, PricingError> {
    let base = match policy.stacking {
        StackingPolicy::Sequential => running,
        StackingPolicy::Additive => original,
    };
    match &p.action {
        Action::OrderPercentage { bps } => {
            let num = base.checked_mul(*bps).ok_or(PricingError::Overflow)?;
            Ok(round_div(effective_rounding(policy, p), num, BPS_SCALE))
        }
        Action::OrderFixedAmount { amount } => Ok(amount.amount),
    }
}

/// `min(raw, cap, remaining_budget, remaining_order_cap, running)` and the first bound in
/// that order whose value equals the result (`None` if the raw discount was not reduced).
pub fn clamp_discount(
    raw: i64,
    cap: Option<i64>,
    budget: Option<i64>,
    order_cap_rem: Option<i64>,
    running: i64,
) -> (i64, Option<ClampReason>) {
    let d1 = match cap {
        Some(c) => raw.min(c),
        None => raw,
    };
    let d2 = match budget {
        Some(b) => d1.min(b),
        None => d1,
    };
    let d3 = match order_cap_rem {
        Some(o) => d2.min(o),
        None => d2,
    };
    let d = d3.min(running);
    let reason = if d == raw {
        None
    } else if cap == Some(d) {
        Some(ClampReason::Cap)
    } else if budget == Some(d) {
        Some(ClampReason::Budget)
    } else if order_cap_rem == Some(d) {
        Some(ClampReason::OrderCap)
    } else {
        Some(ClampReason::Running)
    };
    (d, reason)
}

pub fn order_cap_initial(policy: &PricingPolicy, s: i64) -> Result<Option<i64>, PricingError> {
    let pct = |b: i64| -> Result<i64, PricingError> {
        let num = s.checked_mul(b).ok_or(PricingError::Overflow)?;
        Ok(round_div(policy.rounding, num, BPS_SCALE))
    };
    match (&policy.order_cap.amount, policy.order_cap.bps) {
        (None, None) => Ok(None),
        (Some(m), None) => Ok(Some(m.amount)),
        (None, Some(b)) => Ok(Some(pct(b)?)),
        (Some(m), Some(b)) => Ok(Some(m.amount.min(pct(b)?))),
    }
}

/// Pointwise subtraction `a - b` over lists of equal length.
pub fn sub_lists(a: &[i64], b: &[i64]) -> Vec<i64> {
    a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()
}

pub fn apply_one(
    i: &PricingInput,
    original: i64,
    st: &Step,
    p: &Promotion,
) -> Result<Step, PricingError> {
    let running: i64 = st.line_running.iter().sum();
    let raw = raw_discount(&i.policy, original, running, p)?;
    let (d, reason) = clamp_discount(
        raw,
        p.cap.as_ref().map(|m| m.amount),
        p.remaining_budget.as_ref().map(|m| m.amount),
        st.order_cap_remaining,
        running,
    );
    let alloc = allocate(d, &st.line_running);
    let cur = &i.cart.currency;
    let mut applied = st.applied.clone();
    applied.push(AppliedPromotion {
        promotion_id: p.promotion_id.clone(),
        discount: Money::new(d, cur),
        clamped_by: reason,
        allocation: i
            .cart
            .lines
            .iter()
            .zip(alloc.iter())
            .map(|(l, a)| LineAllocation {
                line_id: l.line_id.clone(),
                amount: Money::new(*a, cur),
            })
            .collect(),
    });
    Ok(Step {
        line_running: sub_lists(&st.line_running, &alloc),
        order_cap_remaining: st.order_cap_remaining.map(|o| o - d),
        applied,
    })
}

pub fn skips_of(i: &PricingInput, sorted: &[Promotion]) -> Vec<(PromotionId, SkipReason)> {
    sorted
        .iter()
        .filter_map(|p| {
            first_failing_rule(&p.rules, p, i).map(|k| {
                (
                    p.promotion_id.clone(),
                    SkipReason::NotEligible { rule_index: k },
                )
            })
        })
        .collect()
}

/// Line subtotals in cart order. Valid input only: each fits `i64` (ADR 3).
pub fn subs_of(i: &PricingInput) -> Vec<i64> {
    i.cart
        .lines
        .iter()
        .map(|l| line_subtotal(l) as i64)
        .collect()
}

/// All promotions in the total order.
pub fn sorted_of(i: &PricingInput) -> Vec<Promotion> {
    sort_promotions(&i.groups, &i.promotions)
}

/// Eligible promotions, in the total order.
pub fn eligible_of(i: &PricingInput) -> Vec<Promotion> {
    sorted_of(i)
        .into_iter()
        .filter(|p| eligible(p, i))
        .collect()
}

pub fn after_exclusive(i: &PricingInput) -> (Vec<Promotion>, Vec<(PromotionId, SkipReason)>) {
    select_exclusive(&eligible_of(i))
}

pub fn after_first_match(i: &PricingInput) -> (Vec<Promotion>, Vec<(PromotionId, SkipReason)>) {
    select_first_match(&i.groups, &after_exclusive(i).0, &[])
}

pub fn to_apply_of(i: &PricingInput) -> Vec<Promotion> {
    after_first_match(i).0
}

pub fn init_step(i: &PricingInput) -> Result<Step, PricingError> {
    let subs = subs_of(i);
    let s: i64 = subs.iter().sum();
    Ok(Step {
        line_running: subs,
        order_cap_remaining: order_cap_initial(&i.policy, s)?,
        applied: Vec::new(),
    })
}

pub fn final_step(i: &PricingInput) -> Result<Step, PricingError> {
    let s: i64 = subs_of(i).iter().sum();
    let mut st = init_step(i)?;
    for p in to_apply_of(i) {
        st = apply_one(i, s, &st, &p)?;
    }
    Ok(st)
}

/// The pure computation after validation.
pub fn compute(i: &PricingInput) -> Result<PricingResult, PricingError> {
    let cur = &i.cart.currency;
    let subs = subs_of(i);
    let s: i64 = subs.iter().sum();
    let sorted = sorted_of(i);
    let mut skips = skips_of(i, &sorted);
    skips.extend(after_exclusive(i).1);
    skips.extend(after_first_match(i).1);
    let fin = final_step(i)?;
    let d = s - fin.line_running.iter().sum::<i64>();
    Ok(PricingResult {
        currency: cur.clone(),
        original_subtotal: Money::new(s, cur),
        total_discount: Money::new(d, cur),
        final_total: Money::new(s - d, cur),
        lines: i
            .cart
            .lines
            .iter()
            .zip(subs.iter().zip(fin.line_running.iter()))
            .map(|(l, (sub, lr))| LineResult {
                line_id: l.line_id.clone(),
                subtotal: Money::new(*sub, cur),
                discount: Money::new(sub - lr, cur),
                total: Money::new(*lr, cur),
            })
            .collect(),
        applied: fin.applied,
        skipped: sorted
            .iter()
            .filter_map(|p| {
                lookup(&skips, &p.promotion_id).map(|r| SkippedPromotion {
                    promotion_id: p.promotion_id.clone(),
                    reason: r.clone(),
                })
            })
            .collect(),
    })
}

// ---------------------------------------------------------------- normalize

/// `m` in the cart currency; unchanged when no rate exists (validation guarantees one).
pub fn normalize_money(rates: &[ExchangeRate], cur: &str, m: &Money) -> Money {
    match convert_amount_i64(rates, m.amount, &m.currency, cur) {
        Some(a) => Money::new(a, cur),
        None => m.clone(),
    }
}

pub fn normalize_rule(rates: &[ExchangeRate], cur: &str, r: &Rule) -> Rule {
    match r {
        Rule::OrderTotalRange { min, max } => Rule::OrderTotalRange {
            min: min.as_ref().map(|m| normalize_money(rates, cur, m)),
            max: max.as_ref().map(|m| normalize_money(rates, cur, m)),
        },
        other => other.clone(),
    }
}

pub fn normalize_action(rates: &[ExchangeRate], cur: &str, a: &Action) -> Action {
    match a {
        Action::OrderFixedAmount { amount } => Action::OrderFixedAmount {
            amount: normalize_money(rates, cur, amount),
        },
        other => other.clone(),
    }
}

pub fn normalize_promotion(rates: &[ExchangeRate], cur: &str, p: &Promotion) -> Promotion {
    Promotion {
        cap: p.cap.as_ref().map(|m| normalize_money(rates, cur, m)),
        remaining_budget: p
            .remaining_budget
            .as_ref()
            .map(|m| normalize_money(rates, cur, m)),
        rules: p
            .rules
            .iter()
            .map(|r| normalize_rule(rates, cur, r))
            .collect(),
        action: normalize_action(rates, cur, &p.action),
        ..p.clone()
    }
}

/// Every promotion amount and the order cap in the cart currency. Everything else is
/// unchanged (OpenSpec `multi-currency`).
pub fn normalize(i: &PricingInput) -> PricingInput {
    let cur = i.cart.currency.as_str();
    let rates = &i.exchange_rates;
    PricingInput {
        policy: PricingPolicy {
            order_cap: OrderCap {
                amount: i
                    .policy
                    .order_cap
                    .amount
                    .as_ref()
                    .map(|m| normalize_money(rates, cur, m)),
                ..i.policy.order_cap.clone()
            },
            ..i.policy.clone()
        },
        promotions: i
            .promotions
            .iter()
            .map(|p| normalize_promotion(rates, cur, p))
            .collect(),
        ..i.clone()
    }
}

pub fn apply(i: &PricingInput) -> Result<PricingResult, PricingError> {
    validate(i)?;
    compute(&normalize(i))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn money(a: i64) -> Money {
        Money::new(a, "EUR")
    }

    fn promo(id: &str, group: &str, priority: i64, action: Action) -> Promotion {
        Promotion {
            promotion_id: id.to_string(),
            group_id: group.to_string(),
            priority,
            exclusive: false,
            cap: None,
            remaining_budget: None,
            uses_total: 0,
            rounding: None,
            rules: Vec::new(),
            action,
        }
    }

    fn input(
        policy: PricingPolicy,
        groups: Vec<StackingGroup>,
        promotions: Vec<Promotion>,
    ) -> PricingInput {
        PricingInput {
            now: 1_760_227_200,
            policy,
            customer: CustomerContext::default(),
            cart: CartSnapshot {
                currency: "EUR".to_string(),
                lines: vec![
                    CartLine {
                        line_id: "L1".to_string(),
                        unit_price: money(1999),
                        quantity: 2,
                    },
                    CartLine {
                        line_id: "L2".to_string(),
                        unit_price: money(501),
                        quantity: 1,
                    },
                ],
                tags: vec![],
                attributes: vec![],
                coupon_codes: vec![],
            },
            coupons: vec![],
            exchange_rates: vec![],
            groups,
            promotions,
        }
    }

    fn policy(rounding: RoundingPolicy, stacking: StackingPolicy) -> PricingPolicy {
        PricingPolicy {
            rounding,
            stacking,
            order_cap: OrderCap {
                amount: None,
                bps: None,
            },
        }
    }

    fn group(id: &str, priority: i64, mode: StackingMode) -> StackingGroup {
        StackingGroup {
            group_id: id.to_string(),
            priority,
            mode,
        }
    }

    /// The worked example of the Lean sample run: 10 % then a capped fixed amount, one
    /// promotion not yet active.
    #[test]
    fn sequential_example() {
        let mut p2 = promo(
            "P2",
            "g",
            5,
            Action::OrderFixedAmount { amount: money(500) },
        );
        p2.cap = Some(money(300));
        let mut p3 = promo("P3", "g", 1, Action::OrderPercentage { bps: 5000 });
        p3.rules = vec![Rule::DateWindow {
            starts_at: Some(1_760_300_000),
            ends_at: None,
        }];
        let mut p1 = promo("P1", "g", 10, Action::OrderPercentage { bps: 1000 });
        p1.rules = vec![Rule::DateWindow {
            starts_at: None,
            ends_at: None,
        }];
        let i = input(
            policy(RoundingPolicy::Down, StackingPolicy::Sequential),
            vec![group("g", 0, StackingMode::Stackable)],
            vec![p1, p2, p3],
        );
        let r = apply(&i).unwrap();
        assert_eq!(r.original_subtotal.amount, 4499);
        assert_eq!(r.total_discount.amount, 749);
        assert_eq!(r.final_total.amount, 3750);
        assert_eq!(r.applied.len(), 2);
        assert_eq!(r.applied[0].discount.amount, 449);
        assert_eq!(
            r.applied[0]
                .allocation
                .iter()
                .map(|a| a.amount.amount)
                .collect::<Vec<_>>(),
            vec![399, 50]
        );
        assert_eq!(r.applied[1].discount.amount, 300);
        assert_eq!(r.applied[1].clamped_by, Some(ClampReason::Cap));
        assert_eq!(
            r.applied[1]
                .allocation
                .iter()
                .map(|a| a.amount.amount)
                .collect::<Vec<_>>(),
            vec![267, 33]
        );
        assert_eq!(r.lines[0].total.amount, 3332);
        assert_eq!(r.lines[1].total.amount, 418);
        assert_eq!(
            r.skipped,
            vec![SkippedPromotion {
                promotion_id: "P3".to_string(),
                reason: SkipReason::NotEligible { rule_index: 0 },
            }]
        );
    }

    #[test]
    fn first_match_budget_and_order_cap() {
        let mut pol = policy(RoundingPolicy::HalfUp, StackingPolicy::Additive);
        pol.order_cap.bps = Some(1500);
        let mut p1 = promo("P1", "prod", 0, Action::OrderPercentage { bps: 1000 });
        p1.remaining_budget = Some(money(700));
        p1.rounding = Some(RoundingPolicy::Down);
        let i = PricingInput {
            cart: CartSnapshot {
                currency: "EUR".to_string(),
                lines: vec![CartLine {
                    line_id: "A".to_string(),
                    unit_price: money(10000),
                    quantity: 1,
                }],
                tags: vec![],
                attributes: vec![],
                coupon_codes: vec![],
            },
            ..input(
                pol,
                vec![
                    group("ship", 10, StackingMode::FirstMatch),
                    group("prod", 0, StackingMode::Stackable),
                ],
                vec![
                    promo(
                        "S2",
                        "ship",
                        1,
                        Action::OrderFixedAmount { amount: money(500) },
                    ),
                    promo(
                        "S1",
                        "ship",
                        5,
                        Action::OrderFixedAmount { amount: money(300) },
                    ),
                    p1,
                    promo("P2", "prod", 0, Action::OrderPercentage { bps: 1000 }),
                ],
            )
        };
        let r = apply(&i).unwrap();
        let applied: Vec<(&str, i64, Option<ClampReason>)> = r
            .applied
            .iter()
            .map(|a| (a.promotion_id.as_str(), a.discount.amount, a.clamped_by))
            .collect();
        assert_eq!(
            applied,
            vec![
                ("S1", 300, None),
                ("P1", 700, Some(ClampReason::Budget)),
                ("P2", 500, Some(ClampReason::OrderCap)),
            ]
        );
        assert_eq!(r.total_discount.amount, 1500);
        assert_eq!(
            r.skipped,
            vec![SkippedPromotion {
                promotion_id: "S2".to_string(),
                reason: SkipReason::FirstMatchTaken {
                    winner: "S1".to_string()
                },
            }]
        );
    }

    #[test]
    fn exclusive_beats_higher_priority() {
        let mut x = promo("X", "g", 1, Action::OrderPercentage { bps: 2000 });
        x.exclusive = true;
        let y = promo("Y", "g", 9, Action::OrderPercentage { bps: 5000 });
        let i = input(
            policy(RoundingPolicy::HalfUp, StackingPolicy::Sequential),
            vec![group("g", 0, StackingMode::Stackable)],
            vec![x, y],
        );
        let r = apply(&i).unwrap();
        assert_eq!(r.applied.len(), 1);
        assert_eq!(r.applied[0].promotion_id, "X");
        assert_eq!(
            r.skipped[0].reason,
            SkipReason::ExclusiveApplied {
                winner: "X".to_string()
            }
        );
    }

    #[test]
    fn validation_errors_in_order() {
        let mut i = input(
            policy(RoundingPolicy::HalfUp, StackingPolicy::Sequential),
            vec![],
            vec![promo("X", "nope", 1, Action::OrderPercentage { bps: 2000 })],
        );
        assert_eq!(
            apply(&i),
            Err(PricingError::UnknownGroup { promotion_index: 0 })
        );
        i.cart.lines[1].line_id = "L1".to_string();
        assert_eq!(
            apply(&i),
            Err(PricingError::DuplicateLineId {
                line_id: "L1".to_string()
            })
        );
        i.cart.currency = "eur".to_string();
        assert_eq!(
            apply(&i),
            Err(PricingError::InvalidCurrency {
                field: "cart.currency".to_string()
            })
        );
    }

    /// Multi-currency: a USD fixed amount converted with 92/100, rounded down (OpenSpec scenarios).
    #[test]
    fn foreign_currency_amounts_are_normalized() {
        let usd = |a: i64| Money::new(a, "USD");
        let mut i = input(
            policy(RoundingPolicy::Down, StackingPolicy::Sequential),
            vec![group("g", 0, StackingMode::Stackable)],
            vec![promo(
                "P1",
                "g",
                0,
                Action::OrderFixedAmount { amount: usd(999) },
            )],
        );
        assert_eq!(
            apply(&i),
            Err(PricingError::CurrencyMismatch {
                field: "promotions[0].action.amount".into()
            })
        );
        i.exchange_rates.push(ExchangeRate {
            from: "USD".into(),
            to: "EUR".into(),
            numerator: 92,
            denominator: 100,
        });
        let r = apply(&i).unwrap();
        assert_eq!(r.applied[0].discount.amount, 919);
        i.promotions[0].action = Action::OrderFixedAmount { amount: usd(1000) };
        assert_eq!(apply(&i).unwrap().applied[0].discount.amount, 920);
        i.exchange_rates[0].denominator = 0;
        assert_eq!(
            apply(&i),
            Err(PricingError::InvalidExchangeRate {
                field: "exchange_rates[0].denominator".into()
            })
        );
    }

    /// Coupons and the per-customer rules (OpenSpec scenarios).
    #[test]
    fn coupon_and_customer_rules() {
        let mut p = promo("P1", "g", 0, Action::OrderPercentage { bps: 1000 });
        p.rules = vec![
            Rule::RequiresCoupon,
            Rule::MaxUsesPerCustomer { max: 2 },
            Rule::MaxCompletedOrders { max: 0 },
        ];
        let mut i = input(
            policy(RoundingPolicy::Down, StackingPolicy::Sequential),
            vec![group("g", 0, StackingMode::Stackable)],
            vec![p],
        );
        let skipped = |r: &PricingResult| r.skipped.first().map(|s| s.reason.clone());
        // no coupon presented
        assert_eq!(
            skipped(&apply(&i).unwrap()),
            Some(SkipReason::NotEligible { rule_index: 0 })
        );
        i.coupons.push(Coupon {
            code: "WELCOME".into(),
            promotion_id: "P1".into(),
            starts_at: None,
            ends_at: Some(2_000_000_000),
            customer_id: Some("c1".into()),
            max_uses: Some(1),
            uses: 0,
        });
        i.cart.coupon_codes.push("WELCOME".into());
        // wrong customer
        i.customer.customer_id = Some("c2".into());
        assert_eq!(
            skipped(&apply(&i).unwrap()),
            Some(SkipReason::NotEligible { rule_index: 0 })
        );
        i.customer.customer_id = Some("c1".into());
        assert_eq!(apply(&i).unwrap().applied.len(), 1);
        // usage limit
        i.customer.uses.push(PromotionUse {
            promotion_id: "P1".into(),
            count: 2,
        });
        assert_eq!(
            skipped(&apply(&i).unwrap()),
            Some(SkipReason::NotEligible { rule_index: 1 })
        );
        i.customer.uses[0].count = 1;
        assert_eq!(apply(&i).unwrap().applied.len(), 1);
        // first order only
        i.customer.completed_orders = 1;
        assert_eq!(
            skipped(&apply(&i).unwrap()),
            Some(SkipReason::NotEligible { rule_index: 2 })
        );
        // validation of the context
        i.customer.uses.push(PromotionUse {
            promotion_id: "P1".into(),
            count: 0,
        });
        assert_eq!(
            apply(&i),
            Err(PricingError::InvalidCustomerContext {
                field: "customer.uses[1]".into()
            })
        );
    }

    /// Tags, attributes and the check-in window (OpenSpec scenarios).
    #[test]
    fn tag_and_attribute_rules() {
        let mut p = promo("P1", "g", 0, Action::OrderPercentage { bps: 1000 });
        p.rules = vec![
            Rule::TagsMatch {
                all: vec!["#ios".into()],
                any: vec![],
                none: vec!["#partner".into()],
            },
            Rule::AttributeIn {
                key: "hotel_kind".into(),
                values: vec!["resort".into()],
            },
            Rule::AttributeDateWindow {
                key: "checkin".into(),
                starts_at: Some(100),
                ends_at: Some(200),
            },
            Rule::OrderTotalRange {
                min: Some(money(4000)),
                max: Some(money(5000)),
            },
        ];
        let mut i = input(
            policy(RoundingPolicy::Down, StackingPolicy::Sequential),
            vec![group("g", 0, StackingMode::Stackable)],
            vec![p],
        );
        i.cart.tags = vec!["#ios".into()];
        i.cart.attributes = vec![
            Attribute {
                key: "hotel_kind".into(),
                value: "resort".into(),
            },
            Attribute {
                key: "checkin".into(),
                value: "150".into(),
            },
        ];
        assert_eq!(apply(&i).unwrap().applied.len(), 1);
        i.cart.attributes[1].value = "200".into();
        assert_eq!(
            apply(&i).unwrap().skipped[0].reason,
            SkipReason::NotEligible { rule_index: 2 }
        );
        i.cart.attributes[1].value = "soon".into();
        assert_eq!(
            apply(&i).unwrap().skipped[0].reason,
            SkipReason::NotEligible { rule_index: 2 }
        );
        i.cart.attributes[1].value = "150".into();
        i.cart.tags.push("#partner".into());
        assert_eq!(
            apply(&i).unwrap().skipped[0].reason,
            SkipReason::NotEligible { rule_index: 0 }
        );
        assert_eq!(crate::eligibility::parse_decimal("+5"), None);
        assert_eq!(crate::eligibility::parse_decimal("-7"), Some(-7));
        assert_eq!(
            crate::eligibility::parse_decimal("1234567890123456789"),
            None
        );
    }

    #[test]
    fn empty_cart_prices_to_zero() {
        let mut i = input(
            policy(RoundingPolicy::HalfUp, StackingPolicy::Sequential),
            vec![],
            vec![],
        );
        i.cart.lines.clear();
        let r = apply(&i).unwrap();
        assert_eq!(r.final_total.amount, 0);
        assert!(r.lines.is_empty());
    }
}
