//! Ports: what the application needs from the outside world (ADR 7). Async so that the
//! Postgres adapter can implement them; `async-trait` keeps them object-safe.

use std::sync::Arc;

use async_trait::async_trait;
use discount_domain::{
    Coupon, ExchangeRate, Money, PricingError, PricingInput, PricingPolicy, PricingResult,
    Promotion, PromotionId, StackingGroup, Timestamp,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    Unavailable(String),
    /// The operation contradicts stored state (a tenant with redemptions cannot be deleted).
    Conflict(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Unavailable(m) => write!(f, "storage unavailable: {m}"),
            StorageError::Conflict(m) => write!(f, "conflict: {m}"),
        }
    }
}

impl std::error::Error for StorageError {}

pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// Domain events emitted after a use case completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Evaluated {
        result: PricingResult,
    },
    Redeemed {
        order_id: String,
        result: PricingResult,
    },
    Confirmed {
        order_id: String,
    },
    Cancelled {
        order_id: String,
    },
}

#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish(&self, event: Event);
}

/// Everything pricing needs from the catalogue at one instant: the policy, all groups,
/// all non-archived promotions with their `remaining_budget` filled in, every coupon and
/// every exchange rate. `pricing_input` narrows the coupons to the presented codes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogSnapshot {
    pub policy: PricingPolicy,
    pub groups: Vec<StackingGroup>,
    pub promotions: Vec<Promotion>,
    pub coupons: Vec<Coupon>,
    pub exchange_rates: Vec<ExchangeRate>,
}

/// Use counts of one customer: reserved and confirmed redemptions per applied promotion.
pub type Uses = Vec<(PromotionId, i64)>;

/// A tenant: one consumer system with its own catalogue and redemptions (OpenSpec
/// `access-control`). Every store handle is bound to exactly one tenant.
pub type TenantId = String;

/// The tenant used without authentication and for data that predates tenancy.
pub const DEFAULT_TENANT: &str = "default";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tenant {
    pub tenant_id: TenantId,
    pub name: String,
    pub created_at: Timestamp,
}

impl Tenant {
    /// `^[a-z][a-z0-9_-]{0,63}$`
    pub fn valid_id(s: &str) -> bool {
        let mut chars = s.chars();
        matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
            && s.len() <= 64
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    }
}

/// Who is calling: the tenant the request runs in, the scopes the token carries and the
/// token subject (for logs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub tenant_id: TenantId,
    pub scopes: Vec<String>,
    pub subject: String,
}

impl Principal {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope || s == SCOPE_ADMIN)
    }
}

pub const SCOPE_PRICING: &str = "discount:pricing";
pub const SCOPE_CUSTOMERS: &str = "discount:customers";
pub const SCOPE_CATALOG_READ: &str = "discount:catalog.read";
pub const SCOPE_CATALOG_WRITE: &str = "discount:catalog.write";
pub const SCOPE_ADMIN: &str = "discount:admin";
pub const ALL_SCOPES: [&str; 5] = [
    SCOPE_PRICING,
    SCOPE_CUSTOMERS,
    SCOPE_CATALOG_READ,
    SCOPE_CATALOG_WRITE,
    SCOPE_ADMIN,
];

/// Hands out store handles bound to one tenant and administers tenants.
#[async_trait]
pub trait Tenancy: Send + Sync {
    fn catalog(&self, tenant_id: &str) -> Arc<dyn PromotionCatalog>;
    fn redemptions(&self, tenant_id: &str) -> Arc<dyn RedemptionRepository>;
    async fn list_tenants(&self) -> Result<Vec<Tenant>, StorageError>;
    async fn get_tenant(&self, tenant_id: &str) -> Result<Option<Tenant>, StorageError>;
    /// Upsert; a new tenant starts with the service's default pricing policy.
    async fn put_tenant(&self, tenant: Tenant) -> Result<(), StorageError>;
    /// `Ok(false)` when unknown; `Err(StorageError::Conflict)` when the tenant still has
    /// redemptions.
    async fn delete_tenant(&self, tenant_id: &str) -> Result<bool, StorageError>;
}

