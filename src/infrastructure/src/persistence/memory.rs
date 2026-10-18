//! In-memory port implementations: one store behind one mutex implements both
//! `PromotionCatalog` and `RedemptionRepository`, so a reservation sees and updates budgets
//! atomically exactly like the Postgres transaction does.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use discount_application::CodeSource;
use discount_application::coupons::CODE_LENGTH;
use discount_application::ports::{
    CatalogSnapshot, Clock, Counters, Event, EventPublisher, GrantRule, GrantTrigger, PricingFn,
    PromotionCatalog, PromotionMetadata, Redemption, RedemptionError, RedemptionRepository,
    RedemptionStatus, ReserveOutcome, StorageError, Uses, transition_allowed,
};
use discount_domain::{
    Coupon, ExchangeRate, Money, OrderCap, PricingPolicy, Promotion, PromotionId, RoundingPolicy,
    StackingGroup, StackingPolicy, Timestamp,
};
use serde::{Deserialize, Serialize};

/// `{"groups": [...], "promotions": [...]}` in the ubiquitous-language encoding.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogFixture {
    pub groups: Vec<StackingGroup>,
    pub promotions: Vec<Promotion>,
    #[serde(default)]
    pub coupons: Vec<Coupon>,
    #[serde(default)]
    pub exchange_rates: Vec<ExchangeRate>,
}

impl CatalogFixture {
    pub fn from_json_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("{}: {e}", path.as_ref().display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.as_ref().display()))
    }
}

#[derive(Debug, Clone)]
struct StoredPromotion {
    body: Promotion,
    archived: bool,
}

#[derive(Debug, Default)]
struct Store {
    policy: Option<PricingPolicy>,
    groups: BTreeMap<String, StackingGroup>,
    promotions: BTreeMap<String, StoredPromotion>,
    budgets: BTreeMap<String, Money>,
    coupons: BTreeMap<String, Coupon>,
    rates: BTreeMap<(String, String), ExchangeRate>,
    redemptions: BTreeMap<String, Redemption>,
    grant_rules: BTreeMap<String, GrantRule>,
    grants: BTreeMap<(String, String), String>,
    saved: std::collections::BTreeSet<(String, String)>,
    metadata: BTreeMap<String, PromotionMetadata>,
}

/// In-memory catalogue and redemption store.
#[derive(Clone, Default)]
pub struct InMemoryStore {
    inner: Arc<Mutex<Store>>,
}

pub fn default_policy() -> PricingPolicy {
    PricingPolicy {
        rounding: RoundingPolicy::Down,
        stacking: StackingPolicy::Sequential,
        order_cap: OrderCap {
            amount: None,
            bps: None,
        },
    }
}

impl InMemoryStore {
    pub fn new(policy: PricingPolicy) -> InMemoryStore {
        let store = InMemoryStore::default();
        store.inner.lock().unwrap().policy = Some(policy);
        store
    }

    /// Seed groups, promotions and their budgets (`remaining_budget` in the fixture becomes
    /// the budget row) from a fixture.
    pub fn with_fixture(self, fixture: CatalogFixture) -> InMemoryStore {
        {
            let mut s = self.inner.lock().unwrap();
            for g in fixture.groups {
                s.groups.insert(g.group_id.clone(), g);
            }
            for mut p in fixture.promotions {
                if let Some(b) = p.remaining_budget.take() {
                    s.budgets.insert(p.promotion_id.clone(), b);
                }
                s.promotions.insert(
                    p.promotion_id.clone(),
                    StoredPromotion {
                        body: p,
                        archived: false,
                    },
                );
            }
            for c in fixture.coupons {
                s.coupons.insert(c.code.clone(), c);
            }
            for r in fixture.exchange_rates {
                s.rates.insert((r.from.clone(), r.to.clone()), r);
            }
        }
        self
    }

