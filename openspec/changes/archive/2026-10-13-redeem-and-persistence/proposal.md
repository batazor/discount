# Proposal

## Why

The engine prices carts but nothing can be consumed: promotions live in a JSON fixture,
`remaining_budget` is an input nobody decrements, and an evaluation leaves no record. To be
a service rather than a calculator the discount system needs a catalogue that can be edited
and a redemption lifecycle that reserves budget atomically, confirms or releases it, and is
idempotent per order.

## What Changes

- Postgres persistence (sqlx) for the promotion catalogue (policy, groups, promotions,
  budgets) and for redemptions, with migrations run at start-up.
- Three new use cases in `app/`: `Redeem` (reserve), `ConfirmRedemption`,
  `CancelRedemption`. `Redeem` re-runs `apply` under a lock on the budgets it touches,
  stores the input and result, decrements `remaining_budget`; `Cancel` restores it.
  Idempotent by `order_id`.
- Application ports become async (`async-trait`) so the Postgres adapter can implement them;
  `Evaluate` follows.
- HTTP: `POST /v1/redeem`, `POST /v1/redemptions/{order_id}/confirm|cancel`,
  `GET /v1/redemptions/{order_id}`, and catalogue CRUD (`/v1/policy`, `/v1/groups/{id}`,
  `/v1/promotions/{id}`, `/v1/promotions/{id}/budget`).
- Telemetry: `discount.redemptions` counter and a `discount.budget.remaining` gauge per
  promotion; the Promotions dashboard gains a budget panel.
- Local stack gains a Postgres container; CI gains a Postgres service for the adapter tests.
- The in-memory adapter implements the same ports so the service still runs without a
  database (`DATABASE_URL` unset) for local experiments and fast tests.

Outside the verified scope: all of it. `core/` and `spec/` are untouched; `Redeem` calls
`apply` exactly as `Evaluate` does. Invariant 6 (idempotence of pricing) stays a core
concern for the next change; service-level idempotence by `order_id` is specified here.

## Capabilities

### New Capabilities
- `redemption`: the reserve/confirm/cancel lifecycle, budget accounting and idempotency
  guarantees of applying a pricing result to an order.
- `promotion-catalog`: storage and HTTP management of the pricing policy, stacking groups,
  promotions and budgets, and the snapshot handed to pricing.

### Modified Capabilities
- `evaluate-http-api`: the service reads promotions from the catalogue (Postgres when
  configured) instead of only a fixture file; configuration gains `DATABASE_URL`.
- `telemetry`: new metrics for redemptions and remaining budgets.

## Impact

- `app/`: ports become async; new ports `PromotionCatalog`, `RedemptionRepository`; new use
  cases. Dependency: `async-trait`.
- `adapters/`: new `postgres` module and `migrations/`; `memory` implements the new ports;
  `http` gains routes; `telemetry` gains two instruments. Dependencies: `sqlx`
  (postgres, runtime-tokio, json, migrate), `async-trait`.
- `deploy/compose.yaml`: Postgres service; `.github/workflows/ci.yml`: Postgres service and
  `DATABASE_URL` for `cargo test`.
- `docs/ubiquitous-language.md`: "Application layer" section describes `Redeem`,
  `Redemption` and its states; telemetry vocabulary grows.