/// Everything the counters of a snapshot are derived from: per promotion, how many reserved
/// or confirmed redemptions applied it; per coupon code, how many of them presented the code
/// and applied the coupon's promotion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counters {
    pub promotion_uses: Vec<(PromotionId, i64)>,
    pub coupon_uses: Vec<(String, i64)>,
}

impl Counters {
    pub fn promotion(&self, pid: &str) -> i64 {
        self.promotion_uses
            .iter()
            .find(|(p, _)| p == pid)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    }
    pub fn coupon(&self, code: &str) -> i64 {
        self.coupon_uses
            .iter()
            .find(|(c, _)| c == code)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    }
    /// Fill `uses_total` and `uses` into a snapshot.
    pub fn apply_to(&self, snapshot: &mut CatalogSnapshot) {
        for p in &mut snapshot.promotions {
            p.uses_total = self.promotion(&p.promotion_id);
        }
        for c in &mut snapshot.coupons {
            c.uses = self.coupon(&c.code);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GrantTrigger {
    Cancel,
    Review,
}

impl GrantTrigger {
    pub fn as_str(&self) -> &'static str {
        match self {
            GrantTrigger::Cancel => "cancel",
            GrantTrigger::Review => "review",
        }
    }
}

/// How a coupon is issued when an event happens (spec `coupon-lifecycle`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantRule {
    pub rule_id: String,
    pub trigger: GrantTrigger,
    pub promotion_id: PromotionId,
    pub priority: i64,
    pub validity_seconds: i64,
    pub max_uses: Option<i64>,
}

/// What a promotion is called and how it is reported; never read by pricing
/// (OpenSpec `promotion-metadata`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionMetadata {
    pub title: String,
    pub description: String,
    pub source: Option<String>,
    pub labels: std::collections::BTreeMap<String, String>,
}

impl PromotionMetadata {
    pub const MAX_TITLE: usize = 200;
    pub const MAX_DESCRIPTION: usize = 2000;
    pub const MAX_LABELS: usize = 20;
    pub const MAX_LABEL_KEY: usize = 64;
    pub const MAX_LABEL_VALUE: usize = 256;

    /// `^[a-z][a-z0-9_-]{0,31}$`
    pub fn valid_source(s: &str) -> bool {
        let mut chars = s.chars();
        matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
            && s.len() <= 32
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    }

    /// The limits of the spec; `Err` names the offending field.
    pub fn check(&self) -> Result<(), String> {
        let n = |s: &str| s.chars().count();
        if n(&self.title) == 0 || n(&self.title) > Self::MAX_TITLE {
            return Err("title must be 1..=200 characters".into());
        }
        if n(&self.description) > Self::MAX_DESCRIPTION {
            return Err("description must be at most 2000 characters".into());
        }
        if let Some(src) = &self.source
            && !Self::valid_source(src)
        {
            return Err("source must match ^[a-z][a-z0-9_-]{0,31}$".into());
        }
        if self.labels.len() > Self::MAX_LABELS {
            return Err("labels must have at most 20 entries".into());
        }
        for (k, v) in &self.labels {
            if n(k) == 0 || n(k) > Self::MAX_LABEL_KEY {
                return Err(format!("label key '{k}' must be 1..=64 characters"));
            }
            if n(v) > Self::MAX_LABEL_VALUE {
                return Err(format!("label '{k}' value must be at most 256 characters"));
            }
        }
        Ok(())
    }
}

/// Request to generate a batch of coupon codes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateCoupons {
    pub promotion_id: PromotionId,
    pub count: u32,
    pub prefix: String,
    pub starts_at: Option<Timestamp>,
    pub ends_at: Option<Timestamp>,
    pub max_uses: Option<i64>,
    pub customer_id: Option<String>,
}

