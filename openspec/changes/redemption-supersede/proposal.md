# Proposal

## Why

A storefront re-prices an order while the customer is still at checkout: a code is added or
removed, the cart changes. Today `Redeem` for a reserved order returns the stored result
whatever the new input, so the second code is silently ignored unless the caller cancels
first and redeems again, with a gap in between where nothing is reserved. The promocode
service being replaced replaces the pending application; Talon.One re-evaluates a session
on update.

## What Changes

- `Redeem` for an `order_id` in status `reserved` with a different `cart` or `customer`
  SHALL supersede the reservation in one transaction: budgets and counters of the old result
  are released, the new input is priced under the same locks, the redemption row is
  replaced, and the previous version is kept in a history. The same input remains the
  idempotent repeat.
- A `confirmed` order cannot be re-priced: `409 {"error": "AlreadyConfirmed"}`.
- `GET /v1/redemptions/{order_id}/history` lists superseded versions.

Outside the verified scope: everything here. Counters and budgets keep their definitions
because the current row is the only one that counts.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `redemption`: idempotency requirement becomes "idempotent on equal input, superseding on
  different input"; HTTP API gains the `409` and the history route.

## Impact

- `api/openapi.yaml`: `409 AlreadyConfirmed` on `/v1/redeem`, route
  `/v1/redemptions/{order_id}/history`, schema `RedemptionVersion`.
- `src/application`: `ReserveOutcome::Superseded`; `src/infrastructure`: table
  `redemption_history` (migration `0010_redemption_history.sql`), the reserve transaction
  releases the old budgets before pricing; tests on both backends; smoke step.
