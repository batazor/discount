# redemption Specification

## Purpose
Applying a pricing result to a real order: reserving promotion budgets, confirming or
releasing them, and never doing it twice for the same order.

## Requirements

### Requirement: Redeem reserves atomically
`Redeem(order_id, cart, customer)` SHALL, inside one transaction, lock the budgets of the
promotions that could apply, take the promotion snapshot with the locked
`remaining_budget` values, run `apply`, store a `Redemption` with status `reserved`
holding the `PricingInput` and `PricingResult`, and decrement each applied promotion's
`remaining_budget` by its discount. Two concurrent redemptions SHALL never together
decrement a budget below zero.

#### Scenario: Reserve decrements budget
- **WHEN** promotion `P1` has `remaining_budget` 1000 and an order is redeemed where `P1`
  grants 300
- **THEN** the redemption is stored as `reserved`, its result shows `P1` with discount 300,
  and `P1`'s `remaining_budget` is 700

#### Scenario: Concurrent reservations respect the budget
- **WHEN** two orders are redeemed concurrently against `P1` with `remaining_budget` 500
  where each would take 300 unclamped
- **THEN** one result shows 300 and the other 200 with `clamped_by = Budget`, and the
  budget ends at 0

### Requirement: Redeem is idempotent by order id
A second `Redeem` for an `order_id` that already has a redemption in status `reserved` or
`confirmed` SHALL return the stored result unchanged and SHALL NOT touch budgets. A
redemption in status `cancelled` SHALL be replaced by a new reservation.

#### Scenario: Repeated redeem
- **WHEN** `Redeem` is called twice with the same `order_id`
- **THEN** both calls return the same `PricingResult` and the budget was decremented once

### Requirement: Confirm and cancel
`ConfirmRedemption(order_id)` SHALL move `reserved → confirmed`. `CancelRedemption(order_id)`
SHALL move `reserved → cancelled` or `confirmed → cancelled` and restore to each applied
promotion's `remaining_budget` the discount it granted. Any other transition SHALL be
rejected with `InvalidTransition { from, to }`; an unknown `order_id` SHALL be rejected
with `NotFound`.

#### Scenario: Cancel restores budget
- **WHEN** a reservation that took 300 from `P1` (now at 700) is cancelled
- **THEN** the redemption is `cancelled` and `P1`'s `remaining_budget` is 1000

#### Scenario: Confirming a cancelled redemption fails
- **WHEN** `ConfirmRedemption` is called for a `cancelled` redemption
- **THEN** the use case returns `InvalidTransition { from: cancelled, to: confirmed }`

### Requirement: Redemption HTTP API
The service SHALL expose `POST /v1/redeem` with body `{order_id, cart, customer}`
responding `200` with `{order_id, status, result}` (also for the idempotent repeat),
`422 {"error": PricingError}` when pricing fails (nothing is stored),
`POST /v1/redemptions/{order_id}/confirm` and `/cancel` responding `200 {order_id, status}`,
`404 {"error": "NotFound"}` for unknown orders and `409 {"error": "InvalidTransition", from, to}`
for rejected transitions, and `GET /v1/redemptions/{order_id}` responding the stored
redemption.

#### Scenario: Redeem over HTTP
- **WHEN** a valid `{order_id, cart, customer}` is posted to `/v1/redeem`
- **THEN** the response is `200` with `status = "reserved"` and a `result` identical to
  what `/v1/evaluate` returns for the same cart at that moment

### Requirement: Redemptions remember the customer
A `Redemption` SHALL store `customer_id` (from `customer.customer_id`, may be absent). The
repository SHALL answer, for a customer id, the number of reserved or confirmed redemptions
per applied `promotion_id`; cancelled redemptions do not count.

#### Scenario: Counting uses
- **WHEN** customer `c1` has redemptions `o1` (reserved, applied `P1`), `o2` (confirmed,
  applied `P1` and `P2`) and `o3` (cancelled, applied `P1`)
- **THEN** the uses for `c1` are `P1: 2, P2: 1`