    fn snapshot_locked(s: &Store) -> CatalogSnapshot {
        let mut snapshot = CatalogSnapshot {
            policy: s.policy.clone().unwrap_or_else(default_policy),
            groups: s.groups.values().cloned().collect(),
            promotions: s
                .promotions
                .values()
                .filter(|p| !p.archived)
                .map(|p| {
                    let mut body = p.body.clone();
                    body.remaining_budget = s.budgets.get(&body.promotion_id).cloned();
                    body
                })
                .collect(),
            coupons: s.coupons.values().cloned().collect(),
            exchange_rates: s.rates.values().cloned().collect(),
        };
        InMemoryStore::counters_locked(s, None).apply_to(&mut snapshot);
        snapshot
    }

    /// Spec `redemption`, "Counting uses per coupon and per promotion": over reserved and
    /// confirmed redemptions, per promotion the redemptions that applied it; per coupon code
    /// (all stored codes, or only `codes`) the redemptions that presented it and applied its
    /// promotion.
    fn counters_locked(s: &Store, codes: Option<&[String]>) -> Counters {
        let mut out = Counters::default();
        for r in s.redemptions.values() {
            if r.status == RedemptionStatus::Cancelled {
                continue;
            }
            for a in &r.result.applied {
                bump(&mut out.promotion_uses, &a.promotion_id);
            }
            for code in &r.input.cart.coupon_codes {
                if codes.is_some_and(|cs| !cs.contains(code)) {
                    continue;
                }
                let Some(c) = s.coupons.get(code) else {
                    continue;
                };
                if r.result
                    .applied
                    .iter()
                    .any(|a| a.promotion_id == c.promotion_id)
                {
                    bump(&mut out.coupon_uses, code);
                }
            }
        }
        out.promotion_uses.sort();
        out.coupon_uses.sort();
        out
    }

    fn coupon_with_uses(s: &Store, code: &str) -> Option<Coupon> {
        let mut c = s.coupons.get(code).cloned()?;
        c.uses = InMemoryStore::counters_locked(s, Some(&[code.to_string()])).coupon(code);
        Some(c)
    }
}

fn bump(v: &mut Vec<(String, i64)>, key: &str) {
    match v.iter_mut().find(|(k, _)| k == key) {
        Some(e) => e.1 += 1,
        None => v.push((key.to_string(), 1)),
    }
}

impl InMemoryStore {
    fn uses_locked(s: &Store, customer_id: &str) -> Uses {
        let mut out: Uses = Vec::new();
        for r in s.redemptions.values() {
            if r.status == RedemptionStatus::Cancelled
                || r.customer_id.as_deref() != Some(customer_id)
            {
                continue;
            }
            for a in &r.result.applied {
                match out.iter_mut().find(|(pid, _)| *pid == a.promotion_id) {
                    Some(e) => e.1 += 1,
                    None => out.push((a.promotion_id.clone(), 1)),
                }
            }
        }
        out.sort();
        out
    }
}

