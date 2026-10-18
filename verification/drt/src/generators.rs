//! proptest strategies for `PricingInput` (ADR 2, ADR 3). The valid generator covers the
//! validated domain of `docs/ubiquitous-language.md`; `mutated` applies one random
//! violation so that the validation order (ADR 6) is compared too. Promotion amounts are
//! sometimes in another currency: with a rate they exercise `normalize`, without one
//! `CurrencyMismatch`.

use discount_domain::*;
use proptest::prelude::*;
use proptest::strategy::BoxedStrategy;

pub const NOW: Timestamp = 1_000;

const CURRENCIES: [&str; 3] = ["EUR", "USD", "RUB"];
pub const TAGS: [&str; 5] = ["#ios", "#android", "#partner", "#promo", "cpa-meta"];
pub const COUPON_CODES: [&str; 4] = ["WELCOME", "SUMMER", "VIP", "X1"];

fn other_currency(c: &str) -> &'static str {
    if c == "EUR" { "USD" } else { "EUR" }
}

pub fn currency() -> impl Strategy<Value = Currency> {
    prop::sample::select(CURRENCIES.to_vec()).prop_map(|c| c.to_string())
}

/// Amounts: mostly small, sometimes mid-range.
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

/// Money for a promotion: usually the cart currency, sometimes another one (needs a rate).
pub fn promo_money(cur: Currency) -> impl Strategy<Value = Money> {
    let other = other_currency(&cur).to_string();
    (small_amount(), prop::bool::weighted(0.2)).prop_map(move |(a, foreign)| Money {
        amount: a,
        currency: if foreign { other.clone() } else { cur.clone() },
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
        prop::option::weighted(0.3, promo_money(cur)),
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

pub fn tags() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(prop::sample::select(TAGS.to_vec()), 0..=3).prop_map(|v| {
        let mut out: Vec<String> = Vec::new();
        for t in v {
            if !out.iter().any(|x| x == t) {
                out.push(t.to_string());
            }
        }
        out
    })
}

pub fn attributes() -> impl Strategy<Value = Vec<Attribute>> {
    let checkin = prop_oneof![
        3 => (NOW - 100..NOW + 100).prop_map(|t| t.to_string()),
        1 => Just("soon".to_string()),
        1 => Just("-5".to_string()),
    ]
    .prop_map(|v| Attribute {
        key: "checkin".into(),
        value: v,
    });
    let kind =
        prop::sample::select(vec!["resort", "hostel", "apartment"]).prop_map(|v| Attribute {
            key: "hotel_kind".into(),
            value: v.to_string(),
        });
    (
        prop::option::weighted(0.6, checkin),
        prop::option::weighted(0.6, kind),
    )
        .prop_map(|(a, b)| a.into_iter().chain(b).collect())
}

/// Distinct codes presented with the cart.
pub fn coupon_codes() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(prop::sample::select(COUPON_CODES.to_vec()), 0..=2).prop_map(|v| {
        let mut out: Vec<String> = Vec::new();
        for c in v {
            if !out.iter().any(|x| x == c) {
                out.push(c.to_string());
            }
        }
        out
    })
}

pub fn cart(cur: Currency) -> impl Strategy<Value = CartSnapshot> {
    (0usize..=6).prop_flat_map(move |n| {
        let cur2 = cur.clone();
        let lines: Vec<_> = (0..n).map(|k| line(k, cur2.clone())).collect();
        let cur3 = cur.clone();
        (lines, tags(), attributes(), coupon_codes()).prop_map(
            move |(lines, tags, attributes, coupon_codes)| CartSnapshot {
                currency: cur3.clone(),
                lines,
                tags,
                attributes,
                coupon_codes,
            },
        )
    })
}

pub fn customer(n_promotions: usize) -> BoxedStrategy<CustomerContext> {
    let uses = prop::collection::vec((0..n_promotions.max(1), 0..=3i64), 0..=2).prop_map(|v| {
        let mut out: Vec<PromotionUse> = Vec::new();
        for (k, count) in v {
            let pid = format!("P{k}");
            if !out.iter().any(|u| u.promotion_id == pid) {
                out.push(PromotionUse {
                    promotion_id: pid,
                    count,
                });
            }
        }
        out
    });
    (
        prop::option::weighted(0.7, prop::sample::select(vec!["c1", "c2"])),
        0..=3i64,
        uses,
        prop::option::weighted(0.6, prop::sample::select(vec!["gold", "silver", "x"])),
        prop::option::weighted(0.6, prop::sample::select(vec!["0", "1", "2", "many"])),
        prop::collection::vec(prop::sample::select(vec!["P0", "P1", "P2"]), 0..=2),
    )
        .prop_map(|(cid, completed_orders, uses, tier, app, listed)| {
            let mut attributes = Vec::new();
            if let Some(t) = tier {
                attributes.push(Attribute {
                    key: "tier".into(),
                    value: t.into(),
                });
            }
            if let Some(a) = app {
                attributes.push(Attribute {
                    key: "completed_app_orders".into(),
                    value: a.into(),
                });
            }
            let mut allowlisted: Vec<String> = Vec::new();
            for pid in listed {
                if !allowlisted.iter().any(|p| p == pid) {
                    allowlisted.push(pid.into());
                }
            }
            CustomerContext {
                customer_id: cid.map(|c| c.to_string()),
                completed_orders,
                uses,
                attributes,
                allowlisted,
            }
        })
        .boxed()
}

/// Coupon snapshots with distinct codes, pointing at generated promotion ids.
pub fn coupons(n_promotions: usize) -> impl Strategy<Value = Vec<Coupon>> {
    prop::collection::vec(
        (
            prop::sample::select(COUPON_CODES.to_vec()),
            0..n_promotions.max(1),
            prop::option::of(NOW - 50..NOW + 50),
            prop::option::of(NOW - 50..NOW + 50),
            prop::option::weighted(0.3, prop::sample::select(vec!["c1", "c2"])),
            prop::option::weighted(0.5, 0..=2i64),
            0..=2i64,
        ),
        0..=3,
    )
    .prop_map(|v| {
        let mut out: Vec<Coupon> = Vec::new();
        for (code, k, starts_at, ends_at, cid, max_uses, uses) in v {
            if !out.iter().any(|c| c.code == code) {
                out.push(Coupon {
                    code: code.to_string(),
                    promotion_id: format!("P{k}"),
                    starts_at,
                    ends_at,
                    customer_id: cid.map(|c| c.to_string()),
                    max_uses,
                    uses,
                });
            }
        }
        out
    })
}

/// Rates from the other currencies into `cur`, distinct pairs.
pub fn exchange_rates(cur: Currency) -> impl Strategy<Value = Vec<ExchangeRate>> {
    let others: Vec<&'static str> = CURRENCIES.iter().copied().filter(|c| *c != cur).collect();
    let cur2 = cur.clone();
    prop::collection::vec(
        (prop::sample::select(others), 1..=200i64, 1..=200i64),
        0..=2,
    )
    .prop_map(move |v| {
        let mut out: Vec<ExchangeRate> = Vec::new();
        for (from, num, den) in v {
            if !out.iter().any(|r| r.from == from) {
                out.push(ExchangeRate {
                    from: from.to_string(),
                    to: cur2.clone(),
                    numerator: num,
                    denominator: den,
                });
            }
        }
        out
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

fn tag_list() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(prop::sample::select(TAGS.to_vec()), 0..=2)
        .prop_map(|v| v.into_iter().map(str::to_string).collect::<Vec<_>>())
}

pub fn rule(cur: Currency) -> impl Strategy<Value = Rule> {
    let window = (
        prop::option::of(NOW - 50..NOW + 50),
        prop::option::of(NOW - 50..NOW + 50),
    );
    prop_oneof![
        3 => window.clone().prop_map(|(starts_at, ends_at)| Rule::DateWindow { starts_at, ends_at }),
        2 => (prop::option::of(promo_money(cur.clone())), prop::option::of(promo_money(cur.clone())))
            .prop_map(|(min, max)| Rule::OrderTotalRange { min, max }),
        2 => (tag_list(), tag_list(), tag_list()).prop_map(|(all, any, none)| Rule::TagsMatch { all, any, none }),
        1 => prop::collection::vec(prop::sample::select(vec!["resort", "hostel", "villa"]), 0..=2)
            .prop_map(|vs| Rule::AttributeIn { key: "hotel_kind".into(), values: vs.into_iter().map(str::to_string).collect() }),
        1 => window.prop_map(|(starts_at, ends_at)| Rule::AttributeDateWindow { key: "checkin".into(), starts_at, ends_at }),
        1 => (0..=2i64).prop_map(|max| Rule::MaxCompletedOrders { max }),
        1 => (0..=3i64).prop_map(|max| Rule::MaxUsesPerCustomer { max }),
        1 => (0..=3i64).prop_map(|max| Rule::MaxUsesTotal { max }),
        2 => Just(Rule::RequiresCoupon),
        1 => prop::collection::vec(prop::sample::select(vec!["gold", "silver"]), 0..=2)
            .prop_map(|vs| Rule::CustomerAttributeIn { key: "tier".into(), values: vs.into_iter().map(str::to_string).collect() }),
        1 => (prop::option::of(0..=2i64), prop::option::of(0..=2i64))
            .prop_map(|(min, max)| Rule::CustomerAttributeRange { key: "completed_app_orders".into(), min, max }),
        1 => Just(Rule::RequiresAllowlist),
    ]
}

pub fn action(cur: Currency) -> impl Strategy<Value = Action> {
    prop_oneof![
        bps().prop_map(|b| Action::OrderPercentage { bps: b }),
        promo_money(cur).prop_map(|amount| Action::OrderFixedAmount { amount }),
    ]
}

pub fn promotion(idx: usize, cur: Currency, n_groups: usize) -> impl Strategy<Value = Promotion> {
    (
        0..n_groups,
        -5i64..=5,
        prop::bool::weighted(0.15),
        prop::option::weighted(0.3, promo_money(cur.clone())),
        prop::option::weighted(0.3, promo_money(cur.clone())),
        prop::option::weighted(0.3, rounding_policy()),
        prop::collection::vec(rule(cur.clone()), 0..=2),
        action(cur),
        0..=3i64,
    )
        .prop_map(
            move |(
                g,
                priority,
                exclusive,
                cap,
                remaining_budget,
                rounding,
                rules,
                action,
                uses_total,
            )| {
                Promotion {
                    promotion_id: format!("P{idx}"),
                    group_id: format!("g{g}"),
                    priority,
                    exclusive,
                    cap,
                    remaining_budget,
                    uses_total,
                    rounding,
                    rules,
                    action,
                }
            },
        )
}

/// A well-formed input: every check of `validate` passes, except that a promotion amount in
/// another currency has a rate only most of the time (so `CurrencyMismatch` is exercised).
pub fn valid_input() -> impl Strategy<Value = PricingInput> {
    currency().prop_flat_map(|cur| {
        (
            policy(cur.clone()),
            cart(cur.clone()),
            groups(),
            exchange_rates(cur.clone()),
            Just(cur),
        )
            .prop_flat_map(|(policy, cart, groups, rates, cur)| {
                let n_groups = groups.len();
                (0usize..=6).prop_flat_map(move |n| {
                    let ps: Vec<_> = (0..n)
                        .map(|k| promotion(k, cur.clone(), n_groups))
                        .collect();
                    let policy = policy.clone();
                    let cart = cart.clone();
                    let groups = groups.clone();
                    let rates = rates.clone();
                    (ps, customer(n), coupons(n)).prop_map(
                        move |(promotions, customer, coupons)| PricingInput {
                            now: NOW,
                            policy: policy.clone(),
                            customer,
                            cart: cart.clone(),
                            coupons,
                            exchange_rates: rates.clone(),
                            groups: groups.clone(),
                            promotions,
                        },
                    )
                })
            })
    })
}

/// The violations `validate` must report, one per case. Some are no-ops on inputs without
/// the relevant element; those cases simply stay valid.
pub const MUTATIONS: usize = 26;

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
                currency: "GBP".to_string(),
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
                    currency: "GBP".to_string(),
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
        14 => {
            if let Some(p) = i.promotions.first_mut() {
                p.remaining_budget = Some(Money {
                    amount: 1,
                    currency: "EU".to_string(),
                });
            }
        }
        15 => i.customer.completed_orders = -1,
        16 => i.customer.uses.push(PromotionUse {
            promotion_id: "P0".into(),
            count: -1,
        }),
        17 => {
            i.customer.uses.push(PromotionUse {
                promotion_id: "P0".into(),
                count: 1,
            });
            i.customer.uses.push(PromotionUse {
                promotion_id: "P0".into(),
                count: 2,
            });
        }
        18 => {
            if let Some(r) = i.exchange_rates.first_mut() {
                r.denominator = 0;
            } else {
                i.exchange_rates.push(ExchangeRate {
                    from: other.clone(),
                    to: cur.clone(),
                    numerator: 0,
                    denominator: 1,
                });
            }
        }
        19 => {
            let r = ExchangeRate {
                from: other.clone(),
                to: cur.clone(),
                numerator: 1,
                denominator: 1,
            };
            i.exchange_rates.push(r.clone());
            i.exchange_rates.push(r);
        }
        20 => {
            let c = Coupon {
                code: "DUP".into(),
                promotion_id: "P0".into(),
                starts_at: None,
                ends_at: None,
                customer_id: None,
                max_uses: None,
                uses: 0,
            };
            i.coupons.push(c.clone());
            i.coupons.push(c);
        }
        21 => {
            if let Some(p) = i.promotions.first_mut() {
                p.rules.push(Rule::MaxUsesPerCustomer { max: -1 });
            }
        }
        22 => {
            if let Some(c) = i.coupons.first_mut() {
                c.uses = -1;
            } else {
                i.coupons.push(Coupon {
                    code: "NEG".into(),
                    promotion_id: "P0".into(),
                    starts_at: None,
                    ends_at: None,
                    customer_id: None,
                    max_uses: Some(-1),
                    uses: 0,
                });
            }
        }
        23 => {
            if let Some(p) = i.promotions.first_mut() {
                p.uses_total = -1;
            }
        }
        24 => {
            if let Some(p) = i.promotions.first_mut() {
                p.rules.push(Rule::MaxUsesTotal { max: -1 });
            }
        }
        _ => {
            i.customer.allowlisted = vec!["P0".into(), "P1".into(), "P0".into()];
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
