//! proptest strategies for `PricingInput` (ADR 2, ADR 3). The valid generator covers the
//! whole validated domain of `docs/ubiquitous-language.md`; `mutated` applies one random
//! violation so that the validation order (ADR 6) is compared too.

use discount_core::*;
use proptest::prelude::*;

pub const NOW: Timestamp = 1_000;

const CURRENCIES: [&str; 3] = ["EUR", "USD", "RUB"];

fn other_currency(c: &str) -> &'static str {
    if c == "EUR" { "USD" } else { "EUR" }
}

pub fn currency() -> impl Strategy<Value = Currency> {
    prop::sample::select(CURRENCIES.to_vec()).prop_map(|c| c.to_string())
}

/// Amounts: mostly small, sometimes mid-range, occasionally near the bound.
pub fn small_amount() -> impl Strategy<Value = i64> {
    prop_oneof![
        6 => 0..10_000i64,
        2 => 0..1_000_000_000i64,
        1 => Just(0i64),
    ]
}

pub fn rounding_policy() -> impl Strategy<Value = RoundingPolicy> {
    prop_oneof![
        Just(RoundingPolicy::HalfUp),
        Just(RoundingPolicy::HalfEven),
        Just(RoundingPolicy::Down)
    ]
}

pub fn stacking_policy() -> impl Strategy<Value = StackingPolicy> {
    prop_oneof![
        Just(StackingPolicy::Sequential),
        Just(StackingPolicy::Additive)
    ]
}

pub fn stacking_mode() -> impl Strategy<Value = StackingMode> {
    prop_oneof![
        Just(StackingMode::Stackable),
        Just(StackingMode::FirstMatch)
    ]
}

pub fn money(cur: Currency) -> impl Strategy<Value = Money> {
    small_amount().prop_map(move |a| Money {
        amount: a,
        currency: cur.clone(),
    })
}

pub fn bps() -> impl Strategy<Value = BasisPoints> {
    prop_oneof![
        5 => 0..=BPS_SCALE,
        1 => Just(0i64),
        1 => Just(BPS_SCALE),
        1 => Just(5_000i64),
    ]
}

pub fn order_cap(cur: Currency) -> impl Strategy<Value = OrderCap> {
    (
        prop::option::weighted(0.3, money(cur)),
        prop::option::weighted(0.3, bps()),
    )
        .prop_map(|(amount, bps)| OrderCap { amount, bps })
}

pub fn policy(cur: Currency) -> impl Strategy<Value = PricingPolicy> {
    (rounding_policy(), stacking_policy(), order_cap(cur)).prop_map(
        |(rounding, stacking, order_cap)| PricingPolicy {
            rounding,
            stacking,
            order_cap,
        },
    )
}

/// One line. Large unit prices come with quantity 1 so that six lines stay within
/// `MAX_MINOR_UNITS`; the subtotal bound is still exercised by `mutated`.
pub fn line(idx: usize, cur: Currency) -> impl Strategy<Value = CartLine> {
    prop_oneof![
        6 => (0..10_000i64, 1..=5i64),
        2 => (0..1_000_000_000i64, 1..=10_000i64),
        1 => (1_000_000_000_000i64..=10_000_000_000_000i64, Just(1i64)),
        1 => (Just(0i64), 1..=3i64),
    ]
    .prop_map(move |(amount, quantity)| CartLine {
        line_id: format!("L{idx}"),
        unit_price: Money {
            amount,
            currency: cur.clone(),
        },
        quantity,
    })
}

pub fn cart(cur: Currency) -> impl Strategy<Value = CartSnapshot> {
    (0usize..=6).prop_flat_map(move |n| {
        let cur2 = cur.clone();
        let lines: Vec<_> = (0..n).map(|k| line(k, cur2.clone())).collect();
        let cur3 = cur.clone();
        lines.prop_map(move |lines| CartSnapshot {
            currency: cur3.clone(),
            lines,
        })
    })
}

pub fn groups() -> impl Strategy<Value = Vec<StackingGroup>> {
    (1usize..=3).prop_flat_map(|n| {
        let gs: Vec<_> = (0..n)
            .map(|k| {
                (-5i64..=5, stacking_mode()).prop_map(move |(priority, mode)| StackingGroup {
                    group_id: format!("g{k}"),
                    priority,
                    mode,
                })
            })
            .collect();
        gs
    })
}

pub fn rule() -> impl Strategy<Value = Rule> {
    (
        prop::option::of(NOW - 50..NOW + 50),
        prop::option::of(NOW - 50..NOW + 50),
    )
        .prop_map(|(starts_at, ends_at)| Rule::DateWindow { starts_at, ends_at })
}

pub fn action(cur: Currency) -> impl Strategy<Value = Action> {
    prop_oneof![
        bps().prop_map(|b| Action::OrderPercentage { bps: b }),
        money(cur).prop_map(|amount| Action::OrderFixedAmount { amount }),
    ]
}