#[async_trait]
impl PromotionCatalog for InMemoryStore {
    async fn snapshot(&self) -> Result<CatalogSnapshot, StorageError> {
        Ok(InMemoryStore::snapshot_locked(&self.inner.lock().unwrap()))
    }
    async fn get_policy(&self) -> Result<PricingPolicy, StorageError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .policy
            .clone()
            .unwrap_or_else(default_policy))
    }
    async fn put_policy(&self, policy: PricingPolicy) -> Result<(), StorageError> {
        self.inner.lock().unwrap().policy = Some(policy);
        Ok(())
    }
    async fn list_groups(&self) -> Result<Vec<StackingGroup>, StorageError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .groups
            .values()
            .cloned()
            .collect())
    }
    async fn get_group(&self, group_id: &str) -> Result<Option<StackingGroup>, StorageError> {
        Ok(self.inner.lock().unwrap().groups.get(group_id).cloned())
    }
    async fn put_group(&self, group: StackingGroup) -> Result<(), StorageError> {
        self.inner
            .lock()
            .unwrap()
            .groups
            .insert(group.group_id.clone(), group);
        Ok(())
    }
    async fn delete_group(&self, group_id: &str) -> Result<bool, StorageError> {
        Ok(self.inner.lock().unwrap().groups.remove(group_id).is_some())
    }
    async fn list_promotions(&self) -> Result<Vec<Promotion>, StorageError> {
        Ok(InMemoryStore::snapshot_locked(&self.inner.lock().unwrap()).promotions)
    }
    async fn get_promotion(&self, promotion_id: &str) -> Result<Option<Promotion>, StorageError> {
        let s = self.inner.lock().unwrap();
        Ok(s.promotions
            .get(promotion_id)
            .filter(|p| !p.archived)
            .map(|p| {
                let mut body = p.body.clone();
                body.remaining_budget = s.budgets.get(promotion_id).cloned();
                body
            }))
    }
    async fn put_promotion(&self, mut promotion: Promotion) -> Result<(), StorageError> {
        promotion.remaining_budget = None;
        self.inner.lock().unwrap().promotions.insert(
            promotion.promotion_id.clone(),
            StoredPromotion {
                body: promotion,
                archived: false,
            },
        );
        Ok(())
    }
    async fn archive_promotion(&self, promotion_id: &str) -> Result<bool, StorageError> {
        let mut s = self.inner.lock().unwrap();
        match s.promotions.get_mut(promotion_id) {
            Some(p) if !p.archived => {
                p.archived = true;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    async fn get_budget(&self, promotion_id: &str) -> Result<Option<Option<Money>>, StorageError> {
        let s = self.inner.lock().unwrap();
        if !s.promotions.contains_key(promotion_id) {
            return Ok(None);
        }
        Ok(Some(s.budgets.get(promotion_id).cloned()))
    }
    async fn put_budget(
        &self,
        promotion_id: &str,
        remaining: Option<Money>,
    ) -> Result<bool, StorageError> {
        let mut s = self.inner.lock().unwrap();
        if !s.promotions.contains_key(promotion_id) {
            return Ok(false);
        }
        match remaining {
            Some(m) => s.budgets.insert(promotion_id.to_string(), m),
            None => s.budgets.remove(promotion_id),
        };
        Ok(true)
    }
    async fn list_budgets(&self) -> Result<Vec<(PromotionId, Money)>, StorageError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .budgets
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
    async fn list_coupons(&self) -> Result<Vec<Coupon>, StorageError> {
        Ok(InMemoryStore::snapshot_locked(&self.inner.lock().unwrap()).coupons)
    }
    async fn get_coupon(&self, code: &str) -> Result<Option<Coupon>, StorageError> {
        Ok(InMemoryStore::coupon_with_uses(
            &self.inner.lock().unwrap(),
            code,
        ))
    }
    async fn put_coupon(&self, coupon: Coupon) -> Result<(), StorageError> {
        self.inner
            .lock()
            .unwrap()
            .coupons
            .insert(coupon.code.clone(), coupon);
        Ok(())
    }
    async fn delete_coupon(&self, code: &str) -> Result<bool, StorageError> {
        Ok(self.inner.lock().unwrap().coupons.remove(code).is_some())
    }
    async fn list_rates(&self) -> Result<Vec<ExchangeRate>, StorageError> {
        Ok(self.inner.lock().unwrap().rates.values().cloned().collect())
    }
    async fn put_rate(&self, rate: ExchangeRate) -> Result<(), StorageError> {
        self.inner
            .lock()
            .unwrap()
            .rates
            .insert((rate.from.clone(), rate.to.clone()), rate);
        Ok(())
    }
    async fn delete_rate(&self, from: &str, to: &str) -> Result<bool, StorageError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .rates
            .remove(&(from.to_string(), to.to_string()))
            .is_some())
    }
    async fn list_grant_rules(&self) -> Result<Vec<GrantRule>, StorageError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .grant_rules
            .values()
            .cloned()
            .collect())
    }
    async fn get_grant_rule(&self, rule_id: &str) -> Result<Option<GrantRule>, StorageError> {
        Ok(self.inner.lock().unwrap().grant_rules.get(rule_id).cloned())
    }
    async fn put_grant_rule(&self, rule: GrantRule) -> Result<(), StorageError> {
        self.inner
            .lock()
            .unwrap()
            .grant_rules
            .insert(rule.rule_id.clone(), rule);
        Ok(())
    }
    async fn delete_grant_rule(&self, rule_id: &str) -> Result<bool, StorageError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .grant_rules
            .remove(rule_id)
            .is_some())
    }
    async fn put_coupons(&self, coupons: Vec<Coupon>) -> Result<(), StorageError> {
        let mut s = self.inner.lock().unwrap();
        if let Some(c) = coupons.iter().find(|c| s.coupons.contains_key(&c.code)) {
            return Err(StorageError::Unavailable(format!(
                "coupon {} already exists",
                c.code
            )));
        }
        for c in coupons {
            s.coupons.insert(c.code.clone(), c);
        }
        Ok(())
    }
    async fn get_grant(
        &self,
        trigger: GrantTrigger,
        order_id: &str,
    ) -> Result<Option<Coupon>, StorageError> {
        let s = self.inner.lock().unwrap();
        Ok(s.grants
            .get(&(trigger.as_str().to_string(), order_id.to_string()))
            .and_then(|code| InMemoryStore::coupon_with_uses(&s, code)))
    }
    async fn record_grant(
        &self,
        trigger: GrantTrigger,
        order_id: &str,
        coupon: Coupon,
        _now: Timestamp,
    ) -> Result<Coupon, StorageError> {
        let mut s = self.inner.lock().unwrap();
        let key = (trigger.as_str().to_string(), order_id.to_string());
        if let Some(code) = s.grants.get(&key).cloned() {
            return Ok(InMemoryStore::coupon_with_uses(&s, &code).unwrap_or(coupon));
        }
        s.grants.insert(key, coupon.code.clone());
        s.coupons.insert(coupon.code.clone(), coupon.clone());
        Ok(coupon)
    }

    async fn coupons_bound_to(&self, customer_id: &str) -> Result<Vec<Coupon>, StorageError> {
        let s = self.inner.lock().unwrap();
        let counters = InMemoryStore::counters_locked(&s, None);
        Ok(s.coupons
            .values()
            .filter(|c| c.customer_id.as_deref() == Some(customer_id))
            .map(|c| {
                let mut c = c.clone();
                c.uses = counters.coupon(&c.code);
                c
            })
            .collect())
    }
    async fn saved_codes(&self, customer_id: &str) -> Result<Vec<String>, StorageError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .saved
            .iter()
            .filter(|(c, _)| c == customer_id)
            .map(|(_, code)| code.clone())
            .collect())
    }
    async fn save_coupon(&self, customer_id: &str, code: &str) -> Result<(), StorageError> {
        self.inner
            .lock()
            .unwrap()
            .saved
            .insert((customer_id.to_string(), code.to_string()));
        Ok(())
    }
    async fn forget_coupon(&self, customer_id: &str, code: &str) -> Result<bool, StorageError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .saved
            .remove(&(customer_id.to_string(), code.to_string())))
    }

    async fn get_metadata(
        &self,
        promotion_id: &str,
    ) -> Result<Option<PromotionMetadata>, StorageError> {
        let s = self.inner.lock().unwrap();
        match s.promotions.get(promotion_id) {
            Some(p) if !p.archived => Ok(s.metadata.get(promotion_id).cloned()),
            _ => Ok(None),
        }
    }
    async fn put_metadata(
        &self,
        promotion_id: &str,
        metadata: PromotionMetadata,
    ) -> Result<bool, StorageError> {
        let mut s = self.inner.lock().unwrap();
        match s.promotions.get(promotion_id) {
            Some(p) if !p.archived => {
                s.metadata.insert(promotion_id.to_string(), metadata);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    async fn delete_metadata(&self, promotion_id: &str) -> Result<bool, StorageError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .metadata
            .remove(promotion_id)
            .is_some())
    }
    async fn list_metadata(&self) -> Result<Vec<(PromotionId, PromotionMetadata)>, StorageError> {
        let s = self.inner.lock().unwrap();
        Ok(s.metadata
            .iter()
            .filter(|(pid, _)| s.promotions.get(*pid).is_some_and(|p| !p.archived))
            .map(|(pid, m)| (pid.clone(), m.clone()))
            .collect())
    }
}