/// Where exchange rates are fetched from and how often (spec `exchange-rate-source`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeRateSource {
    pub url: String,
    pub interval_seconds: i64,
    /// Rates older than this count as stale; never below `interval_seconds`.
    pub max_age_seconds: i64,
    pub timeout_seconds: i64,
}

impl ExchangeRateSource {
    pub const MIN_INTERVAL: i64 = 60;
    pub const MAX_INTERVAL: i64 = 604_800;
    pub const MAX_AGE: i64 = 2_592_000;
    pub const MIN_TIMEOUT: i64 = 1;
    pub const MAX_TIMEOUT: i64 = 120;

    /// The limits of the spec; `Err` names the offending field.
    pub fn check(&self) -> Result<(), String> {
        let lower = self.url.to_ascii_lowercase();
        let scheme_ok = lower.starts_with("http://") || lower.starts_with("https://");
        if !scheme_ok || self.url.len() <= 8 || self.url.chars().any(char::is_whitespace) {
            return Err("url must be an absolute http or https URL".into());
        }
        if !(Self::MIN_INTERVAL..=Self::MAX_INTERVAL).contains(&self.interval_seconds) {
            return Err("interval_seconds must be between 60 and 604800".into());
        }
        if self.max_age_seconds < self.interval_seconds || self.max_age_seconds > Self::MAX_AGE {
            return Err("max_age_seconds must be between interval_seconds and 2592000".into());
        }
        if !(Self::MIN_TIMEOUT..=Self::MAX_TIMEOUT).contains(&self.timeout_seconds) {
            return Err("timeout_seconds must be between 1 and 120".into());
        }
        Ok(())
    }
}

/// The stored source with its fetch bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeRateSourceRecord {
    pub source: ExchangeRateSource,
    pub updated_at: Timestamp,
    pub last_attempt_at: Option<Timestamp>,
    pub last_success_at: Option<Timestamp>,
    pub last_error: Option<String>,
    /// Pairs stored by the last successful fetch.
    pub last_count: i64,
}

/// A stored pair written by the source, with the time of the fetch that wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePair {
    pub from: String,
    pub to: String,
    pub fetched_at: Timestamp,
}

/// Outcome of one fetch, as recorded on the source: the number of pairs stored, or why it
/// failed (shown verbatim to operators).
pub type FetchOutcome = Result<i64, String>;

/// Fetches a provider's rate table (spec `exchange-rate-source`, "Provider format"). The
/// infrastructure implements it over HTTP; tests return canned bodies.
#[async_trait]
pub trait RateFetcher: Send + Sync {
    /// The response body of `GET url`, or why it could not be fetched (status, timeout, ...).
    async fn fetch(&self, url: &str, timeout_seconds: i64) -> Result<String, String>;
}

/// The promotion catalogue (spec `promotion-catalog`).
#[async_trait]
pub trait PromotionCatalog: Send + Sync {
    // ---- exchange-rate source (spec `exchange-rate-source`) ----
    async fn get_rate_source(&self) -> Result<Option<ExchangeRateSourceRecord>, StorageError>;
    /// Upsert the setting; keeps the fetch bookkeeping of an existing record.
    async fn put_rate_source(
        &self,
        source: ExchangeRateSource,
        now: Timestamp,
    ) -> Result<(), StorageError>;
    /// Remove the setting and make every `source` pair `manual`. `Ok(false)` when none was set.
    async fn delete_rate_source(&self) -> Result<bool, StorageError>;
    /// Record an attempt: a success moves `last_success_at` and `last_count`, a failure
    /// stores its message. No-op without a source.
    async fn record_rate_fetch(
        &self,
        now: Timestamp,
        outcome: FetchOutcome,
    ) -> Result<(), StorageError>;
    /// Upsert the pairs whose stored origin is not `manual`, stamped `source` at `now`;
    /// returns how many were written.
    async fn put_fetched_rates(
        &self,
        rates: Vec<ExchangeRate>,
        now: Timestamp,
    ) -> Result<i64, StorageError>;
    /// Pairs with origin `source`, ordered by `(from, to)`.
    async fn list_source_pairs(&self) -> Result<Vec<SourcePair>, StorageError>;

