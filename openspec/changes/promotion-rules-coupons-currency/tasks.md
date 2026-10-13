# Tasks

## 1. Ubiquitous language and ADRs (`docs/`)

- [x] 1.1 Add `Attribute`, `Coupon`, `ExchangeRate`, the new `CartSnapshot` / `CustomerContext` fields, the seven rules, `normalize`, the new errors and the extended validation order to `docs/ubiquitous-language.md`; verify every name used by the specs appears there
- [x] 1.2 Edit ADR 3 (conversion rounding `Down`, rate bounds) and ADR 6 (new error variants, check order) in place and restate `docs/invariants.md` on the normalized input; verify the invariants table still names every Lean theorem

## 2. Lean specification (`spec/`)

- [x] 2.1 Extend `Discount/Types.lean` (new structures, fields, rule variants, errors) and `Discount/Json.lean`; verify `lake build` and the fixture round trip through `discount-spec`
- [x] 2.2 Extend `Discount/Validate.lean` with rates, coupons, customer context, rule parameters and money conversion checks in the normative order, and add `normalize` to `Discount/Apply.lean`; verify `lake build`
- [x] 2.3 Extend `Discount/Eligibility.lean` with the seven rules; verify `lake build` and hand-written examples in a `#eval` block match the spec scenarios
- [x] 2.4 Re-prove: `validate_ok` for the extended checks, `validate_ok_normalized : validate i = ok → Valid (normalize i)`, restate every theorem in `Theorems/Invariants.lean` on `normalize i`, update `eligible_depends_on_input` to `(now, cart, customer, coupons)`; verify `lake build` and `AxiomCheck.lean` without `sorryAx`

## 3. Rust core and DRT (`core/`, `drt/`)

- [x] 3.1 Mirror types, validation, normalization and eligibility in `core/`; verify `cargo test -p discount-core` including new unit tests for each rule and the conversion scenarios
- [x] 3.2 Extend the DRT generators and mutations (`drt/src/generators.rs`) for tags, attributes, coupon codes, customer context, coupons, rates and the new rules; add fixtures; verify `DRT_CASES=5000 cargo test -p discount-drt` passes and an injected rule bug is caught

## 4. Application and adapters (`app/`, `adapters/`)

- [x] 4.1 Extend `CatalogSnapshot` and `PromotionCatalog` with coupons and exchange rates, `pricing_input` with coupon filtering, `RedemptionRepository` with `customer_id` and `uses_for_customer`, and `Evaluate`/`Redeem` to fill `customer.uses`; verify `cargo build --workspace`
- [x] 4.2 Implement the new port methods in `adapters/src/memory.rs` and `adapters/src/postgres.rs` with migration `0002_coupons_rates.sql`; verify the shared port scenarios (coupons, rates, uses counting) pass on both backends
- [x] 4.3 Add HTTP routes `/v1/coupons[/{code}]` and `/v1/exchange-rates[/{from}/{to}]` and update fixtures (`deploy/promotions.json`, `drt/fixtures`); verify axum tests cover the coupon and rate scenarios and the smoke script still passes

## 5. Integration

- [x] 5.1 Run the full suite (`lake build`, `cargo test --workspace` with `DATABASE_URL`, `deploy/smoke.sh`) and update `README.md`; verify all green

## Workflow follow-up

- Archive the change after review: `openspec archive promotion-rules-coupons-currency`.