/// Budget bookkeeping shared with the Postgres adapter's logic: what each applied promotion
/// took, for promotions that have a budget.
pub fn budget_moves(
    result: &discount_domain::PricingResult,
    budgets: &BTreeMap<String, Money>,
) -> Vec<(String, i64)> {
    result
        .applied
        .iter()
        .filter(|a| budgets.contains_key(&a.promotion_id))
        .map(|a| (a.promotion_id.clone(), a.discount.amount))
        .collect()
}

#[async_trait]
impl RedemptionRepository for InMemoryStore {
    async fn reserve(
        &self,
        order_id: &str,
        customer_id: Option<&str>,
        _codes: &[String],
        now: Timestamp,
        price: &PricingFn<'_>,
    ) -> Result<ReserveOutcome, RedemptionError> {
        let mut s = self.inner.lock().unwrap();
        if let Some(existing) = s.redemptions.get(order_id)
            && existing.status != RedemptionStatus::Cancelled
        {
            return Ok(ReserveOutcome::Idempotent(existing.clone()));
        }
        let snapshot = InMemoryStore::snapshot_locked(&s);
        let uses = customer_id
            .map(|c| InMemoryStore::uses_locked(&s, c))
            .unwrap_or_default();
        let (input, result) = price(snapshot, uses).map_err(RedemptionError::Pricing)?;
        for (pid, amount) in budget_moves(&result, &s.budgets) {
            if let Some(b) = s.budgets.get_mut(&pid) {
                b.amount -= amount;
            }
        }
        let r = Redemption {
            order_id: order_id.to_string(),
            customer_id: customer_id.map(str::to_string),
            status: RedemptionStatus::Reserved,
            input,
            result,
            created_at: now,
            updated_at: now,
        };
        s.redemptions.insert(order_id.to_string(), r.clone());
        Ok(ReserveOutcome::Reserved(r))
    }

