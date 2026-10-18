# Tasks

## 1. Spans (`src/infrastructure/`)

- [x] 1.1 In `src/infrastructure/src/persistence/postgres.rs` add `#[tracing::instrument(name = "db.<fn>", skip_all, fields(otel.kind = "client", otel.status_code = tracing::field::Empty, db.system = "postgresql", db.operation.name = "<fn>"))]` to every method of the `PromotionCatalog` and `RedemptionRepository` impls and record `otel.status_code = "ERROR"` plus an error log in `storage()`; verify `cargo clippy -p discount-infrastructure --all-targets -- -D warnings`
- [x] 1.2 In `src/infrastructure/src/telemetry.rs` add `SPAN_DB_PREFIX` (`db.`), `SPAN_ATTR_DB_SYSTEM`, `SPAN_ATTR_DB_OPERATION` to the constants and `VOCABULARY`, plus a unit test that scans `persistence/postgres.rs` and fails on an `async fn` inside the trait impls without `#[tracing::instrument(name = "db.`; verify `cargo test -p discount-infrastructure --lib`
- [x] 1.3 In `src/infrastructure/tests/telemetry.rs` add a `DATABASE_URL`-gated scenario: `GET /v1/policy` through the router on `PgStore` exports `db.get_policy` (kind client, attributes) under the server span, and `reserve` exports `db.reserve`; verify `DATABASE_URL=postgres://discount:discount@localhost:5432/discount_test cargo test -p discount-infrastructure --test telemetry`

## 2. Vocabulary (`docs/`)

- [x] 2.1 Add the `db.<method>` span family with `db.system` and `db.operation.name` to the Telemetry vocabulary in `docs/ubiquitous-language.md`; verify `cargo test -p discount-infrastructure --lib vocabulary`
