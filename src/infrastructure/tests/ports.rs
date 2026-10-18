//! The `redemption` and `promotion-catalog` scenarios, run against the in-memory store and,
//! against Postgres (`DATABASE_URL` or the local stack; never skipped, ADR 11).

mod common;

use std::sync::Arc;

use common::*;
use discount_application::ports::{
    CatalogState, Counters, GenerateCoupons, GrantRule, GrantTrigger, PromotionCatalog,
    PromotionMetadata, RedemptionError, RedemptionRepository, RedemptionStatus, ReserveOutcome,
    StorageError, Tenancy, Tenant,
};
use discount_application::{
    CancelRedemption, ConfirmRedemption, CouponError, CouponStatus, CustomerCouponError, Evaluate,
    GenerateCouponsUseCase, GrantCoupon, ListCustomerCoupons, Redeem, SaveCustomerCoupon,
};
use discount_domain::*;
use discount_infrastructure::{FixedClock, NoopEventPublisher, default_policy};

struct Ports {
    catalog: Arc<dyn PromotionCatalog>,
    redemptions: Arc<dyn RedemptionRepository>,
}

async fn memory_ports() -> Ports {
    let store = discount_infrastructure::InMemoryStore::new(default_policy());
    Ports {
        catalog: Arc::new(store.clone()),
        redemptions: Arc::new(store),
    }
}

async fn postgres_ports() -> Ports {
    let store = pg_store().await;
    Ports {
        catalog: Arc::new(store.clone()),
        redemptions: Arc::new(store),
    }
}

/// Postgres scenarios share one database, so they run one at a time.
static PG_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Run `f` against every available implementation.
async fn for_each_backend<F, Fut>(f: F)
where
    F: Fn(Ports) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    f(memory_ports().await).await;
    let _serial = PG_LOCK.lock().await;
    f(postgres_ports().await).await;
}

async fn seed(p: &Ports, budget: Option<i64>) {
    p.catalog.put_group(group_g()).await.unwrap();
    p.catalog.put_promotion(fixed_300("P1")).await.unwrap();
    if let Some(b) = budget {
        assert!(p.catalog.put_budget("P1", Some(eur(b))).await.unwrap());
    }
}

async fn remaining(p: &Ports, id: &str) -> Option<Money> {
    p.catalog
        .get_budget(id)
        .await
        .unwrap()
        .expect("promotion exists")
}

fn redeem<'a>(p: &'a Ports, clock: &'a FixedClock) -> Redeem<'a> {
    Redeem {
        catalog: &*p.catalog,
        redemptions: &*p.redemptions,
        clock,
        events: &NoopEventPublisher,
    }
}

#[tokio::test]
async fn reserve_decrements_budget_and_is_idempotent() {
    for_each_backend(|p| async move {
        seed(&p, Some(1000)).await;
        let clock = FixedClock(NOW);
        let o1 = redeem(&p, &clock)
            .run("order-1", sample_cart(), CustomerContext::default())
            .await
            .unwrap();
        let r1 = match &o1 {
            ReserveOutcome::Reserved(r) => r.clone(),
            other => panic!("expected Reserved, got {other:?}"),
        };
        assert_eq!(r1.status, RedemptionStatus::Reserved);
        assert_eq!(r1.result.applied[0].discount.amount, 300);
        assert_eq!(remaining(&p, "P1").await, Some(eur(700)));
        // repeat: stored result, budget untouched
        let o2 = redeem(&p, &clock)
            .run("order-1", sample_cart(), CustomerContext::default())
            .await
            .unwrap();
        assert!(matches!(o2, ReserveOutcome::Idempotent(_)));
        assert_eq!(o2.redemption().result, r1.result);
        assert_eq!(remaining(&p, "P1").await, Some(eur(700)));
        // stored, and the stored input is a complete replay record (invariant 6)
        let stored = p.redemptions.get("order-1").await.unwrap().unwrap();
        assert_eq!(stored.result, r1.result);
        assert_eq!(stored.input.promotions[0].remaining_budget, Some(eur(1000)));
        assert_eq!(apply(&stored.input), Ok(stored.result.clone()));
    })
    .await;
}

#[tokio::test]
async fn concurrent_reservations_respect_the_budget() {
    for_each_backend(|p| async move {
        seed(&p, Some(500)).await;
        let clock = FixedClock(NOW);
        let ra = redeem(&p, &clock);
        let rb = redeem(&p, &clock);
        let (a, b) = tokio::join!(
            ra.run("order-a", sample_cart(), CustomerContext::default()),
            rb.run("order-b", sample_cart(), CustomerContext::default())
        );
        let mut discounts: Vec<(i64, Option<ClampReason>)> = [a.unwrap(), b.unwrap()]
            .iter()
            .map(|o| {
                (
                    o.redemption().result.applied[0].discount.amount,
                    o.redemption().result.applied[0].clamped_by,
                )
            })
            .collect();
        discounts.sort_by_key(|d| d.0);
        assert_eq!(
            discounts,
            vec![(200, Some(ClampReason::Budget)), (300, None)]
        );
        assert_eq!(remaining(&p, "P1").await, Some(eur(0)));
    })
    .await;
}

