# coupons Specification

## Purpose
How a code presented with a cart turns a promotion on, and where coupons live.

## Requirements

### Requirement: Coupon snapshot
A `Coupon` SHALL have `code`, `promotion_id`, `starts_at?`, `ends_at?` (half-open window on
`now`), `customer_id?` (the only customer allowed to use it), `max_uses?` (absent =
unlimited; `1` = single use) and `uses` (how many reserved or confirmed redemptions already
used it, `≥ 0`). Pricing receives the relevant coupons in `PricingInput.coupons`; the cart
lists the codes presented in `cart.coupon_codes`.

#### Scenario: Coupon turns a promotion on
- **WHEN** promotion `P1` carries `RequiresCoupon`, `coupons` holds
  `{code: "WELCOME", promotion_id: "P1", max_uses: null, uses: 0}` and `cart.coupon_codes` is
  `["WELCOME"]`
- **THEN** `P1` is eligible

### Requirement: RequiresCoupon semantics
`RequiresCoupon` SHALL hold when some coupon in `coupons` has this promotion's
`promotion_id`, its `code` is in `cart.coupon_codes`, its window contains `now`, its
`customer_id` is absent or equals `customer.customer_id`, and it has remaining uses
(`max_uses` absent or `uses < max_uses`).

#### Scenario: Wrong customer
- **WHEN** the coupon is bound to customer `c1` and `customer.customer_id` is `c2`
- **THEN** the promotion is not eligible

#### Scenario: Expired coupon
- **WHEN** the coupon has `ends_at = 100` and `now = 100`
- **THEN** the promotion is not eligible

#### Scenario: Exhausted coupon
- **WHEN** the coupon has `max_uses = 1` and `uses = 1`
- **THEN** the promotion is not eligible; with `uses = 0` it is

### Requirement: Coupons are validated
`validate` SHALL reject a `code` listed twice in `coupons` (`DuplicateCouponCode { code }`), a
`code` listed twice in `cart.coupon_codes` (`DuplicateCouponCode { code }`), a negative
`uses` or `max_uses` (`InvalidCoupon { field }` with `coupons[<k>].uses|max_uses`), after the
cart checks and before the promotion checks.

#### Scenario: Duplicate code
- **WHEN** `coupons` lists `WELCOME` twice
- **THEN** `validate` reports `DuplicateCouponCode { code: "WELCOME" }`

#### Scenario: Negative counter
- **WHEN** `coupons[0].uses` is `-1`
- **THEN** `validate` reports `InvalidCoupon { field: "coupons[0].uses" }`

### Requirement: Coupon storage and API
The catalogue SHALL store coupons and expose `GET /v1/coupons`, `GET|PUT|DELETE
/v1/coupons/{code}`. `Evaluate` and `Redeem` SHALL pass to pricing exactly the stored
coupons whose `code` is in `cart.coupon_codes` (unknown codes contribute nothing).

#### Scenario: Unknown code
- **WHEN** `cart.coupon_codes` is `["NOPE"]` and no such coupon is stored
- **THEN** pricing proceeds with an empty `coupons` list and `RequiresCoupon` promotions are
  skipped as `NotEligible`
