# eligibility-rules Specification

## Purpose
The rules a promotion may carry and exactly when each holds. A promotion is eligible when
every rule holds; the index of the first failing rule is reported as `NotEligible`.

## Requirements

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

### Requirement: Rule parameters are validated
`validate` SHALL reject, per promotion in order of its rules, `OrderTotalRange` amounts
outside `0 ..= MAX_MINOR_UNITS` or in a currency without a rate (`AmountOutOfRange` /
`CurrencyMismatch` with field `promotions[<i>].rules[<k>].min|max`), and negative `max` in
`MaxCompletedOrders` / `MaxUsesPerCustomer` (`InvalidRule { field }`).

#### Scenario: Negative usage limit
- **WHEN** a promotion carries `MaxUsesPerCustomer { max: -1 }`
- **THEN** `validate` reports `InvalidRule { field: "promotions[0].rules[0].max" }`

### Requirement: Customer context is validated
`validate` SHALL reject `customer.completed_orders < 0`, a negative count in
`customer.uses`, and a `promotion_id` listed twice in `customer.uses`
(`InvalidCustomerContext { field }`).

#### Scenario: Duplicate use entry
- **WHEN** `customer.uses` lists `P1` twice
- **THEN** `validate` reports `InvalidCustomerContext { field: "customer.uses[1]" }`
