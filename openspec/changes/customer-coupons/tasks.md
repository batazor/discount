# Tasks

## 1. Contract first (`api/`, `docs/`)

- [x] 1.1 Extend `api/openapi.yaml`: tag `customers`, schemas `CouponStatus`, `CouponPromotion`, `CustomerCoupon`, operations `listCustomerCoupons`, `saveCustomerCoupon`, `forgetCustomerCoupon`; verify Redocly lint has no errors
- [x] 1.2 `docs/ubiquitous-language.md` application-layer names (`CustomerCoupon`, `CouponStatus`, `SavedCoupon`, `ListCustomerCoupons`); verify `openspec validate customer-coupons`

## 2. Service (`src/application`, `src/infrastructure`, `src/client`)

- [x] 2.1 Ports: `PromotionCatalog::{saved_coupons, save_coupon, forget_coupon, coupons_bound_to}`, `RedemptionRepository::coupon_uses_of_customer`; use case `ListCustomerCoupons` with the status function and unit tests for every status branch
- [x] 2.2 In-memory and Postgres stores (migration `0004_saved_coupons.sql`, index on the coupon's `customer_id`); verify the shared port scenarios on both backends
- [x] 2.3 HTTP routes and the generated client; verify axum tests, `openapi_covers_every_route` and the contract test cover list, save (200/404/409) and forget (204/404)
- [x] 2.4 Smoke: the single-use code redeemed in the smoke run is listed for customer `smoke` as `applied`; verify `deploy/smoke.sh` on the compose stack

## Workflow follow-up

- Archive the change after review: `openspec archive customer-coupons`.
