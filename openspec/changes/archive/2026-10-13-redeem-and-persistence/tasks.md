# Tasks

## 1. Application layer (`app/`)

- [x] 1.1 Make the ports async with `async-trait`, add `PromotionCatalog` (policy, groups, promotions, budgets, snapshot) and `RedemptionRepository` (reserve with pricing closure, confirm, cancel, get) with `Redemption {order_id, status, input, result}` and `RedemptionError {NotFound, InvalidTransition, Pricing, Storage}`; verify `cargo build -p discount-app`
- [x] 1.2 Implement `Evaluate` (async), `Redeem`, `ConfirmRedemption`, `CancelRedemption` use cases in `app/src/`; verify the shared port scenarios (`adapters/tests/ports.rs`, in-memory store) cover reserve, idempotent repeat, confirm, cancel, invalid transition

## 2. In-memory adapter (`adapters/src/memory.rs`)

- [x] 2.1 Implement `PromotionCatalog` and `RedemptionRepository` in memory (mutex, seeded from the fixture file); verify a generic adapter test module (`adapters/tests/ports.rs`) passes all `redemption` and `promotion-catalog` scenarios against it, including the concurrent-reservation scenario

## 3. Postgres adapter (`adapters/src/postgres.rs`, `adapters/migrations/`)

- [x] 3.1 Add sqlx (postgres, runtime-tokio, json, migrate) and the migration `0001_catalog.sql` (pricing_policy, stacking_groups, promotions, promotion_budgets, redemptions); verify `sqlx::migrate!` applies on an empty database
- [x] 3.2 Implement `PgCatalog` (CRUD, snapshot with budgets) and `PgRedemptions` (reserve in a transaction with `FOR UPDATE`, confirm, cancel restoring budgets, idempotency); verify the generic port test module passes against Postgres when `DATABASE_URL` is set (skipped with a notice otherwise)

## 4. HTTP, config, wiring (`adapters/`)

- [x] 4.1 Add `DATABASE_URL` to `config.rs` and choose the adapter at start-up in `discount-server` (Postgres when set, else in-memory from the fixture; migrations at start); verify the server starts both ways and `/healthz` answers
- [x] 4.2 Add the redemption routes (`/v1/redeem`, `/v1/redemptions/{order_id}`, `/confirm`, `/cancel`) with the documented status codes; verify axum tests on the in-memory adapter cover redeem, idempotent repeat, 422, 404, 409
- [x] 4.3 Add the catalogue routes (`/v1/policy`, `/v1/groups[/{id}]`, `/v1/promotions[/{id}]`, `/v1/promotions/{id}/budget`); verify axum tests cover create-and-price, archive-hides, budget-flows-into-pricing

## 5. Telemetry and dashboards (`adapters/src/telemetry.rs`, `deploy/`)

- [x] 5.1 Add `discount.redemptions{action,outcome}` and the `discount.budget.remaining{promotion_id,currency}` observable gauge fed by a background refresh; verify an in-memory exporter test sees both after a reservation, and the vocabulary test passes with the new names in `docs/ubiquitous-language.md`
- [x] 5.2 Add a "Budgets" row to `deploy/grafana/dashboards/promotions.json` (remaining per promotion, reservations by action/outcome); verify the panels return data on the local stack after `deploy/smoke.sh` posts a redemption

## 6. Stack and CI (`deploy/`, `.github/`)

- [x] 6.1 Add `postgres:16` to `deploy/compose.yaml` with `DATABASE_URL` on the service and extend `deploy/smoke.sh` with a redeem/confirm round trip; verify `docker compose up -d --build` is healthy and the smoke passes
- [ ] 6.2 Add a Postgres service and `DATABASE_URL` to `.github/workflows/ci.yml`, document the redemption API and the database mode in `README.md`; verify CI is green and the README commands run

## Workflow follow-up

- Archive the change after review: `openspec archive redeem-and-persistence`.