    async fn transition(
        &self,
        order_id: &str,
        to: RedemptionStatus,
        now: Timestamp,
    ) -> Result<Redemption, RedemptionError> {
        let mut s = self.inner.lock().unwrap();
        let current = s
            .redemptions
            .get(order_id)
            .cloned()
            .ok_or(RedemptionError::NotFound)?;
        if !transition_allowed(current.status, to) {
            return Err(RedemptionError::InvalidTransition {
                from: current.status,
                to,
            });
        }
        if to == RedemptionStatus::Cancelled {
            for (pid, amount) in budget_moves(&current.result, &s.budgets) {
                if let Some(b) = s.budgets.get_mut(&pid) {
                    b.amount += amount;
                }
            }
        }
        let r = s.redemptions.get_mut(order_id).unwrap();
        r.status = to;
        r.updated_at = now;
        Ok(r.clone())
    }

    async fn uses_for_customer(&self, customer_id: &str) -> Result<Uses, RedemptionError> {
        Ok(InMemoryStore::uses_locked(
            &self.inner.lock().unwrap(),
            customer_id,
        ))
    }

    async fn counters(&self, codes: &[String]) -> Result<Counters, RedemptionError> {
        Ok(InMemoryStore::counters_locked(
            &self.inner.lock().unwrap(),
            Some(codes),
        ))
    }