    async fn snapshot(&self) -> Result<CatalogSnapshot, StorageError>;
    async fn get_policy(&self) -> Result<PricingPolicy, StorageError>;
    async fn put_policy(&self, policy: PricingPolicy) -> Result<(), StorageError>;
    async fn list_groups(&self) -> Result<Vec<StackingGroup>, StorageError>;
    async fn get_group(&self, group_id: &str) -> Result<Option<StackingGroup>, StorageError>;
    async fn put_group(&self, group: StackingGroup) -> Result<(), StorageError>;
    /// `Ok(false)` when the group did not exist.
    async fn delete_group(&self, group_id: &str) -> Result<bool, StorageError>;
    /// Non-archived promotions with `remaining_budget` filled in.
    async fn list_promotions(&self) -> Result<Vec<Promotion>, StorageError>;
    async fn get_promotion(&self, promotion_id: &str) -> Result<Option<Promotion>, StorageError>;
    /// Upsert the promotion body; `remaining_budget` in the body is ignored (see `put_budget`).
    async fn put_promotion(&self, promotion: Promotion) -> Result<(), StorageError>;
    /// Archive: the promotion leaves snapshots and listings. `Ok(false)` when unknown.
    async fn archive_promotion(&self, promotion_id: &str) -> Result<bool, StorageError>;
    /// `Ok(None)` when the promotion is unknown; `Ok(Some(None))` when it has no budget.
    async fn get_budget(&self, promotion_id: &str) -> Result<Option<Option<Money>>, StorageError>;
    /// Set or clear the remaining budget. `Ok(false)` when the promotion is unknown.
    async fn put_budget(
        &self,
        promotion_id: &str,
        remaining: Option<Money>,
    ) -> Result<bool, StorageError>;
    /// Every promotion that has a budget, with its remaining amount.
    async fn list_budgets(&self) -> Result<Vec<(PromotionId, Money)>, StorageError>;
    async fn list_coupons(&self) -> Result<Vec<Coupon>, StorageError>;
    async fn get_coupon(&self, code: &str) -> Result<Option<Coupon>, StorageError>;
    async fn put_coupon(&self, coupon: Coupon) -> Result<(), StorageError>;
    async fn delete_coupon(&self, code: &str) -> Result<bool, StorageError>;
    async fn list_rates(&self) -> Result<Vec<ExchangeRate>, StorageError>;
    async fn put_rate(&self, rate: ExchangeRate) -> Result<(), StorageError>;
    async fn delete_rate(&self, from: &str, to: &str) -> Result<bool, StorageError>;
    async fn list_grant_rules(&self) -> Result<Vec<GrantRule>, StorageError>;
    async fn get_grant_rule(&self, rule_id: &str) -> Result<Option<GrantRule>, StorageError>;
    async fn put_grant_rule(&self, rule: GrantRule) -> Result<(), StorageError>;
    async fn delete_grant_rule(&self, rule_id: &str) -> Result<bool, StorageError>;
    /// Store a batch of new coupons; fails on a code that already exists.
    async fn put_coupons(&self, coupons: Vec<Coupon>) -> Result<(), StorageError>;
    /// The coupon issued for `(trigger, order_id)`, if any (idempotency record of `grant`).
    async fn get_grant(
        &self,
        trigger: GrantTrigger,
        order_id: &str,
    ) -> Result<Option<Coupon>, StorageError>;
    /// Record the grant and store its coupon atomically. Returns the coupon now on record for
    /// `(trigger, order_id)`: `coupon`, unless another grant for the same key won the race.
    async fn record_grant(
        &self,
        trigger: GrantTrigger,
        order_id: &str,
        coupon: Coupon,
        now: Timestamp,
    ) -> Result<Coupon, StorageError>;
    /// Coupons whose `customer_id` is `customer_id`, with `uses` counted, ordered by code.
    async fn coupons_bound_to(&self, customer_id: &str) -> Result<Vec<Coupon>, StorageError>;
    /// Codes the customer saved (`SavedCoupon`), ordered.
    async fn saved_codes(&self, customer_id: &str) -> Result<Vec<String>, StorageError>;
    /// Idempotent; the caller has checked that the code exists.
    async fn save_coupon(&self, customer_id: &str, code: &str) -> Result<(), StorageError>;
    /// `Ok(false)` when nothing was saved.
    async fn forget_coupon(&self, customer_id: &str, code: &str) -> Result<bool, StorageError>;
    /// Metadata of a non-archived promotion, if stored.
    async fn get_metadata(
        &self,
        promotion_id: &str,
    ) -> Result<Option<PromotionMetadata>, StorageError>;
    /// Upsert; `Ok(false)` when the promotion is unknown or archived.
    async fn put_metadata(
        &self,
        promotion_id: &str,
        metadata: PromotionMetadata,
    ) -> Result<bool, StorageError>;
    /// `Ok(false)` when nothing was stored.
    async fn delete_metadata(&self, promotion_id: &str) -> Result<bool, StorageError>;
    /// Metadata of every non-archived promotion that has some, ordered by promotion id.
    async fn list_metadata(&self) -> Result<Vec<(PromotionId, PromotionMetadata)>, StorageError>;
    /// The allowlist of a non-archived promotion, sorted; `None` when the promotion is unknown.
    async fn get_allowlist(&self, promotion_id: &str) -> Result<Option<Vec<String>>, StorageError>;
    /// Replace the whole set; `Ok(false)` when the promotion is unknown or archived.
    async fn put_allowlist(
        &self,
        promotion_id: &str,
        customer_ids: Vec<String>,
    ) -> Result<bool, StorageError>;
    /// Clear the set; `Ok(false)` when the promotion is unknown or archived.
    async fn delete_allowlist(&self, promotion_id: &str) -> Result<bool, StorageError>;
    /// `Ok(false)` when the promotion is unknown or archived.
    async fn add_to_allowlist(
        &self,
        promotion_id: &str,
        customer_id: &str,
    ) -> Result<bool, StorageError>;
    /// `Ok(false)` when the customer was not listed (or the promotion is unknown).
    async fn remove_from_allowlist(
        &self,
        promotion_id: &str,
        customer_id: &str,
    ) -> Result<bool, StorageError>;
    async fn is_allowlisted(
        &self,
        promotion_id: &str,
        customer_id: &str,
    ) -> Result<bool, StorageError>;
    /// Non-archived promotions whose allowlist contains the customer, sorted.
    async fn allowlisted_for(&self, customer_id: &str) -> Result<Vec<PromotionId>, StorageError>;
    /// `None` when the promotion is unknown or archived.
    async fn get_promotion_state(
        &self,
        promotion_id: &str,
    ) -> Result<Option<CatalogState>, StorageError>;
    /// `Ok(false)` when the promotion is unknown or archived.
    async fn set_promotion_state(
        &self,
        promotion_id: &str,
        state: CatalogState,
    ) -> Result<bool, StorageError>;
    /// `None` when the coupon is unknown.
    async fn get_coupon_state(&self, code: &str) -> Result<Option<CatalogState>, StorageError>;
    /// `Ok(false)` when the coupon is unknown.
    async fn set_coupon_state(&self, code: &str, state: CatalogState)
    -> Result<bool, StorageError>;
}

