# Tasks

## 1. Contract first (`api/`, `docs/`)

- [x] 1.1 Extend `api/openapi.yaml` (Coupon `max_uses`/`uses`, Promotion `uses_total`, rule `MaxUsesTotal`, errors, `/v1/coupons/generate`, `/v1/coupons/grant`, `/v1/grant-rules`); verify Redocly lint has no errors
- [x] 1.2 Update `docs/ubiquitous-language.md` (fields, rule, errors, validation order) and ADR 6; verify the telemetry vocabulary test and `openspec validate` pass

## 2. Core (`verification/spec`, `src/domain`, `verification/drt`)

- [x] 2.1 Lean: new fields, `MaxUsesTotal`, remaining-uses in `couponValid`, validation checks, JSON, `#guard` examples, theorem plumbing; verify `lake build` and `AxiomCheck`
- [x] 2.2 Rust mirror; verify `cargo test -p discount-domain` with new unit tests
- [x] 2.3 DRT generators and mutations for the counters; verify `DRT_CASES=3000 cargo test -p discount-drt`

## 3. Service (`src/application`, `src/infrastructure`, `src/client`)

- [x] 3.1 Ports: `uses_for_codes`, `uses_total`, grant rules CRUD, `generate_coupons`, `grant_coupon`; snapshot fills counters (in-memory and Postgres, migration `0003_grant_rules.sql`); verify shared port scenarios incl. concurrent single use on both backends
- [x] 3.2 HTTP routes and the generated client; verify axum tests and the contract test cover generate, grant (idempotent) and grant rules
- [x] 3.3 Smoke: generate a single-use code, redeem twice, assert the second skips; verify `deploy/smoke.sh` on the compose stack

## Workflow follow-up

- Archive the change after review: `openspec archive coupon-lifecycle`.
