# Tasks

## 1. Contract first (`api/`, `docs/`)

- [x] 1.1 Extend `api/openapi.yaml`: `CustomerContext.attributes/allowlisted`, three `Rule` variants, schema `Allowlist`, allowlist operations; verify Redocly lint has no errors
- [x] 1.2 `docs/ubiquitous-language.md`: customer fields, rules, validation order, application-layer `Allowlist`; verify `openspec validate customer-attributes`

## 2. Core (`verification/spec`, `src/domain`, `verification/drt`)

- [x] 2.1 Lean: fields, rules, `checkCustomer` duplicates, JSON, `#guard` examples; verify `lake build` in spec and proofs, AxiomCheck
- [x] 2.2 Rust mirror with unit tests; verify `cargo test -p discount-domain`
- [x] 2.3 DRT generators (attributes, allowlisted, three rules) and a duplicate-allowlist mutation, fixtures; verify `DRT_CASES=3000 cargo test -p discount-drt`

## 3. Service (`src/application`, `src/infrastructure`, `src/client`)

- [x] 3.1 Ports and stores (migration `0006_allowlists.sql`), `Evaluate`/`Redeem` fill `allowlisted`; verify the shared port scenario on both backends
- [x] 3.2 HTTP routes and the generated client; verify axum tests, `openapi_covers_every_route`, the contract test
- [x] 3.3 Smoke: allowlist on a promotion, evaluate for a listed and an unlisted customer; verify `deploy/smoke.sh`

## Workflow follow-up

- Archive the change after review: `openspec archive customer-attributes`.
