//! The `redemption` and `promotion-catalog` scenarios, run against the in-memory store and,
//! when `DATABASE_URL` is set, against Postgres.

mod common;

use std::sync::Arc;

use common::*;
use discount_application::ports::{
    PromotionCatalog, RedemptionError, RedemptionRepository, RedemptionStatus, ReserveOutcome,
};
use discount_application::{CancelRedemption, ConfirmRedemption, Evaluate, Redeem};
use discount_domain::*;
use discount_infrastructure::{FixedClock, NoopEventPublisher, PgStore, default_policy};

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

async fn postgres_ports() -> Option<Ports> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let store = PgStore::connect(&url, &default_policy())
        .await
        .expect("connect to DATABASE_URL");
    store.reset_for_tests(&default_policy()).await.unwrap();
    Some(Ports {
        catalog: Arc::new(store.clone()),
        redemptions: Arc::new(store),
    })
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
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("DATABASE_URL not set: Postgres scenarios skipped");
        return;
    }
    let _serial = PG_LOCK.lock().await;
    let p = postgres_ports().await.expect("postgres");
    f(p).await;
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
        // stored
        let stored = p.redemptions.get("order-1").await.unwrap().unwrap();
        assert_eq!(stored.result, r1.result);
        assert_eq!(stored.input.promotions[0].remaining_budget, Some(eur(1000)));
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
