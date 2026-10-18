# Proposal

## Why

A customer has no way to see the coupons that belong to them: codes granted after a
cancellation or a review, codes generated for them personally, public codes they used or
want to keep. The promocode service being replaced lists a user's codes with a status
(`active`, `expired`, `applied`) and the order that consumed them; the storefront's "my
coupons" screen depends on it.

## What Changes

- New read model `CustomerCoupon { coupon, status, order_id?, promotion: { promotion_id,
  action } }` with `CouponStatus = active | scheduled | expired | applied`, computed from the
  stored coupon, its promotion and the customer's redemptions at `now`. No cart is involved:
  cart-dependent eligibility stays with `Evaluate`.
- Three sources make a coupon the customer's: bound (`coupon.customer_id` equals the
  customer), used (a reserved or confirmed redemption of the customer presented the code and
  applied its promotion) and saved (the customer saved a public code).
- New application-layer concept `SavedCoupon { customer_id, code }`, stored in the catalogue.
- Routes: `GET /v1/customers/{customer_id}/coupons`, `PUT` and `DELETE
  /v1/customers/{customer_id}/coupons/{code}` (save, forget). New OpenAPI tag `customers`.
- `api/openapi.yaml` first, then scenarios, then code; the generated client follows.

Outside the verified scope: everything in this change. The pricing engine (ubiquitous
language → Lean → Rust → DRT) is untouched: no new domain type, rule or error.

## Capabilities

### New Capabilities
- `customer-coupons`: which coupons belong to a customer, their status, and saving a public
  code.

### Modified Capabilities
- `redemption`: the repository answers which codes a customer used and in which order.

## Impact

- `api/openapi.yaml`: tag `customers`, schemas `CustomerCoupon`, `CouponStatus`,
  `CouponPromotion`, three operations.
- `docs/ubiquitous-language.md`: application-layer names `CustomerCoupon`, `CouponStatus`,
  `SavedCoupon`, `ListCustomerCoupons`.
- `src/application`: ports `saved_coupons`, `save_coupon`, `forget_coupon`,
  `coupon_uses_of_customer`; use case `ListCustomerCoupons`.
- `src/infrastructure`: in-memory and Postgres stores, migration `0004_saved_coupons.sql`,
  HTTP routes; tests on both backends; smoke step; `src/client` contract test.
