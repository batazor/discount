# Spec Delta

## ADDED Requirements

### Requirement: Explanations on request
`POST /v1/evaluate?explain=true` SHALL respond `200 {result, explanations}` and
`POST /v1/redeem?explain=true` SHALL respond `200 {order_id, status, result, explanations}`;
without the flag both responses are unchanged. `explanations` holds one
`Explanation { subject, code, promotion_id?, coupon_code?, rule_index?, rule?, message }` per
skipped promotion and per presented coupon code, promotions first in the total order, then
codes in the order of `cart.coupon_codes`.

#### Scenario: Result is untouched
- **WHEN** the same cart is evaluated with and without `explain=true`
- **THEN** `result` is byte-for-byte the same `PricingResult`

### Requirement: Explanation codes
Promotion entries SHALL use `not_eligible.<rule type in snake_case>` (with `rule_index` and
the `rule` body), `exclusive_applied` or `first_match_taken`. Coupon entries SHALL use
`coupon.applied`, `coupon.unknown`, `coupon.not_started`, `coupon.expired`,
`coupon.exhausted`, `coupon.wrong_customer`, `coupon.paused`, `coupon.promotion_paused` or
`coupon.promotion_archived`.

#### Scenario: Minimum not met
- **WHEN** `P1` carries `OrderTotalRange { min: 5000 }` and a cart of 4499 is evaluated with
  `explain=true`
- **THEN** `explanations` holds `{subject: "promotion", promotion_id: "P1",
  code: "not_eligible.order_total_range", rule_index: 0, rule: {...}}`

#### Scenario: Unknown and expired codes
- **WHEN** the cart presents `NOPE` (not in the catalogue) and `OLD` (ends_at in the past)
- **THEN** `explanations` holds `coupon.unknown` for `NOPE` and `coupon.expired` for `OLD`

