//! Coupon lifecycle use cases (spec `coupon-lifecycle`): bulk generation and grants by
//! event. Outside the verified scope.

use discount_domain::{Coupon, Timestamp};

use crate::ports::{
    CatalogState, Clock, GenerateCoupons, GrantRule, GrantTrigger, PromotionCatalog, StorageError,
};

/// Alphabet without `0/O` and `1/I`.
pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
pub const CODE_LENGTH: usize = 8;
pub const MAX_BATCH: u32 = 10_000;

/// A source of randomness for codes; the infrastructure provides a real one, tests a fixed one.
pub trait CodeSource: Send + Sync {
    /// `CODE_LENGTH` indices into `CODE_ALPHABET`.
    fn next_code(&self) -> [u8; CODE_LENGTH];
}

pub fn render_code(prefix: &str, idx: &[u8; CODE_LENGTH]) -> String {
    let mut s = String::with_capacity(prefix.len() + CODE_LENGTH);
    s.push_str(prefix);
    for i in idx {
        s.push(CODE_ALPHABET[(*i as usize) % CODE_ALPHABET.len()] as char);
    }
    s
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CouponError {
    UnknownPromotion,
    /// The promotion exists but is paused (spec `catalogue-states`).
    PromotionPaused,
    InvalidCount,
    NoRuleForTrigger,
    CodeSpaceExhausted,
    Storage(StorageError),
}

impl From<StorageError> for CouponError {
    fn from(e: StorageError) -> Self {
        CouponError::Storage(e)
    }
}

pub struct GenerateCouponsUseCase<'a> {
    pub catalog: &'a dyn PromotionCatalog,
    pub codes: &'a dyn CodeSource,
}

impl GenerateCouponsUseCase<'_> {
    pub async fn run(&self, req: GenerateCoupons) -> Result<Vec<String>, CouponError> {
        if req.count == 0 || req.count > MAX_BATCH {
            return Err(CouponError::InvalidCount);
        }
        match self.catalog.get_promotion_state(&req.promotion_id).await? {
            None => return Err(CouponError::UnknownPromotion),
            Some(CatalogState::Paused) => return Err(CouponError::PromotionPaused),
            Some(CatalogState::Active) => {}
        }
        let mut coupons: Vec<Coupon> = Vec::with_capacity(req.count as usize);
        let mut attempts = 0;
        while coupons.len() < req.count as usize {
            let code = render_code(&req.prefix, &self.codes.next_code());
            let taken = coupons.iter().any(|c| c.code == code)
                || self.catalog.get_coupon(&code).await?.is_some();
            if taken {
                attempts += 1;
                if attempts > 10 * req.count {
                    return Err(CouponError::CodeSpaceExhausted);
                }
                continue;
            }
            coupons.push(Coupon {
                code,
                promotion_id: req.promotion_id.clone(),
                starts_at: req.starts_at,
                ends_at: req.ends_at,
                customer_id: req.customer_id.clone(),
                max_uses: req.max_uses,
                uses: 0,
            });
        }
        let codes = coupons.iter().map(|c| c.code.clone()).collect();
        self.catalog.put_coupons(coupons).await?;
        Ok(codes)
    }
}

pub struct GrantCoupon<'a> {
    pub catalog: &'a dyn PromotionCatalog,
    pub codes: &'a dyn CodeSource,
    pub clock: &'a dyn Clock,
}

/// The rule that wins for a trigger: highest priority, ties by `rule_id`.
pub fn pick_rule(rules: &[GrantRule], trigger: GrantTrigger) -> Option<&GrantRule> {
    rules
        .iter()
        .filter(|r| r.trigger == trigger)
        .max_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| b.rule_id.cmp(&a.rule_id))
        })
}

impl GrantCoupon<'_> {
    pub async fn run(
        &self,
        trigger: GrantTrigger,
        customer_id: &str,
        order_id: &str,
    ) -> Result<Coupon, CouponError> {
        if let Some(existing) = self.catalog.get_grant(trigger, order_id).await? {
            return Ok(existing);
        }
        let rules = self.catalog.list_grant_rules().await?;
        let rule = pick_rule(&rules, trigger).ok_or(CouponError::NoRuleForTrigger)?;
        match self.catalog.get_promotion_state(&rule.promotion_id).await? {
            None => return Err(CouponError::UnknownPromotion),
            Some(CatalogState::Paused) => return Err(CouponError::PromotionPaused),
            Some(CatalogState::Active) => {}
        }
        let now: Timestamp = self.clock.now();
        let mut code = render_code(
            &format!("{}-", trigger.as_str().to_uppercase()),
            &self.codes.next_code(),
        );
        let mut attempts = 0;
        while self.catalog.get_coupon(&code).await?.is_some() {
            attempts += 1;
            if attempts > 10 {
                return Err(CouponError::CodeSpaceExhausted);
            }
            code = render_code(
                &format!("{}-", trigger.as_str().to_uppercase()),
                &self.codes.next_code(),
            );
        }
        let coupon = Coupon {
            code,
            promotion_id: rule.promotion_id.clone(),
            starts_at: Some(now),
            ends_at: Some(now + rule.validity_seconds),
            customer_id: Some(customer_id.to_string()),
            max_uses: rule.max_uses,
            uses: 0,
        };
        Ok(self
            .catalog
            .record_grant(trigger, order_id, coupon, now)
            .await?)
    }
}
