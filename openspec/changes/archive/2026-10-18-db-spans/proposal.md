# Proposal

## Why

A redemption trace shows the HTTP server span and `discount.evaluate` only; the time spent in
the Postgres transaction (`FOR UPDATE` on budgets, snapshot, insert, decrement) is invisible,
so a slow `POST /v1/redeem` cannot be attributed to the database or to pricing.

## What Changes

- Every `PgStore` repository method produces a client span `db.<method>` (for example
  `db.reserve`, `db.get_promotion`) under the current span, with `db.system=postgresql` and
  `db.operation.name=<method>`; a storage failure marks the span `Error`.
- The in-memory store stays unspanned (tests and tools).
- The Telemetry vocabulary lists the new span family.

Outside the verified scope: everything here. Only `src/infrastructure/src/persistence/postgres.rs`,
the telemetry test and the vocabulary change; pricing results and metrics are untouched.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `telemetry`: one added requirement, "Database spans".

## Impact

- `src/infrastructure/src/persistence/postgres.rs`: `#[tracing::instrument]` on each trait method.
- `src/infrastructure/tests/telemetry.rs`: a Postgres-gated scenario (`DATABASE_URL`).
- `docs/ubiquitous-language.md`, `src/infrastructure/src/telemetry.rs` (`VOCABULARY`).
