# Spec Delta

## MODIFIED Requirements

### Requirement: Redeem is idempotent by order id
A second `Redeem` for an `order_id` whose redemption is `reserved` SHALL, when `cart` and
`customer` equal the stored input's, return the stored result unchanged and SHALL NOT touch
budgets. When they differ, it SHALL supersede the reservation inside one transaction: restore
the budgets the stored result took, price the new input under the budget lock, replace the
redemption (status `reserved`, new `created_at`), and keep the previous version in the
order's history with status `superseded`. A `confirmed` redemption SHALL be returned
unchanged for equal input and rejected with `AlreadyConfirmed` for different input. A
`cancelled` redemption SHALL be replaced by a new reservation.

#### Scenario: Repeated redeem
- **WHEN** `Redeem` is called twice with the same `order_id`, cart and customer
- **THEN** both calls return the same `PricingResult` and the budget was decremented once

#### Scenario: Coupon added at checkout
- **WHEN** order `o1` is reserved without a code (P1 took 300 of a 1000 budget) and `Redeem`
  is called for `o1` with the cart now presenting `WELCOME`
- **THEN** the new result applies the coupon's promotion, the budget reflects only the new
  result, the stored redemption is the new one, and the history of `o1` holds the first
  version as `superseded`

#### Scenario: Confirmed orders are final
- **WHEN** `Redeem` is called for a `confirmed` order with a different cart
- **THEN** the use case returns `AlreadyConfirmed` and nothing changes

### Requirement: Redemption HTTP API
The service SHALL expose `POST /v1/redeem` with body `{order_id, cart, customer}`
responding `200` with `{order_id, status, result}` (also for the idempotent repeat and the
superseding re-price), `422 {"error": PricingError}` when pricing fails (nothing is stored,
and a superseded reservation is not released), `409 {"error": "AlreadyConfirmed"}` for a
different input on a confirmed order, `POST /v1/redemptions/{order_id}/confirm` and
`/cancel` responding `200 {order_id, status}`, `404 {"error": "NotFound"}` for unknown
orders and `409 {"error": "InvalidTransition", from, to}` for rejected transitions,
`GET /v1/redemptions/{order_id}` responding the stored redemption, and
`GET /v1/redemptions/{order_id}/history` responding the superseded versions, newest first,
each `{status: "superseded", input, result, created_at, superseded_at}`.

#### Scenario: Redeem over HTTP
- **WHEN** a valid `{order_id, cart, customer}` is posted to `/v1/redeem`
- **THEN** the response is `200` with `status = "reserved"` and a `result` identical to
  what `/v1/evaluate` returns for the same cart at that moment

#### Scenario: History
- **WHEN** `o1` was re-priced twice
- **THEN** `GET /v1/redemptions/o1/history` lists two superseded versions