    async fn coupon_uses_of_customer(
        &self,
        customer_id: &str,
    ) -> Result<Vec<(String, String)>, RedemptionError> {
        let s = self.inner.lock().unwrap();
        let mut rs: Vec<&Redemption> = s
            .redemptions
            .values()
            .filter(|r| {
                r.status != RedemptionStatus::Cancelled
                    && r.customer_id.as_deref() == Some(customer_id)
            })
            .collect();
        rs.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.order_id.cmp(&b.order_id))
        });
        let mut out = Vec::new();
        for r in rs {
            for code in &r.input.cart.coupon_codes {
                let Some(c) = s.coupons.get(code) else {
                    continue;
                };
                if r.result
                    .applied
                    .iter()
                    .any(|a| a.promotion_id == c.promotion_id)
                {
                    out.push((code.clone(), r.order_id.clone()));
                }
            }
        }
        Ok(out)
    }

    async fn get(&self, order_id: &str) -> Result<Option<Redemption>, RedemptionError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .redemptions
            .get(order_id)
            .cloned())
    }
}

pub struct SystemClock;

/// Codes from the standard library's randomly keyed hasher over a counter: unpredictable
/// enough for coupon codes, no extra dependency.
#[derive(Default)]
pub struct RandomCodes {
    counter: std::sync::atomic::AtomicU64,
    keys: std::collections::hash_map::RandomState,
}

impl CodeSource for RandomCodes {
    fn next_code(&self) -> [u8; CODE_LENGTH] {
        use std::hash::{BuildHasher, Hasher};
        let n = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut h = self.keys.build_hasher();
        h.write_u64(n);
        h.write_u128(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        let mut bits = h.finish();
        let mut out = [0u8; CODE_LENGTH];
        for b in &mut out {
            *b = (bits & 31) as u8;
            bits >>= 5;
        }
        out
    }
}

/// Fixed sequence of codes for tests: `AAAAAAAA`, `AAAAAAAB`, ...
#[derive(Default)]
pub struct SequentialCodes(std::sync::atomic::AtomicU64);

impl CodeSource for SequentialCodes {
    fn next_code(&self) -> [u8; CODE_LENGTH] {
        let mut n = self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut out = [0u8; CODE_LENGTH];
        for b in out.iter_mut().rev() {
            *b = (n & 31) as u8;
            n >>= 5;
        }
        out
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

/// A clock that always answers the same instant; for tests.
pub struct FixedClock(pub Timestamp);

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}

pub struct NoopEventPublisher;

#[async_trait]
impl EventPublisher for NoopEventPublisher {
    async fn publish(&self, _event: Event) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use discount_application::Evaluate;
    use discount_domain::*;

    fn eur(a: i64) -> Money {
        Money::new(a, "EUR")
    }

    #[tokio::test]
    async fn fixture_file_prices_sample_cart() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../deploy/promotions.json");
        let fixture = CatalogFixture::from_json_file(path).expect("fixture loads");
        assert_eq!(fixture.groups.len(), 1);
        assert_eq!(fixture.promotions.len(), 3);
        let store = InMemoryStore::new(default_policy()).with_fixture(fixture);
        let clock = FixedClock(1_760_227_200);
        let use_case = Evaluate {
            catalog: &store,
            redemptions: &store,
            clock: &clock,
            events: &NoopEventPublisher,
        };
        let cart = CartSnapshot {
            currency: "EUR".into(),
            lines: vec![
                CartLine {
                    line_id: "L1".into(),
                    unit_price: eur(1999),
                    quantity: 2,
                },
                CartLine {
                    line_id: "L2".into(),
                    unit_price: eur(501),
                    quantity: 1,
                },
            ],
            tags: vec![],
            attributes: vec![],
            coupon_codes: vec![],
        };
        let r = use_case
            .run(cart, CustomerContext::default())
            .await
            .unwrap();
        assert_eq!(r.total_discount.amount, 749);
        assert_eq!(r.final_total.amount, 3750);
    }
}
