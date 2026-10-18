# Spec Delta

## Purpose
Which coupons belong to a customer, what state each one is in, and how a customer keeps a
public code.

## ADDED Requirements

### Requirement: A customer's coupons
`GET /v1/customers/{customer_id}/coupons` SHALL return every stored coupon that is bound to
the customer (`coupon.customer_id` equals `customer_id`), used by the customer (a reserved or
confirmed redemption with that `customer_id` presented the code and applied the coupon's
promotion), or saved by the customer, each once, ordered by `code`, as
`CustomerCoupon { coupon, status, order_id, promotion }` with `promotion` =
`{ promotion_id, action }` of the coupon's promotion.

#### Scenario: Unknown customer
- **WHEN** nothing refers to customer `nobody`
- **THEN** the list is empty with status `200`

#### Scenario: Promotion deleted
- **WHEN** the promotion of a coupon bound to `c1` was deleted
- **THEN** the entry has `promotion = null` and status `expired`

#### Scenario: Three sources, no duplicates
- **WHEN** `GRANT-1` is bound to `c1`, `c1` used public code `WELCOME` in a reserved
  redemption, `c1` saved `SPRING`, and `OTHER` belongs to nobody
- **THEN** the list for `c1` holds `GRANT-1`, `SPRING`, `WELCOME` and nothing else

#### Scenario: Another customer's binding
- **WHEN** `GRANT-2` is bound to `c2`
- **THEN** it is not in the list for `c1`

### Requirement: Coupon status
`status` SHALL be `applied` when the customer used the coupon (then `order_id` is the
`order_id` of the earliest such redemption); otherwise `expired` when `ends_at ≤ now`, when
`max_uses` is set and `uses ≥ max_uses`, or when the promotion is archived or missing;
otherwise `scheduled` when `starts_at > now`; otherwise `active`. `order_id` is `null`
unless the status is `applied`.

#### Scenario: Applied wins
- **WHEN** `c1` redeemed order `o1` with single-use `ONCE` (so `uses = 1 = max_uses`)
- **THEN** `ONCE` is listed for `c1` as `applied` with `order_id = "o1"`

#### Scenario: Exhausted by someone else
- **WHEN** `c1` saved public single-use `ONCE` and `c2` used it
- **THEN** `ONCE` is listed for `c1` as `expired`

#### Scenario: Window
- **WHEN** a coupon bound to `c1` has `starts_at = now + 10`
- **THEN** it is `scheduled`; with `ends_at = now` it is `expired`; with both absent it is
  `active`

#### Scenario: Cancelled redemption does not apply
- **WHEN** `c1`'s only redemption with `WELCOME` is cancelled
- **THEN** `WELCOME` is not listed for `c1` unless bound or saved, and if saved its status is
  `active`

### Requirement: Saving a public code
`PUT /v1/customers/{customer_id}/coupons/{code}` SHALL record `SavedCoupon { customer_id,
code }` and respond `200` with the `CustomerCoupon`; it is idempotent. An unknown `code`
SHALL be `404 {"error": "NotFound"}`; a code bound to another customer SHALL be
`409 {"error": "BoundToAnotherCustomer"}`. `DELETE` SHALL remove the saved record (`204`,
or `404` when it was not saved); a bound or used coupon stays in the list.

#### Scenario: Save and forget
- **WHEN** `c1` saves `SPRING` twice, then deletes it
- **THEN** both saves return `200`, `SPRING` is listed once in between, the delete returns
  `204` and `SPRING` is no longer listed

#### Scenario: Someone else's code
- **WHEN** `c1` saves `GRANT-2`, which is bound to `c2`
- **THEN** the response is `409`
