//! `ListCustomerCoupons` (spec `customer-coupons`): the coupons that belong to a customer
//! with a cart-free status. Outside the verified scope.

use std::collections::BTreeMap;

use discount_domain::{Action, Coupon, Promotion, Timestamp};
use serde::{Deserialize, Serialize};

use crate::ports::{Clock, PromotionCatalog, RedemptionError, RedemptionRepository, StorageError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CouponStatus {
    Active,
    Scheduled,
    Expired,
    Applied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CouponPromotion {
    pub promotion_id: String,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomerCoupon {
    pub coupon: Coupon,
    pub status: CouponStatus,
    pub order_id: Option<String>,
    pub promotion: Option<CouponPromotion>,
}

/// The status rule of the spec, as a pure function. `used` is the earliest order of the
/// customer that used the coupon, `promotion` the coupon's promotion if it still exists.
pub fn coupon_status(
    c: &Coupon,
    promotion: Option<&Promotion>,
    used: Option<&str>,
    now: Timestamp,
) -> CouponStatus {
    if used.is_some() {
        return CouponStatus::Applied;
    }
    let exhausted = c.max_uses.is_some_and(|m| c.uses >= m);
    if c.ends_at.is_some_and(|e| e <= now) || exhausted || promotion.is_none() {
        return CouponStatus::Expired;
    }
    if c.starts_at.is_some_and(|s| s > now) {
        return CouponStatus::Scheduled;
    }
    CouponStatus::Active
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomerCouponError {
    UnknownCode,
    BoundToAnotherCustomer,
    Storage(StorageError),
}

impl From<StorageError> for CustomerCouponError {
    fn from(e: StorageError) -> Self {
        CustomerCouponError::Storage(e)
    }
}

impl From<RedemptionError> for CustomerCouponError {
    fn from(e: RedemptionError) -> Self {
        match e {
            RedemptionError::Storage(s) => CustomerCouponError::Storage(s),
            other => CustomerCouponError::Storage(StorageError::Unavailable(format!("{other:?}"))),
        }
    }
}

pub struct ListCustomerCoupons<'a> {
    pub catalog: &'a dyn PromotionCatalog,
    pub redemptions: &'a dyn RedemptionRepository,
    pub clock: &'a dyn Clock,
}

impl ListCustomerCoupons<'_> {
    /// Bound ∪ used ∪ saved, each once, ordered by code.
    pub async fn run(&self, customer_id: &str) -> Result<Vec<CustomerCoupon>, CustomerCouponError> {
        let now = self.clock.now();
        let uses = self
            .redemptions
            .coupon_uses_of_customer(customer_id)
            .await?;
        let mut first_order: BTreeMap<String, String> = BTreeMap::new();
        for (code, order_id) in uses {
            first_order.entry(code).or_insert(order_id);
        }
        let mut coupons: BTreeMap<String, Coupon> = BTreeMap::new();
        for c in self.catalog.coupons_bound_to(customer_id).await? {
            coupons.insert(c.code.clone(), c);
        }
        let saved = self.catalog.saved_codes(customer_id).await?;
        for code in first_order.keys().chain(saved.iter()) {
            if !coupons.contains_key(code)
                && let Some(c) = self.catalog.get_coupon(code).await?
            {
                coupons.insert(code.clone(), c);
            }
        }
        let mut out = Vec::with_capacity(coupons.len());
        for (code, coupon) in coupons {
            out.push(
                self.entry(coupon, first_order.get(&code).map(String::as_str), now)
                    .await?,
            );
        }
        Ok(out)
    }

    /// One listing entry for a coupon that is already known to belong to the customer.
    pub async fn entry_for(
        &self,
        customer_id: &str,
        coupon: Coupon,
    ) -> Result<CustomerCoupon, CustomerCouponError> {
        let now = self.clock.now();
        let used = self
            .redemptions
            .coupon_uses_of_customer(customer_id)
            .await?
            .into_iter()
            .find(|(code, _)| *code == coupon.code)
            .map(|(_, order_id)| order_id);
        self.entry(coupon, used.as_deref(), now).await
    }

    async fn entry(
        &self,
        coupon: Coupon,
        used: Option<&str>,
        now: Timestamp,
    ) -> Result<CustomerCoupon, CustomerCouponError> {
        let promotion = self.catalog.get_promotion(&coupon.promotion_id).await?;
        let status = coupon_status(&coupon, promotion.as_ref(), used, now);
        Ok(CustomerCoupon {
            status,
            order_id: used.map(str::to_string),
            promotion: promotion.map(|p| CouponPromotion {
                promotion_id: p.promotion_id,
                action: p.action,
            }),
            coupon,
        })
    }
}

pub struct SaveCustomerCoupon<'a> {
    pub list: ListCustomerCoupons<'a>,
}

impl SaveCustomerCoupon<'_> {
    pub async fn run(
        &self,
        customer_id: &str,
        code: &str,
    ) -> Result<CustomerCoupon, CustomerCouponError> {
        let coupon = self
            .list
            .catalog
            .get_coupon(code)
            .await?
            .ok_or(CustomerCouponError::UnknownCode)?;
        match &coupon.customer_id {
            Some(owner) if owner != customer_id => {
                return Err(CustomerCouponError::BoundToAnotherCustomer);
            }
            Some(_) => {} // the customer's own bound coupon: nothing to save
            None => self.list.catalog.save_coupon(customer_id, code).await?,
        }
        self.list.entry_for(customer_id, coupon).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use discount_domain::Money;

    fn coupon(
        starts_at: Option<i64>,
        ends_at: Option<i64>,
        max_uses: Option<i64>,
        uses: i64,
    ) -> Coupon {
        Coupon {
            code: "C".into(),
            promotion_id: "P".into(),
            starts_at,
            ends_at,
            customer_id: None,
            max_uses,
            uses,
        }
    }

    fn promo() -> Promotion {
        Promotion {
            promotion_id: "P".into(),
            group_id: "g".into(),
            priority: 0,
            exclusive: false,
            cap: None,
            remaining_budget: None,
            uses_total: 0,
            rounding: None,
            rules: vec![],
            action: Action::OrderFixedAmount {
                amount: Money::new(100, "EUR"),
            },
        }
    }

    #[test]
    fn status_branches() {
        let p = promo();
        let now = 1000;
        assert_eq!(
            coupon_status(&coupon(None, None, None, 0), Some(&p), None, now),
            CouponStatus::Active
        );
        assert_eq!(
            coupon_status(&coupon(Some(1010), None, None, 0), Some(&p), None, now),
            CouponStatus::Scheduled
        );
        assert_eq!(
            coupon_status(&coupon(Some(1000), None, None, 0), Some(&p), None, now),
            CouponStatus::Active
        );
        assert_eq!(
            coupon_status(&coupon(None, Some(1000), None, 0), Some(&p), None, now),
            CouponStatus::Expired
        );
        assert_eq!(
            coupon_status(&coupon(None, Some(1001), None, 0), Some(&p), None, now),
            CouponStatus::Active
        );
        assert_eq!(
            coupon_status(&coupon(None, None, Some(1), 1), Some(&p), None, now),
            CouponStatus::Expired
        );
        assert_eq!(
            coupon_status(&coupon(None, None, Some(2), 1), Some(&p), None, now),
            CouponStatus::Active
        );
        assert_eq!(
            coupon_status(&coupon(None, None, None, 0), None, None, now),
            CouponStatus::Expired
        );
        // applied wins over everything
        assert_eq!(
            coupon_status(&coupon(None, Some(0), Some(1), 1), None, Some("o1"), now),
            CouponStatus::Applied
        );
        // a scheduled coupon that is also past its end is expired
        assert_eq!(
            coupon_status(&coupon(Some(2000), Some(900), None, 0), Some(&p), None, now),
            CouponStatus::Expired
        );
    }
}