/// Operator switch on a promotion or a coupon (OpenSpec `catalogue-states`): `paused` keeps
/// the record but takes it out of every pricing snapshot. Archiving stays terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatalogState {
    Active,
    Paused,
}

impl CatalogState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CatalogState::Active => "active",
            CatalogState::Paused => "paused",
        }
    }
    pub fn parse(s: &str) -> Option<CatalogState> {
        match s {
            "active" => Some(CatalogState::Active),
            "paused" => Some(CatalogState::Paused),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedemptionStatus {
    Reserved,
    Confirmed,
    Cancelled,
}

impl RedemptionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RedemptionStatus::Reserved => "reserved",
            RedemptionStatus::Confirmed => "confirmed",
            RedemptionStatus::Cancelled => "cancelled",
        }
    }
}

/// A pricing result applied to an order (spec `redemption`). `input` is the replay record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Redemption {
    pub order_id: String,
    pub customer_id: Option<String>,
    pub status: RedemptionStatus,
    pub input: PricingInput,
    pub result: PricingResult,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedemptionError {
    NotFound,
    InvalidTransition {
        from: RedemptionStatus,
        to: RedemptionStatus,
    },
    Pricing(PricingError),
    Storage(StorageError),
}

impl From<StorageError> for RedemptionError {
    fn from(e: StorageError) -> Self {
        RedemptionError::Storage(e)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReserveOutcome {
    Reserved(Redemption),
    /// A reserved or confirmed redemption already existed; nothing changed.
    Idempotent(Redemption),
}

impl ReserveOutcome {
    pub fn redemption(&self) -> &Redemption {
        match self {
            ReserveOutcome::Reserved(r) | ReserveOutcome::Idempotent(r) => r,
        }
    }
}

/// Prices a cart against a snapshot taken under the budget lock. Returns the input (for the
/// replay record) and the result.
pub type PricingFn<'a> = dyn Fn(CatalogSnapshot, Uses) -> Result<(PricingInput, PricingResult), PricingError>
    + Send
    + Sync
    + 'a;

/// Redemption storage. The transaction boundary lives here (design decision 2).
#[async_trait]
pub trait RedemptionRepository: Send + Sync {
    /// In one transaction: return the existing reserved/confirmed redemption for
    /// `order_id` (idempotent), or lock the budgets, take the snapshot, run `price`, store
    /// the redemption as `reserved` and decrement the applied promotions' budgets.
    async fn reserve(
        &self,
        order_id: &str,
        customer_id: Option<&str>,
        codes: &[String],
        now: Timestamp,
        price: &PricingFn<'_>,
    ) -> Result<ReserveOutcome, RedemptionError>;
    /// Reserved and confirmed redemptions of `customer_id`, counted per applied promotion.
    async fn uses_for_customer(&self, customer_id: &str) -> Result<Uses, RedemptionError>;
    /// Promotion and coupon counters over all reserved and confirmed redemptions, for the
    /// given coupon codes.
    async fn counters(&self, codes: &[String]) -> Result<Counters, RedemptionError>;
    /// Spec `redemption`, "Coupon uses of a customer": `(code, order_id)` for every code the
    /// customer's reserved or confirmed redemptions presented and whose promotion applied,
    /// ordered by `created_at` then `order_id`.
    async fn coupon_uses_of_customer(
        &self,
        customer_id: &str,
    ) -> Result<Vec<(String, String)>, RedemptionError>;
    /// `reserved → confirmed`, `reserved|confirmed → cancelled` (restoring budgets).
    async fn transition(
        &self,
        order_id: &str,
        to: RedemptionStatus,
        now: Timestamp,
    ) -> Result<Redemption, RedemptionError>;
    async fn get(&self, order_id: &str) -> Result<Option<Redemption>, RedemptionError>;
}

/// The transitions the `redemption` spec allows.
pub fn transition_allowed(from: RedemptionStatus, to: RedemptionStatus) -> bool {
    matches!(
        (from, to),
        (RedemptionStatus::Reserved, RedemptionStatus::Confirmed)
            | (RedemptionStatus::Reserved, RedemptionStatus::Cancelled)
            | (RedemptionStatus::Confirmed, RedemptionStatus::Cancelled)
    )
}
