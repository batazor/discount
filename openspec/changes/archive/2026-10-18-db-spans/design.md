# Design

## Context

`PgStore` (sqlx) implements `PromotionCatalog` (~35 methods) and `RedemptionRepository`
(6 methods) behind `#[async_trait]`. sqlx emits no spans of its own. See proposal.md, Why.

## Goals / Non-Goals

Goals: one span per repository call with the semantic-convention attributes, cheap to add
and to keep as methods are added. Non-goals: per-statement spans, SQL text as an attribute
(`db.query.text` would leak parameters into Tempo), connection-pool metrics.

## Decisions

1. **`#[tracing::instrument]` per method, not a wrapper type.** The attribute
   (`name = "db.<fn>", skip_all, err(Debug), fields(otel.kind = "client", db.system =
   "postgresql", db.operation.name = "<fn>")`) is applied inside the `#[async_trait]`
   impls, which tracing supports. A delegating `Traced<R>` wrapper was rejected: it
   repeats every signature and silently misses methods added later. `skip_all` keeps
   arguments (ids, bodies, closures) out of the span.
2. **Error status set where sqlx errors are converted.** Every sqlx failure passes through
   `storage()` / `redemption_storage()`; there the current span gets `otel.status_code =
   "ERROR"` (declared `Empty` on the span) and an `error!` log line. `#[instrument(err)]`
   was rejected: it would also flag domain outcomes returned as `Err` (`NotFound`,
   `InvalidTransition` behind a `409`), which are not failures of the database.
3. **Span name `db.<fn>` rather than `<operation> <table>`.** Methods map to use cases, not
   tables (`reserve` touches four tables); the method name is the stable identity and
   matches the ubiquitous language.

## Risks / Trade-offs

- [Forgetting the attribute on a new method] → the telemetry test lists the methods it
  checks; a short unit test asserts every `async fn` in the two impls carries it (source
  scan, like the vocabulary test).
- [Postgres not available in the test run] → the scenario skips with a message, as the
  existing `ports.rs` tests do.

## Migration Plan

Rebuild the container; nothing else. Rollback is the previous image.