#[tokio::test]
async fn confirm_and_cancel() {
    for_each_backend(|p| async move {
        seed(&p, Some(1000)).await;
        let clock = FixedClock(NOW);
        redeem(&p, &clock)
            .run("order-1", sample_cart(), CustomerContext::default())
            .await
            .unwrap();
        assert_eq!(remaining(&p, "P1").await, Some(eur(700)));
        let confirm = ConfirmRedemption {
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        let cancel = CancelRedemption {
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        assert_eq!(
            confirm.run("order-1").await.unwrap().status,
            RedemptionStatus::Confirmed
        );
        assert_eq!(
            confirm.run("order-1").await,
            Err(RedemptionError::InvalidTransition {
                from: RedemptionStatus::Confirmed,
                to: RedemptionStatus::Confirmed
            })
        );
        assert_eq!(
            cancel.run("order-1").await.unwrap().status,
            RedemptionStatus::Cancelled
        );
        assert_eq!(remaining(&p, "P1").await, Some(eur(1000)));
        assert_eq!(
            confirm.run("order-1").await,
            Err(RedemptionError::InvalidTransition {
                from: RedemptionStatus::Cancelled,
                to: RedemptionStatus::Confirmed
            })
        );
        assert_eq!(confirm.run("nope").await, Err(RedemptionError::NotFound));
        // a cancelled order can be redeemed again
        let again = redeem(&p, &clock)
            .run("order-1", sample_cart(), CustomerContext::default())
            .await
            .unwrap();
        assert!(matches!(again, ReserveOutcome::Reserved(_)));
        assert_eq!(remaining(&p, "P1").await, Some(eur(700)));
    })
    .await;
}

#[tokio::test]
async fn pricing_error_stores_nothing() {
    for_each_backend(|p| async move {
        seed(&p, Some(1000)).await;
        let clock = FixedClock(NOW);
        let mut cart = sample_cart();
        cart.currency = "eur".into();
        let r = redeem(&p, &clock)
            .run("order-x", cart, CustomerContext::default())
            .await;
        assert_eq!(
            r,
            Err(RedemptionError::Pricing(PricingError::InvalidCurrency {
                field: "cart.currency".into()
            }))
        );
        assert_eq!(p.redemptions.get("order-x").await.unwrap(), None);
        assert_eq!(remaining(&p, "P1").await, Some(eur(1000)));
    })
    .await;
}

#[tokio::test]
async fn catalogue_crud_archive_and_budget_flow() {
    for_each_backend(|p| async move {
        let clock = FixedClock(NOW);
        let evaluate = Evaluate {
            catalog: &*p.catalog,
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        // empty catalogue prices to no discount
        assert!(p.catalog.list_promotions().await.unwrap().is_empty());
        assert_eq!(
            evaluate
                .run(sample_cart(), CustomerContext::default())
                .await
                .unwrap()
                .total_discount
                .amount,
            0
        );
        // create and price
        p.catalog.put_group(group_g()).await.unwrap();
        let mut pct = fixed_300("P10");
        pct.action = Action::OrderPercentage { bps: 1000 };
        p.catalog.put_promotion(pct.clone()).await.unwrap();
        assert_eq!(
            evaluate
                .run(sample_cart(), CustomerContext::default())
                .await
                .unwrap()
                .total_discount
                .amount,
            449
        );
        assert_eq!(
            p.catalog
                .get_promotion("P10")
                .await
                .unwrap()
                .unwrap()
                .action,
            pct.action
        );
        // budget flows into pricing
        assert!(p.catalog.put_budget("P10", Some(eur(250))).await.unwrap());
        let r = evaluate
            .run(sample_cart(), CustomerContext::default())
            .await
            .unwrap();
        assert_eq!(
            (r.applied[0].discount.amount, r.applied[0].clamped_by),
            (250, Some(ClampReason::Budget))
        );
        assert_eq!(
            p.catalog.list_budgets().await.unwrap(),
            vec![("P10".to_string(), eur(250))]
        );
        assert!(!p.catalog.put_budget("nope", Some(eur(1))).await.unwrap());
        // policy round trip
        let mut pol = default_policy();
        pol.rounding = RoundingPolicy::HalfUp;
        p.catalog.put_policy(pol.clone()).await.unwrap();
        assert_eq!(p.catalog.get_policy().await.unwrap(), pol);
        assert_eq!(p.catalog.snapshot().await.unwrap().policy, pol);
        // archive hides from pricing and listing
        assert!(p.catalog.archive_promotion("P10").await.unwrap());
        assert!(!p.catalog.archive_promotion("P10").await.unwrap());
        assert!(p.catalog.list_promotions().await.unwrap().is_empty());
        assert_eq!(p.catalog.get_promotion("P10").await.unwrap(), None);
        assert_eq!(
            evaluate
                .run(sample_cart(), CustomerContext::default())
                .await
                .unwrap()
                .total_discount
                .amount,
            0
        );
        // groups
        assert_eq!(p.catalog.list_groups().await.unwrap().len(), 1);
        assert!(p.catalog.delete_group("g").await.unwrap());
        assert!(!p.catalog.delete_group("g").await.unwrap());
        assert_eq!(p.catalog.get_group("g").await.unwrap(), None);
    })
    .await;
}

#[tokio::test]
async fn coupons_rates_and_uses_flow_into_pricing() {
    for_each_backend(|p| async move {
        let clock = FixedClock(NOW);
        p.catalog.put_group(group_g()).await.unwrap();
        // P1: requires a coupon, grants 1000 USD; the cart is in EUR
        let mut p1 = fixed_300("P1");
        p1.action = Action::OrderFixedAmount {
            amount: Money::new(1000, "USD"),
        };
        p1.rules = vec![Rule::RequiresCoupon, Rule::MaxUsesPerCustomer { max: 2 }];
        p.catalog.put_promotion(p1).await.unwrap();
        p.catalog
            .put_coupon(Coupon {
                code: "WELCOME".into(),
                promotion_id: "P1".into(),
                starts_at: None,
                ends_at: None,
                customer_id: None,
                max_uses: None,
                uses: 0,
            })
            .await
            .unwrap();
        assert_eq!(p.catalog.list_coupons().await.unwrap().len(), 1);
        assert!(p.catalog.get_coupon("WELCOME").await.unwrap().is_some());
        p.catalog
            .put_rate(ExchangeRate {
                from: "USD".into(),
                to: "EUR".into(),
                numerator: 92,
                denominator: 100,
            })
            .await
            .unwrap();
        assert_eq!(p.catalog.list_rates().await.unwrap().len(), 1);
        let snapshot = p.catalog.snapshot().await.unwrap();
        assert_eq!(
            (snapshot.coupons.len(), snapshot.exchange_rates.len()),
            (1, 1)
        );

        let evaluate = Evaluate {
            catalog: &*p.catalog,
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        let mut cart = sample_cart();
        let mut customer = CustomerContext {
            customer_id: Some("c1".into()),
            ..CustomerContext::default()
        };
        // no code presented -> not eligible
        let r = evaluate.run(cart.clone(), customer.clone()).await.unwrap();
        assert_eq!(
            r.skipped[0].reason,
            SkipReason::NotEligible { rule_index: 0 }
        );
        // code presented -> converted amount applies (920 EUR)
        cart.coupon_codes = vec!["WELCOME".into()];
        let r = evaluate.run(cart.clone(), customer.clone()).await.unwrap();
        assert_eq!(r.applied[0].discount.amount, 920);
        // two reservations for c1 -> uses reach the limit; a third evaluation skips P1
        let redeem = Redeem {
            catalog: &*p.catalog,
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        let o1 = redeem
            .run("c1-1", cart.clone(), customer.clone())
            .await
            .unwrap();
        assert_eq!(o1.redemption().customer_id.as_deref(), Some("c1"));
        redeem
            .run("c1-2", cart.clone(), customer.clone())
            .await
            .unwrap();
        assert_eq!(
            p.redemptions.uses_for_customer("c1").await.unwrap(),
            vec![("P1".to_string(), 2)]
        );
        let r = evaluate.run(cart.clone(), customer.clone()).await.unwrap();
        assert_eq!(
            r.skipped[0].reason,
            SkipReason::NotEligible { rule_index: 1 }
        );
        // the third redeem itself also sees the limit
        let o3 = redeem
            .run("c1-3", cart.clone(), customer.clone())
            .await
            .unwrap();
        assert!(o3.redemption().result.applied.is_empty());
        // cancelled redemptions do not count
        let cancel = CancelRedemption {
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        cancel.run("c1-1").await.unwrap();
        assert_eq!(
            p.redemptions.uses_for_customer("c1").await.unwrap(),
            vec![("P1".to_string(), 1)]
        );
        // another customer is unaffected
        customer.customer_id = Some("c2".into());
        assert_eq!(
            evaluate.run(cart.clone(), customer).await.unwrap().applied[0]
                .discount
                .amount,
            920
        );
        // unknown code contributes nothing
        cart.coupon_codes = vec!["NOPE".into()];
        let r = evaluate
            .run(cart, CustomerContext::default())
            .await
            .unwrap();
        assert_eq!(
            r.skipped[0].reason,
            SkipReason::NotEligible { rule_index: 0 }
        );
        assert!(p.catalog.delete_coupon("WELCOME").await.unwrap());
        assert!(p.catalog.delete_rate("USD", "EUR").await.unwrap());
        assert!(!p.catalog.delete_rate("USD", "EUR").await.unwrap());
    })
    .await;
}

fn coupon(code: &str, max_uses: Option<i64>) -> Coupon {
    Coupon {
        code: code.into(),
        promotion_id: "P1".into(),
        starts_at: None,
        ends_at: None,
        customer_id: None,
        max_uses,
        uses: 0,
    }
}

fn cart_with(code: &str) -> CartSnapshot {
    let mut cart = sample_cart();
    cart.coupon_codes = vec![code.into()];
    cart
}

/// Spec `coupon-lifecycle` (single-use code, counters) and `redemption` (counts after a
/// cancellation), `promotion-catalog` (counters flow into pricing, uses are read-only).
#[tokio::test]
async fn single_use_code_and_counters() {
    for_each_backend(|p| async move {
        let clock = FixedClock(NOW);
        p.catalog.put_group(group_g()).await.unwrap();
        let mut p1 = fixed_300("P1");
        p1.rules = vec![Rule::RequiresCoupon, Rule::MaxUsesTotal { max: 2 }];
        p.catalog.put_promotion(p1).await.unwrap();
        // `uses` on PUT is ignored
        let mut once = coupon("ONCE", Some(1));
        once.uses = 5;
        p.catalog.put_coupon(once).await.unwrap();
        assert_eq!(p.catalog.get_coupon("ONCE").await.unwrap().unwrap().uses, 0);
        p.catalog.put_coupon(coupon("MANY", None)).await.unwrap();

        let redeem = redeem(&p, &clock);
        let customer = CustomerContext::default();
        // o1 uses ONCE
        let o1 = redeem
            .run("o1", cart_with("ONCE"), customer.clone())
            .await
            .unwrap();
        assert_eq!(o1.redemption().result.applied.len(), 1);
        let snapshot = p.catalog.snapshot().await.unwrap();
        assert_eq!(
            snapshot
                .coupons
                .iter()
                .find(|c| c.code == "ONCE")
                .unwrap()
                .uses,
            1
        );
        assert_eq!(
            snapshot
                .coupons
                .iter()
                .find(|c| c.code == "MANY")
                .unwrap()
                .uses,
            0
        );
        assert_eq!(snapshot.promotions[0].uses_total, 1);
        assert_eq!(
            p.redemptions.counters(&["ONCE".to_string()]).await.unwrap(),
            Counters {
                promotion_uses: vec![("P1".into(), 1)],
                coupon_uses: vec![("ONCE".into(), 1)]
            }
        );
        // o2 with ONCE: exhausted -> NotEligible on RequiresCoupon (rule 0); evaluate agrees
        let evaluate = Evaluate {
            catalog: &*p.catalog,
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        let r = evaluate
            .run(cart_with("ONCE"), customer.clone())
            .await
            .unwrap();
        assert_eq!(
            r.skipped[0].reason,
            SkipReason::NotEligible { rule_index: 0 }
        );
        let o2 = redeem
            .run("o2", cart_with("ONCE"), customer.clone())
            .await
            .unwrap();
        assert_eq!(
            o2.redemption().result.skipped[0].reason,
            SkipReason::NotEligible { rule_index: 0 }
        );
        // a presented code whose promotion did not apply is not a use
        assert_eq!(p.catalog.get_coupon("ONCE").await.unwrap().unwrap().uses, 1);
        // MANY applies again -> uses_total reaches MaxUsesTotal
        let o3 = redeem
            .run("o3", cart_with("MANY"), customer.clone())
            .await
            .unwrap();
        assert_eq!(o3.redemption().result.applied.len(), 1);
        let r = evaluate
            .run(cart_with("MANY"), customer.clone())
            .await
            .unwrap();
        assert_eq!(
            r.skipped[0].reason,
            SkipReason::NotEligible { rule_index: 1 }
        );
        assert_eq!(
            p.catalog.snapshot().await.unwrap().promotions[0].uses_total,
            2
        );
        // cancelling o1 frees ONCE and one slot of the total
        let cancel = CancelRedemption {
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        cancel.run("o1").await.unwrap();
        assert_eq!(p.catalog.get_coupon("ONCE").await.unwrap().unwrap().uses, 0);
        assert_eq!(
            p.catalog.snapshot().await.unwrap().promotions[0].uses_total,
            1
        );
        let o4 = redeem.run("o4", cart_with("ONCE"), customer).await.unwrap();
        assert_eq!(o4.redemption().result.applied.len(), 1);
    })
    .await;
}

/// Spec `coupon-lifecycle`, "Concurrent single use": exactly one of two concurrent
/// reservations with the same single-use code applies the promotion.
#[tokio::test]
async fn concurrent_single_use() {
    for_each_backend(|p| async move {
        p.catalog.put_group(group_g()).await.unwrap();
        let mut p1 = fixed_300("P1");
        p1.rules = vec![Rule::RequiresCoupon];
        p.catalog.put_promotion(p1).await.unwrap();
        p.catalog.put_coupon(coupon("ONCE", Some(1))).await.unwrap();
        let mut handles = Vec::new();
        for k in 0..8 {
            let redemptions = p.redemptions.clone();
            let catalog = p.catalog.clone();
            handles.push(tokio::spawn(async move {
                let clock = FixedClock(NOW);
                let redeem = Redeem {
                    catalog: &*catalog,
                    redemptions: &*redemptions,
                    clock: &clock,
                    events: &NoopEventPublisher,
                };
                redeem
                    .run(
                        &format!("cc-{k}"),
                        cart_with("ONCE"),
                        CustomerContext::default(),
                    )
                    .await
                    .unwrap()
            }));
        }
        let mut applied = 0;
        for h in handles {
            applied += h.await.unwrap().redemption().result.applied.len();
        }
        assert_eq!(applied, 1);
        assert_eq!(p.catalog.get_coupon("ONCE").await.unwrap().unwrap().uses, 1);
    })
    .await;
}

/// Spec `coupon-lifecycle`: bulk generation, grant rules and the idempotent grant.
#[tokio::test]
async fn generation_and_grants() {
    for_each_backend(|p| async move {
        let clock = FixedClock(NOW);
        let codes = discount_infrastructure::SequentialCodes::default();
        p.catalog.put_group(group_g()).await.unwrap();
        let mut p1 = fixed_300("P1");
        p1.rules = vec![Rule::RequiresCoupon];
        p.catalog.put_promotion(p1).await.unwrap();

        let generate = GenerateCouponsUseCase {
            catalog: &*p.catalog,
            codes: &codes,
        };
        let req = GenerateCoupons {
            promotion_id: "P1".into(),
            count: 100,
            prefix: "SUMMER-".into(),
            starts_at: None,
            ends_at: Some(NOW + 3600),
            max_uses: Some(1),
            customer_id: None,
        };
        let generated = generate.run(req.clone()).await.unwrap();
        assert_eq!(generated.len(), 100);
        let stored = p.catalog.list_coupons().await.unwrap();
        assert_eq!(stored.len(), 100);
        assert!(stored.iter().all(|c| c.code.starts_with("SUMMER-")
            && c.max_uses == Some(1)
            && c.ends_at == Some(NOW + 3600)));
        // a second batch from a fresh sequence collides with the first 100 codes and retries
        // past them (the retry budget is 10 attempts per requested code)
        let fresh = discount_infrastructure::SequentialCodes::default();
        let generate2 = GenerateCouponsUseCase {
            catalog: &*p.catalog,
            codes: &fresh,
        };
        let more = generate2
            .run(GenerateCoupons {
                count: 20,
                ..req.clone()
            })
            .await
            .unwrap();
        assert!(more.iter().all(|c| !generated.contains(c)));
        assert_eq!(p.catalog.list_coupons().await.unwrap().len(), 120);
        assert_eq!(
            generate
                .run(GenerateCoupons {
                    promotion_id: "nope".into(),
                    ..req.clone()
                })
                .await,
            Err(CouponError::UnknownPromotion)
        );
        assert_eq!(
            generate.run(GenerateCoupons { count: 0, ..req }).await,
            Err(CouponError::InvalidCount)
        );

        // grant rules
        let grant = GrantCoupon {
            catalog: &*p.catalog,
            codes: &codes,
            clock: &clock,
        };
        assert_eq!(
            grant.run(GrantTrigger::Cancel, "c1", "o9").await,
            Err(CouponError::NoRuleForTrigger)
        );
        let rule = |id: &str, priority: i64| GrantRule {
            rule_id: id.into(),
            trigger: GrantTrigger::Cancel,
            promotion_id: "P1".into(),
            priority,
            validity_seconds: 86400,
            max_uses: Some(1),
        };
        p.catalog.put_grant_rule(rule("low", 1)).await.unwrap();
        p.catalog.put_grant_rule(rule("high", 5)).await.unwrap();
        assert_eq!(
            p.catalog.get_grant_rule("low").await.unwrap(),
            Some(rule("low", 1))
        );
        assert_eq!(p.catalog.list_grant_rules().await.unwrap().len(), 2);
        let g1 = grant.run(GrantTrigger::Cancel, "c1", "o9").await.unwrap();
        assert_eq!(
            (
                g1.promotion_id.as_str(),
                g1.customer_id.as_deref(),
                g1.starts_at,
                g1.ends_at,
                g1.max_uses,
                g1.uses
            ),
            ("P1", Some("c1"), Some(NOW), Some(NOW + 86400), Some(1), 0)
        );
        assert!(g1.code.starts_with("CANCEL-"));
        let g2 = grant.run(GrantTrigger::Cancel, "c1", "o9").await.unwrap();
        assert_eq!(g1, g2);
        assert_eq!(
            p.catalog
                .get_grant(GrantTrigger::Cancel, "o9")
                .await
                .unwrap(),
            Some(g1.clone())
        );
        assert_eq!(p.catalog.list_coupons().await.unwrap().len(), 121);
        // the granted code is bound to c1 and single-use
        let redeem = redeem(&p, &clock);
        let c1 = CustomerContext {
            customer_id: Some("c1".into()),
            ..CustomerContext::default()
        };
        let c2 = CustomerContext {
            customer_id: Some("c2".into()),
            ..CustomerContext::default()
        };
        assert!(
            redeem
                .run("g-wrong", cart_with(&g1.code), c2)
                .await
                .unwrap()
                .redemption()
                .result
                .applied
                .is_empty()
        );
        assert_eq!(
            redeem
                .run("g-ok", cart_with(&g1.code), c1.clone())
                .await
                .unwrap()
                .redemption()
                .result
                .applied
                .len(),
            1
        );
        assert!(
            redeem
                .run("g-again", cart_with(&g1.code), c1)
                .await
                .unwrap()
                .redemption()
                .result
                .applied
                .is_empty()
        );
        // no rule for the other trigger
        assert_eq!(
            grant.run(GrantTrigger::Review, "c1", "o9").await,
            Err(CouponError::NoRuleForTrigger)
        );
        assert!(p.catalog.delete_grant_rule("high").await.unwrap());
        assert!(!p.catalog.delete_grant_rule("high").await.unwrap());
    })
    .await;
}

/// Spec `customer-coupons`: three sources, statuses, save and forget; spec `redemption`:
/// coupon uses of a customer.
#[tokio::test]
async fn customer_coupons_listing() {
    for_each_backend(|p| async move {
        let clock = FixedClock(NOW);
        p.catalog.put_group(group_g()).await.unwrap();
        let mut p1 = fixed_300("P1");
        p1.rules = vec![Rule::RequiresCoupon];
        p.catalog.put_promotion(p1).await.unwrap();
        // P2 requires a coupon and a tag the sample cart lacks: presenting its code is not a use
        let mut p2 = fixed_300("P2");
        p2.rules = vec![
            Rule::RequiresCoupon,
            Rule::TagsMatch {
                all: vec!["#vip".into()],
                any: vec![],
                none: vec![],
            },
        ];
        p.catalog.put_promotion(p2).await.unwrap();
        let bound = |code: &str, cid: &str| Coupon {
            customer_id: Some(cid.into()),
            ..coupon(code, None)
        };
        p.catalog.put_coupon(bound("GRANT-1", "c1")).await.unwrap();
        p.catalog.put_coupon(bound("GRANT-2", "c2")).await.unwrap();
        p.catalog
            .put_coupon(Coupon {
                starts_at: Some(NOW + 10),
                ..bound("LATER", "c1")
            })
            .await
            .unwrap();
        p.catalog
            .put_coupon(Coupon {
                ends_at: Some(NOW),
                ..bound("GONE", "c1")
            })
            .await
            .unwrap();
        p.catalog.put_coupon(coupon("WELCOME", None)).await.unwrap();
        p.catalog.put_coupon(coupon("SPRING", None)).await.unwrap();
        p.catalog.put_coupon(coupon("ONCE", Some(1))).await.unwrap();
        p.catalog.put_coupon(coupon("OTHER", None)).await.unwrap();
        p.catalog
            .put_coupon(Coupon {
                promotion_id: "P2".into(),
                ..coupon("VIP", None)
            })
            .await
            .unwrap();

        let redeem = redeem(&p, &clock);
        let c1 = CustomerContext {
            customer_id: Some("c1".into()),
            ..CustomerContext::default()
        };
        let c2 = CustomerContext {
            customer_id: Some("c2".into()),
            ..CustomerContext::default()
        };
        // c1 uses WELCOME twice (o1, o2), presents VIP without applying it (o3), c2 uses ONCE
        let mut two = cart_with("WELCOME");
        two.coupon_codes.push("VIP".into());
        assert_eq!(
            redeem
                .run("o1", two.clone(), c1.clone())
                .await
                .unwrap()
                .redemption()
                .result
                .applied
                .len(),
            1
        );
        redeem
            .run("o2", cart_with("WELCOME"), c1.clone())
            .await
            .unwrap();
        assert!(
            redeem
                .run("o3", cart_with("VIP"), c1.clone())
                .await
                .unwrap()
                .redemption()
                .result
                .applied
                .is_empty()
        );
        redeem
            .run("o4", cart_with("ONCE"), c2.clone())
            .await
            .unwrap();
        assert_eq!(
            p.redemptions.coupon_uses_of_customer("c1").await.unwrap(),
            vec![
                ("WELCOME".to_string(), "o1".to_string()),
                ("WELCOME".to_string(), "o2".to_string())
            ]
        );
        assert_eq!(
            p.redemptions
                .coupon_uses_of_customer("nobody")
                .await
                .unwrap(),
            vec![]
        );

        let list = ListCustomerCoupons {
            catalog: &*p.catalog,
            redemptions: &*p.redemptions,
            clock: &clock,
        };
        let save = SaveCustomerCoupon {
            list: ListCustomerCoupons {
                catalog: &*p.catalog,
                redemptions: &*p.redemptions,
                clock: &clock,
            },
        };
        // save: idempotent, unknown, bound to another, own bound coupon
        let saved = save.run("c1", "SPRING").await.unwrap();
        assert_eq!(
            (saved.coupon.code.as_str(), saved.status),
            ("SPRING", CouponStatus::Active)
        );
        save.run("c1", "SPRING").await.unwrap();
        save.run("c1", "ONCE").await.unwrap();
        assert_eq!(
            save.run("c1", "NOPE").await,
            Err(CustomerCouponError::UnknownCode)
        );
        assert_eq!(
            save.run("c1", "GRANT-2").await,
            Err(CustomerCouponError::BoundToAnotherCustomer)
        );
        assert_eq!(
            save.run("c1", "GRANT-1").await.unwrap().status,
            CouponStatus::Active
        );
        assert_eq!(
            p.catalog.saved_codes("c1").await.unwrap(),
            vec!["ONCE".to_string(), "SPRING".to_string()]
        );

        let listed = list.run("c1").await.unwrap();
        let codes: Vec<&str> = listed.iter().map(|e| e.coupon.code.as_str()).collect();
        assert_eq!(
            codes,
            vec!["GONE", "GRANT-1", "LATER", "ONCE", "SPRING", "WELCOME"]
        );
        let by = |code: &str| listed.iter().find(|e| e.coupon.code == code).unwrap();
        assert_eq!(
            (by("GRANT-1").status, by("GRANT-1").order_id.as_deref()),
            (CouponStatus::Active, None)
        );
        assert_eq!(by("LATER").status, CouponStatus::Scheduled);
        assert_eq!(by("GONE").status, CouponStatus::Expired);
        assert_eq!(
            (by("WELCOME").status, by("WELCOME").order_id.as_deref()),
            (CouponStatus::Applied, Some("o1"))
        );
        // ONCE: saved by c1, exhausted by c2
        assert_eq!(
            (by("ONCE").status, by("ONCE").coupon.uses),
            (CouponStatus::Expired, 1)
        );
        assert_eq!(
            by("SPRING")
                .promotion
                .as_ref()
                .map(|p| p.promotion_id.as_str()),
            Some("P1")
        );
        assert!(
            by("SPRING")
                .promotion
                .as_ref()
                .is_some_and(|p| matches!(p.action, Action::OrderFixedAmount { .. }))
        );
        // c2: bound GRANT-2 and applied ONCE; nothing of c1's
        let listed2 = list.run("c2").await.unwrap();
        let codes2: Vec<(&str, CouponStatus)> = listed2
            .iter()
            .map(|e| (e.coupon.code.as_str(), e.status))
            .collect();
        assert_eq!(
            codes2,
            vec![
                ("GRANT-2", CouponStatus::Active),
                ("ONCE", CouponStatus::Applied)
            ]
        );
        assert_eq!(list.run("nobody").await.unwrap(), vec![]);
        // cancelling o1 and o2 drops WELCOME from c1's list
        let cancel = CancelRedemption {
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        cancel.run("o1").await.unwrap();
        assert_eq!(
            list.run("c1")
                .await
                .unwrap()
                .iter()
                .find(|e| e.coupon.code == "WELCOME")
                .map(|e| e.order_id.clone()),
            Some(Some("o2".into()))
        );
        cancel.run("o2").await.unwrap();
        assert!(
            list.run("c1")
                .await
                .unwrap()
                .iter()
                .all(|e| e.coupon.code != "WELCOME")
        );
        // deleted promotion -> expired with no promotion
        p.catalog.archive_promotion("P1").await.unwrap();
        let e = list
            .run("c1")
            .await
            .unwrap()
            .into_iter()
            .find(|e| e.coupon.code == "GRANT-1")
            .unwrap();
        assert_eq!((e.status, e.promotion), (CouponStatus::Expired, None));
        // forget
        assert!(p.catalog.forget_coupon("c1", "SPRING").await.unwrap());
        assert!(!p.catalog.forget_coupon("c1", "SPRING").await.unwrap());
        assert!(
            list.run("c1")
                .await
                .unwrap()
                .iter()
                .all(|e| e.coupon.code != "SPRING")
        );
    })
    .await;
}

fn metadata(title: &str, source: Option<&str>) -> PromotionMetadata {
    PromotionMetadata {
        title: title.into(),
        description: String::new(),
        source: source.map(str::to_string),
        labels: [("channel".to_string(), "app".to_string())]
            .into_iter()
            .collect(),
    }
}

/// Spec `promotion-catalog`: metadata round trip, status codes, listing and source filter;
/// spec `customer-coupons`: metadata in the entry and the source filter.
#[tokio::test]
async fn promotion_metadata_and_source_filter() {
    for_each_backend(|p| async move {
        let clock = FixedClock(NOW);
        p.catalog.put_group(group_g()).await.unwrap();
        for id in ["P1", "P2", "P3"] {
            let mut pr = fixed_300(id);
            pr.rules = vec![Rule::RequiresCoupon];
            p.catalog.put_promotion(pr).await.unwrap();
        }
        // nothing stored yet
        assert_eq!(p.catalog.get_metadata("P1").await.unwrap(), None);
        assert!(!p.catalog.delete_metadata("P1").await.unwrap());
        assert!(
            !p.catalog
                .put_metadata("nope", metadata("x", None))
                .await
                .unwrap()
        );
        // round trip
        assert!(
            p.catalog
                .put_metadata("P1", metadata("Summer", Some("hotels")))
                .await
                .unwrap()
        );
        assert!(
            p.catalog
                .put_metadata("P2", metadata("Roads", Some("cars")))
                .await
                .unwrap()
        );
        assert_eq!(
            p.catalog.get_metadata("P1").await.unwrap(),
            Some(metadata("Summer", Some("hotels")))
        );
        // upsert replaces
        assert!(
            p.catalog
                .put_metadata("P1", metadata("Summer 2", Some("hotels")))
                .await
                .unwrap()
        );
        assert_eq!(
            p.catalog.get_metadata("P1").await.unwrap().unwrap().title,
            "Summer 2"
        );
        let listed = p.catalog.list_metadata().await.unwrap();
        assert_eq!(
            listed.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            vec!["P1", "P2"]
        );
        // limits are checked by the type, independent of the store
        assert!(metadata("", None).check().is_err());
        assert!(metadata("ok", Some("Hotels")).check().is_err());
        assert!(metadata("ok", Some("hotels-2")).check().is_ok());
        let mut many = metadata("ok", None);
        many.labels = (0..21).map(|i| (format!("k{i}"), String::new())).collect();
        assert!(many.check().is_err());

        // customer coupons carry the metadata and filter by source
        let bound = |code: &str, pid: &str| Coupon {
            customer_id: Some("c1".into()),
            promotion_id: pid.into(),
            ..coupon(code, None)
        };
        p.catalog.put_coupon(bound("H1", "P1")).await.unwrap();
        p.catalog.put_coupon(bound("C1", "P2")).await.unwrap();
        p.catalog.put_coupon(bound("N1", "P3")).await.unwrap();
        let list = ListCustomerCoupons {
            catalog: &*p.catalog,
            redemptions: &*p.redemptions,
            clock: &clock,
        };
        let all = list.run("c1").await.unwrap();
        assert_eq!(all.len(), 3);
        let by = |code: &str| all.iter().find(|e| e.coupon.code == code).unwrap();
        assert_eq!(
            by("H1")
                .promotion
                .as_ref()
                .unwrap()
                .metadata
                .as_ref()
                .unwrap()
                .title,
            "Summer 2"
        );
        assert_eq!(by("N1").promotion.as_ref().unwrap().metadata, None);
        let hotels = list.run_filtered("c1", Some("hotels")).await.unwrap();
        assert_eq!(
            hotels
                .iter()
                .map(|e| e.coupon.code.as_str())
                .collect::<Vec<_>>(),
            vec!["H1"]
        );
        assert!(
            list.run_filtered("c1", Some("transfers"))
                .await
                .unwrap()
                .is_empty()
        );

        // archiving hides the metadata from reads and listings, deleting removes it
        p.catalog.archive_promotion("P2").await.unwrap();
        assert_eq!(p.catalog.get_metadata("P2").await.unwrap(), None);
        assert_eq!(p.catalog.list_metadata().await.unwrap().len(), 1);
        assert!(
            !p.catalog
                .put_metadata("P2", metadata("x", None))
                .await
                .unwrap()
        );
        assert!(p.catalog.delete_metadata("P1").await.unwrap());
        assert_eq!(p.catalog.get_metadata("P1").await.unwrap(), None);
    })
    .await;
}

/// Spec `promotion-catalog`, "Allowlists", and `eligibility-rules`, "Customer attributes and
/// allowlists in the context": storage, enrichment of the customer context, gating.
#[tokio::test]
async fn allowlists_gate_promotions() {
    for_each_backend(|p| async move {
        let clock = FixedClock(NOW);
        p.catalog.put_group(group_g()).await.unwrap();
        for id in ["P1", "P2"] {
            let mut pr = fixed_300(id);
            pr.rules = vec![Rule::RequiresAllowlist];
            p.catalog.put_promotion(pr).await.unwrap();
        }
        // unknown promotion
        assert_eq!(p.catalog.get_allowlist("nope").await.unwrap(), None);
        assert!(
            !p.catalog
                .put_allowlist("nope", vec!["c1".into()])
                .await
                .unwrap()
        );
        assert!(!p.catalog.add_to_allowlist("nope", "c1").await.unwrap());
        assert!(!p.catalog.delete_allowlist("nope").await.unwrap());
        // empty by default; replace, add (idempotent), membership, sorted read-back
        assert_eq!(p.catalog.get_allowlist("P1").await.unwrap(), Some(vec![]));
        assert!(
            p.catalog
                .put_allowlist("P1", vec!["c2".into(), "c1".into(), "c1".into()])
                .await
                .unwrap()
        );
        assert!(p.catalog.add_to_allowlist("P1", "c3").await.unwrap());
        assert!(p.catalog.add_to_allowlist("P1", "c3").await.unwrap());
        assert_eq!(
            p.catalog.get_allowlist("P1").await.unwrap(),
            Some(vec!["c1".to_string(), "c2".to_string(), "c3".to_string()])
        );
        assert!(p.catalog.is_allowlisted("P1", "c1").await.unwrap());
        assert!(!p.catalog.is_allowlisted("P1", "c9").await.unwrap());
        assert!(p.catalog.add_to_allowlist("P2", "c1").await.unwrap());
        assert_eq!(
            p.catalog.allowlisted_for("c1").await.unwrap(),
            vec!["P1".to_string(), "P2".to_string()]
        );
        assert_eq!(
            p.catalog.allowlisted_for("c9").await.unwrap(),
            Vec::<String>::new()
        );

        // evaluation: a listed customer gets both promotions, an unlisted one none, and a
        // caller-supplied allowlisted id is merged with the stored ones
        let evaluate = Evaluate {
            catalog: &*p.catalog,
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        let c1 = CustomerContext {
            customer_id: Some("c1".into()),
            ..CustomerContext::default()
        };
        let c9 = CustomerContext {
            customer_id: Some("c9".into()),
            ..CustomerContext::default()
        };
        assert_eq!(
            evaluate
                .run(sample_cart(), c1.clone())
                .await
                .unwrap()
                .applied
                .len(),
            2
        );
        let r = evaluate.run(sample_cart(), c9.clone()).await.unwrap();
        assert!(r.applied.is_empty());
        assert_eq!(
            r.skipped[0].reason,
            SkipReason::NotEligible { rule_index: 0 }
        );
        let c9b = CustomerContext {
            allowlisted: vec!["P2".into()],
            ..c9.clone()
        };
        let r = evaluate.run(sample_cart(), c9b).await.unwrap();
        assert_eq!(
            r.applied
                .iter()
                .map(|a| a.promotion_id.as_str())
                .collect::<Vec<_>>(),
            vec!["P2"]
        );
        // Redeem enriches too, and the stored replay record shows the merged list
        let o = redeem(&p, &clock)
            .run("al-1", sample_cart(), c1.clone())
            .await
            .unwrap();
        assert_eq!(o.redemption().result.applied.len(), 2);
        assert_eq!(
            o.redemption().input.customer.allowlisted,
            vec!["P1".to_string(), "P2".to_string()]
        );
        // remove, clear, archive
        assert!(p.catalog.remove_from_allowlist("P1", "c1").await.unwrap());
        assert!(!p.catalog.remove_from_allowlist("P1", "c1").await.unwrap());
        assert_eq!(
            evaluate
                .run(sample_cart(), c1.clone())
                .await
                .unwrap()
                .applied
                .len(),
            1
        );
        assert!(p.catalog.delete_allowlist("P1").await.unwrap());
        assert_eq!(p.catalog.get_allowlist("P1").await.unwrap(), Some(vec![]));
        p.catalog.archive_promotion("P2").await.unwrap();
        assert_eq!(p.catalog.get_allowlist("P2").await.unwrap(), None);
        assert_eq!(
            p.catalog.allowlisted_for("c1").await.unwrap(),
            Vec::<String>::new()
        );
        assert!(!p.catalog.is_allowlisted("P2", "c1").await.unwrap());
    })
    .await;
}

/// Spec `access-control`, "Tenant isolation": the same ids in two tenants are different
/// records on both backends, and tenant administration seeds a policy per tenant.
#[tokio::test]
async fn tenants_are_hard_partitions() {
    // in-memory
    {
        let tenancy = discount_infrastructure::InMemoryTenants::new(default_policy());
        check_isolation(&tenancy).await;
    }
    // postgres: the wiped test store, viewed as a tenancy (never skipped, ADR 11)
    let _serial = PG_LOCK.lock().await;
    let store = pg_store().await;
    let tenancy = store.tenants(&default_policy());
    check_isolation(&tenancy).await;
}

async fn check_isolation(tenancy: &dyn Tenancy) {
    let clock = FixedClock(NOW);
    for t in ["hotels", "cars"] {
        tenancy
            .put_tenant(Tenant {
                tenant_id: t.into(),
                name: t.into(),
                created_at: NOW,
            })
            .await
            .unwrap();
    }
    let hotels = tenancy.catalog("hotels");
    let cars = tenancy.catalog("cars");
    hotels.put_group(group_g()).await.unwrap();
    hotels.put_promotion(fixed_300("P1")).await.unwrap();
    hotels.put_budget("P1", Some(eur(1000))).await.unwrap();
    hotels
        .put_coupon(Coupon {
            customer_id: Some("c1".into()),
            ..coupon("WELCOME", None)
        })
        .await
        .unwrap();
    // cars sees none of it and has its own policy row
    assert!(cars.list_promotions().await.unwrap().is_empty());
    assert_eq!(cars.get_promotion("P1").await.unwrap(), None);
    assert_eq!(cars.get_coupon("WELCOME").await.unwrap(), None);
    assert_eq!(cars.list_groups().await.unwrap().len(), 0);
    assert_eq!(cars.get_policy().await.unwrap(), default_policy());
    // the same promotion id in cars is another promotion with another budget
    cars.put_group(group_g()).await.unwrap();
    let mut p1 = fixed_300("P1");
    p1.action = Action::OrderFixedAmount { amount: eur(100) };
    cars.put_promotion(p1).await.unwrap();
    assert_eq!(
        hotels.get_budget("P1").await.unwrap(),
        Some(Some(eur(1000)))
    );
    assert_eq!(cars.get_budget("P1").await.unwrap(), Some(None));
    // redemptions and counters are per tenant
    let h = tenancy.catalog("hotels");
    let hr = tenancy.redemptions("hotels");
    let redeem_h = Redeem {
        catalog: &*h,
        redemptions: &*hr,
        clock: &clock,
        events: &NoopEventPublisher,
    };
    let o = redeem_h
        .run("o1", sample_cart(), CustomerContext::default())
        .await
        .unwrap();
    assert_eq!(o.redemption().result.total_discount.amount, 300);
    let c = tenancy.catalog("cars");
    let cr = tenancy.redemptions("cars");
    let redeem_c = Redeem {
        catalog: &*c,
        redemptions: &*cr,
        clock: &clock,
        events: &NoopEventPublisher,
    };
    let o2 = redeem_c
        .run("o1", sample_cart(), CustomerContext::default())
        .await
        .unwrap();
    assert_eq!(o2.redemption().result.total_discount.amount, 100);
    assert_eq!(
        tenancy
            .redemptions("hotels")
            .get("o1")
            .await
            .unwrap()
            .unwrap()
            .result
            .total_discount
            .amount,
        300
    );
    assert_eq!(
        tenancy.catalog("hotels").get_budget("P1").await.unwrap(),
        Some(Some(eur(700)))
    );
    // deleting a tenant with redemptions is refused; an empty one goes
    assert!(matches!(
        tenancy.delete_tenant("hotels").await,
        Err(StorageError::Conflict(_))
    ));
    tenancy
        .put_tenant(Tenant {
            tenant_id: "transfers".into(),
            name: "T".into(),
            created_at: NOW,
        })
        .await
        .unwrap();
    assert!(tenancy.delete_tenant("transfers").await.unwrap());
    assert!(!tenancy.delete_tenant("transfers").await.unwrap());
    let ids: Vec<String> = tenancy
        .list_tenants()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.tenant_id)
        .collect();
    assert!(
        ids.contains(&"hotels".to_string())
            && ids.contains(&"cars".to_string())
            && ids.contains(&"default".to_string())
    );
}

/// Spec `catalogue-states`: paused promotions and coupons leave the pricing snapshot, stay
/// in the listings, come back on resume; generation and grants refuse paused promotions;
/// the customer listing shows `paused`.
#[tokio::test]
async fn paused_records_leave_the_snapshot() {
    for_each_backend(|p| async move {
        let clock = FixedClock(NOW);
        p.catalog.put_group(group_g()).await.unwrap();
        p.catalog.put_promotion(fixed_300("P1")).await.unwrap();
        let mut p2 = fixed_300("P2");
        p2.rules = vec![Rule::RequiresCoupon];
        p.catalog.put_promotion(p2).await.unwrap();
        p.catalog.put_coupon(coupon("WELCOME", None)).await.unwrap();
        assert_eq!(
            p.catalog.get_promotion_state("P1").await.unwrap(),
            Some(CatalogState::Active)
        );
        assert_eq!(
            p.catalog.get_coupon_state("WELCOME").await.unwrap(),
            Some(CatalogState::Active)
        );
        assert_eq!(p.catalog.get_promotion_state("nope").await.unwrap(), None);
        assert!(
            !p.catalog
                .set_promotion_state("nope", CatalogState::Paused)
                .await
                .unwrap()
        );
        assert!(
            !p.catalog
                .set_coupon_state("nope", CatalogState::Paused)
                .await
                .unwrap()
        );
        let evaluate = Evaluate {
            catalog: &*p.catalog,
            redemptions: &*p.redemptions,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        let r = evaluate
            .run(cart_with("WELCOME"), CustomerContext::default())
            .await
            .unwrap();
        assert_eq!(r.applied.len(), 2);
        // pause P1: gone from the snapshot and from applied/skipped, still listed
        assert!(
            p.catalog
                .set_promotion_state("P1", CatalogState::Paused)
                .await
                .unwrap()
        );
        let r = evaluate
            .run(cart_with("WELCOME"), CustomerContext::default())
            .await
            .unwrap();
        assert_eq!(
            r.applied
                .iter()
                .map(|a| a.promotion_id.as_str())
                .collect::<Vec<_>>(),
            vec!["P2"]
        );
        assert!(r.skipped.iter().all(|s| s.promotion_id != "P1"));
        assert_eq!(p.catalog.list_promotions().await.unwrap().len(), 2);
        assert_eq!(p.catalog.snapshot().await.unwrap().promotions.len(), 1);
        assert!(p.catalog.get_promotion("P1").await.unwrap().is_some());
        // a PUT of the body does not resume it
        p.catalog.put_promotion(fixed_300("P1")).await.unwrap();
        assert_eq!(
            p.catalog.get_promotion_state("P1").await.unwrap(),
            Some(CatalogState::Paused)
        );
        // pause the coupon: P2 is NotEligible on RequiresCoupon
        assert!(
            p.catalog
                .set_coupon_state("WELCOME", CatalogState::Paused)
                .await
                .unwrap()
        );
        let r = evaluate
            .run(cart_with("WELCOME"), CustomerContext::default())
            .await
            .unwrap();
        assert!(r.applied.is_empty());
        assert_eq!(
            r.skipped[0].reason,
            SkipReason::NotEligible { rule_index: 0 }
        );
        assert_eq!(p.catalog.list_coupons().await.unwrap().len(), 1);
        // generation and grants refuse a paused promotion
        let codes = discount_infrastructure::SequentialCodes::default();
        let generate = GenerateCouponsUseCase {
            catalog: &*p.catalog,
            codes: &codes,
        };
        let req = GenerateCoupons {
            promotion_id: "P1".into(),
            count: 1,
            prefix: "X-".into(),
            starts_at: None,
            ends_at: None,
            max_uses: None,
            customer_id: None,
        };
        assert_eq!(generate.run(req).await, Err(CouponError::PromotionPaused));
        p.catalog
            .put_grant_rule(GrantRule {
                rule_id: "r".into(),
                trigger: GrantTrigger::Cancel,
                promotion_id: "P1".into(),
                priority: 1,
                validity_seconds: 60,
                max_uses: None,
            })
            .await
            .unwrap();
        let grant = GrantCoupon {
            catalog: &*p.catalog,
            codes: &codes,
            clock: &clock,
        };
        assert_eq!(
            grant.run(GrantTrigger::Cancel, "c1", "o1").await,
            Err(CouponError::PromotionPaused)
        );
        // the customer listing shows paused
        p.catalog
            .put_coupon(Coupon {
                customer_id: Some("c1".into()),
                promotion_id: "P1".into(),
                ..coupon("MINE", None)
            })
            .await
            .unwrap();
        let list = ListCustomerCoupons {
            catalog: &*p.catalog,
            redemptions: &*p.redemptions,
            clock: &clock,
        };
        assert_eq!(
            list.run("c1").await.unwrap()[0].status,
            CouponStatus::Paused
        );
        // resume both
        assert!(
            p.catalog
                .set_promotion_state("P1", CatalogState::Active)
                .await
                .unwrap()
        );
        assert!(
            p.catalog
                .set_coupon_state("WELCOME", CatalogState::Active)
                .await
                .unwrap()
        );
        let r = evaluate
            .run(cart_with("WELCOME"), CustomerContext::default())
            .await
            .unwrap();
        assert_eq!(r.applied.len(), 2);
        assert_eq!(
            list.run("c1").await.unwrap()[0].status,
            CouponStatus::Active
        );
        // archived promotions have no state
        p.catalog.archive_promotion("P1").await.unwrap();
        assert_eq!(p.catalog.get_promotion_state("P1").await.unwrap(), None);
        assert!(
            !p.catalog
                .set_promotion_state("P1", CatalogState::Paused)
                .await
                .unwrap()
        );
    })
    .await;
}
