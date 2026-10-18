# Spec Delta

## MODIFIED Requirements

### Requirement: Rule catalogue
`Rule` SHALL have exactly these variants, evaluated against `(cart, customer, coupons, now)`
and the promotion being evaluated:
- `DateWindow { starts_at?, ends_at? }`: `starts_at ≤ now` (or absent) and `now < ends_at`
  (or absent).
- `OrderTotalRange { min?, max? }`: `min ≤ orderSubtotal` (or absent) and
  `orderSubtotal ≤ max` (or absent), amounts in the cart currency after normalization.
- `TagsMatch { all, any, none }`: every tag in `all` is in `cart.tags`; `any` is empty or
  shares a tag with `cart.tags`; no tag in `none` is in `cart.tags`.
- `AttributeIn { key, values }`: some `cart.attributes` entry has that `key` and a `value`
  in `values`.
- `AttributeDateWindow { key, starts_at?, ends_at? }`: the attribute `key` exists, its
  value parses as a decimal integer `t` (Unix seconds), and `starts_at ≤ t` (or absent) and
  `t < ends_at` (or absent).
- `AttributeRange { key, min?, max? }`: the cart attribute `key` exists, its value parses as
  a decimal integer `t`, and `min ≤ t` (or absent) and `t ≤ max` (or absent); a missing or
  non-integer value fails the rule.
- `MaxCompletedOrders { max }`: `customer.completed_orders ≤ max`.
- `MaxUsesPerCustomer { max }`: the count in `customer.uses` for this promotion (0 when
  absent) is `< max`.
- `RequiresCoupon`: see `coupons`.

#### Scenario: Minimum order total
- **WHEN** a promotion carries `OrderTotalRange { min: 5000, max: null }` and the cart
  subtotal is 4499
- **THEN** the promotion is skipped with `NotEligible { rule_index }` of that rule

#### Scenario: First order only
- **WHEN** a promotion carries `MaxCompletedOrders { max: 0 }` and `customer.completed_orders`
  is 1
- **THEN** the promotion is not eligible

#### Scenario: Usage limit
- **WHEN** a promotion `P1` carries `MaxUsesPerCustomer { max: 2 }` and `customer.uses`
  lists `P1` with count 2
- **THEN** `P1` is not eligible; with count 1 it is

#### Scenario: Check-in window
- **WHEN** a promotion carries `AttributeDateWindow { key: "checkin", starts_at: 100, ends_at: 200 }`
  and `cart.attributes` has `checkin = "150"`
- **THEN** the rule holds; with `checkin = "200"` or `checkin = "soon"` it does not

#### Scenario: Floor on a second amount
- **WHEN** a promotion carries `AttributeRange { key: "commission_free_subtotal", min: 500000, max: null }`
  and `cart.attributes` has `commission_free_subtotal = "500000"`
- **THEN** the rule holds; with `"499999"`, `"n/a"` or no such attribute it does not
