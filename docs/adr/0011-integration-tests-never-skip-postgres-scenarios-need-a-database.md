# 11. Integration tests never skip: Postgres scenarios need a database

Date: 2026-10-18

## Status

Accepted

## Context

The adapter tests of `src/infrastructure/` run every port scenario against the in-memory
store and against `PgStore`. Until now the Postgres half ran only when `DATABASE_URL` was
set and otherwise printed "skipped" and passed. A green `cargo test` therefore said nothing
about the Postgres adapter: a developer without the variable, or a CI job whose service
container failed to start, got the same result as a full run. The database spans of the
`telemetry` spec and the redemption transaction (`FOR UPDATE`, budgets) exist only in the
Postgres implementation, so the skipped half is the half that matters.

## Decision

Tests do not skip. A scenario that needs an external resource either finds it or fails
with the remedy in the message.

- Postgres scenarios connect to `DATABASE_URL` or, when it is unset, to the
  `discount_test` database of the local stack (`ops/compose.yaml`,
  `postgres://discount:discount@localhost:5432/discount_test`). They create that database
  on first use and wipe it on every run; they never touch `discount`, the service's own.
- No `if env is missing { return }` in a test, no `#[ignore]` for missing infrastructure.
  A missing server is a failed test whose message names the `docker compose` command.
- CI provides the same Postgres service it always did; the variable now only selects a
  different server, it no longer decides whether the scenarios run.

## Consequences

- `cargo test --workspace` needs a running Postgres (one `docker compose up -d postgres`).
  That is the price of a result that means what it says.
- A test that cannot reach its resource fails loudly instead of passing quietly, so a
  broken CI service container is visible in the run, not months later.
- Future resources (an OTLP collector, a browser for the console) follow the same rule:
  provide it in the local stack and in CI, never skip.