pub fn promotion(idx: usize, cur: Currency, n_groups: usize) -> impl Strategy<Value = Promotion> {
    (
        0..n_groups,
        -5i64..=5,
        prop::bool::weighted(0.15),
        prop::option::weighted(0.3, money(cur.clone())),
        prop::option::weighted(0.3, money(cur.clone())),
        prop::option::weighted(0.3, rounding_policy()),
        prop::collection::vec(rule(), 0..=2),
        action(cur),
    )
        .prop_map(
            move |(g, priority, exclusive, cap, remaining_budget, rounding, rules, action)| {
                Promotion {
                    promotion_id: format!("P{idx}"),
                    group_id: format!("g{g}"),
                    priority,
                    exclusive,
                    cap,
                    remaining_budget,
                    rounding,
                    rules,
                    action,
                }
            },
        )
}

/// A valid input: every check of `validate` passes.
pub fn valid_input() -> impl Strategy<Value = PricingInput> {
    currency().prop_flat_map(|cur| {
        (policy(cur.clone()), cart(cur.clone()), groups(), Just(cur)).prop_flat_map(
            |(policy, cart, groups, cur)| {
                let n_groups = groups.len();
                (0usize..=6).prop_flat_map(move |n| {
                    let ps: Vec<_> = (0..n)
                        .map(|k| promotion(k, cur.clone(), n_groups))
                        .collect();
                    let policy = policy.clone();
                    let cart = cart.clone();
                    let groups = groups.clone();
                    ps.prop_map(move |promotions| PricingInput {
                        now: NOW,
                        policy: policy.clone(),
                        customer: CustomerContext {},
                        cart: cart.clone(),
                        groups: groups.clone(),
                        promotions,
                    })
                })
            },
        )
    })
}

/// The violations `validate` must report, one per case. Some are no-ops on inputs without
/// the relevant element; those cases simply stay valid.
pub const MUTATIONS: usize = 15;

pub fn mutate(i: &mut PricingInput, which: usize) {
    let cur = i.cart.currency.clone();
    let other = other_currency(&cur).to_string();
    match which {
        0 => i.cart.currency = cur.to_lowercase(),
        1 => {
            if let Some(l) = i.cart.lines.first_mut() {
                l.unit_price.currency = other;
            }
        }
        2 => {
            if let Some(l) = i.cart.lines.first_mut() {
                l.unit_price.amount = -1;
            }
        }
        3 => {
            if let Some(l) = i.cart.lines.last_mut() {
                l.unit_price.amount = MAX_MINOR_UNITS + 1;
            }
        }
        4 => {
            if let Some(l) = i.cart.lines.first_mut() {
                l.quantity = 0;
            }
        }
        5 => {
            if i.cart.lines.len() >= 2 {
                let id = i.cart.lines[0].line_id.clone();
                i.cart.lines[1].line_id = id;
            }
        }
        6 => i.policy.order_cap.bps = Some(BPS_SCALE + 1),
        7 => {
            i.policy.order_cap.amount = Some(Money {
                amount: 10,
                currency: other,
            })
        }
        8 => {
            if i.groups.len() >= 2 {
                let id = i.groups[0].group_id.clone();
                i.groups[1].group_id = id;
            }
        }
        9 => {
            if let Some(p) = i.promotions.first_mut() {
                p.group_id = "nope".to_string();
            }
        }
        10 => {
            if let Some(p) = i.promotions.first_mut() {
                p.action = match &p.action {
                    Action::OrderPercentage { .. } => {
                        Action::OrderPercentage { bps: BPS_SCALE + 1 }
                    }
                    Action::OrderFixedAmount { amount } => Action::OrderFixedAmount {
                        amount: Money {
                            amount: -5,
                            currency: amount.currency.clone(),
                        },
                    },
                };
            }
        }
        11 => {
            if let Some(p) = i.promotions.last_mut() {
                p.cap = Some(Money {
                    amount: 100,
                    currency: other,
                });
            }
        }
        12 => {
            if i.promotions.len() >= 2 {
                let id = i.promotions[0].promotion_id.clone();
                i.promotions[1].promotion_id = id;
            }
        }
        13 => {
            if let Some(l) = i.cart.lines.first_mut() {
                l.unit_price.amount = MAX_MINOR_UNITS;
                l.quantity = 2;
            }
        }
        _ => {
            if let Some(p) = i.promotions.first_mut() {
                p.remaining_budget = Some(Money {
                    amount: 1,
                    currency: "EU".to_string(),
                });
            }
        }
    }
}

/// Mostly valid inputs, about one in ten carrying one violation.
pub fn input() -> impl Strategy<Value = PricingInput> {
    (valid_input(), prop::option::weighted(0.1, 0..MUTATIONS)).prop_map(|(mut i, m)| {
        if let Some(which) = m {
            mutate(&mut i, which);
        }
        i
    })
}

/// Deterministic Fisher-Yates driven by a seed, for permutation tests.
pub fn shuffle<T>(v: &mut [T], mut seed: u64) {
    for k in (1..v.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let j = (seed % (k as u64 + 1)) as usize;
        v.swap(k, j);
    }
}
