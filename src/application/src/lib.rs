//! # discount-application
//!
//! Use cases and ports (hexagonal architecture). The domain is `discount-domain`; adapters
//! implement the ports in `discount-infrastructure`. Nothing here is in the verified scope:
//! every use case prices with `discount_domain::apply` and only adds bookkeeping.

pub mod coupons;
pub mod customer_coupons;
pub mod evaluate;
pub mod ports;

pub use coupons::{
    CodeSource, CouponError, GenerateCouponsUseCase, GrantCoupon, pick_rule, render_code,
};
pub use customer_coupons::{
    CouponPromotion, CouponStatus, CustomerCoupon, CustomerCouponError, ListCustomerCoupons,
    SaveCustomerCoupon, coupon_status,
};
pub use evaluate::{
    CancelRedemption, ConfirmRedemption, Evaluate, EvaluateError, Redeem, merge_allowlisted,
    pricing_input,
};
pub use ports::{
    CatalogSnapshot, CatalogState, Clock, Event, EventPublisher, PricingFn, PromotionCatalog,
    Redemption, RedemptionError, RedemptionRepository, RedemptionStatus, ReserveOutcome,
    StorageError, transition_allowed,
};
