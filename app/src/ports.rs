//! Ports: what the application needs from the outside world (ADR 7). Async so that the
//! Postgres adapter can implement them; `async-trait` keeps them object-safe.

use async_trait::async_trait;
use discount_core::{
    Coupon, ExchangeRate, Money, PricingError, PricingInput, PricingPolicy, PricingResult,
    Promotion, PromotionId, StackingGroup, Timestamp,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    Unavailable(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Unavailable(m) => write!(f, "storage unavailable: {m}"),
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

/// The promotion catalogue (spec `promotion-catalog`).
#[async_trait]
pub trait PromotionCatalog: Send + Sync {
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
        now: Timestamp,
        price: &PricingFn<'_>,
    ) -> Result<ReserveOutcome, RedemptionError>;
    /// Reserved and confirmed redemptions of `customer_id`, counted per applied promotion.
    async fn uses_for_customer(&self, customer_id: &str) -> Result<Uses, RedemptionError>;
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
