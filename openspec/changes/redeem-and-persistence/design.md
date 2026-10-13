# Design

## Context

See proposal.md. Today `app::Evaluate` is synchronous over sync ports, the only repository
is an in-memory fixture, and the HTTP adapter has one business route. Constraints: `core/`
stays untouched; the Lean model and DRT are unaffected; `Redeem` must be exactly
`apply` plus bookkeeping.

## Goals / Non-Goals

**Goals:** atomic budget reservation under concurrency, idempotency by order, an editable
catalogue, the same service runnable with and without Postgres.

**Non-Goals:** customer usage counters and coupons (next change, they need core support),
multi-tenant catalogues, catalogue versioning beyond `updated_at` (a redemption keeps its
own copy of the input, which is the replay record), authentication.

## Decisions

1. **Async ports with `async-trait`.** sqlx is async; blocking it from sync traits would
   tie up the runtime. `async-trait` keeps the ports `dyn`-compatible so the HTTP layer
   can hold `Arc<dyn Port>`. Alternative: native `async fn` in traits — not object-safe
   yet without extra crates.

2. **The transaction boundary is the port.** `RedemptionRepository::reserve` takes the
   pricing closure: it opens the transaction, locks budgets (`SELECT … FOR UPDATE` on
   `promotion_budgets`), builds the snapshot, calls the closure (which calls `apply`),
   persists and decrements, commits. The use case stays pure orchestration and the
   in-memory adapter implements the same contract with a mutex. Alternative: a generic
   unit-of-work abstraction — more machinery than three use cases justify.

3. **Lock scope: all budget rows.** The set of promotions that might apply is the whole
   active catalogue, so `Redeem` locks every budget row (tens to hundreds). Simple and
   correct; revisit with per-group locking if the catalogue grows.

4. **Budgets are a separate table** (`promotion_budgets(promotion_id, remaining, currency)`),
   because they are the only mutable state; the promotion body is stored as the
   ubiquitous-language JSON (`jsonb`) minus `remaining_budget`, so adding fields to
   `Promotion` in the next change needs no migration.

5. **Redemption stores input and result as `jsonb`.** Cancel restores budgets from the
   stored result (`applied[*].discount`), not from a separate ledger table.

6. **Idempotency:** `redemptions.order_id` is the primary key; a reserved or confirmed row
   short-circuits `Redeem` and returns the stored result (`outcome = idempotent`); a
   cancelled row is replaced.

7. **Policy lives in the database when Postgres is configured** (`pricing_policy` single
   row, seeded from the env defaults on first start), so merchants can change rounding or
   the order cap without a redeploy.

8. **Budget gauge** is an `ObservableGauge` reading a map refreshed by a background task
   every 10 s from `list_budgets`; the callback never touches the database.

9. **Tests:** HTTP and use-case tests run on the in-memory adapter; Postgres tests are
   integration tests that skip (with a notice) when `DATABASE_URL` is unset and run in CI
   against a `postgres:16` service and locally against the compose Postgres.

## Risks / Trade-offs

- [Locking all budgets serialises redemptions] → acceptable at current scale; measured by
  `discount.evaluate.duration`/redemption latency once in Grafana; per-group locks later.
- [In-memory and Postgres adapters drift] → the same integration test suite runs against
  both (test module generic over the port).
- [Stored result and live budget disagree after a manual budget edit] → cancel restores
  what the result granted, never more than that; documented.

## Migration Plan

New tables only; `sqlx::migrate!` at start-up. Rollback: unset `DATABASE_URL` and the
service falls back to the in-memory catalogue.
