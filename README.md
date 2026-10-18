# discount

A standalone discount and promotion service whose pricing core is formally specified in
Lean 4 and checked against the production Rust code by differential random testing.

## What it is

Given a cart snapshot, a customer context, the current time and a set of promotions, the
engine returns the discounted order: per-line totals, which promotions applied and why the
others did not, and how every minor unit of discount was allocated to lines. Promotions
are campaign-style (Talon.One / Voucherify / Sylius): rules decide eligibility, an action
decides the effect, groups and priorities decide stacking, caps and budgets bound the
result. Money is integer minor units; rounding is an explicit policy.

The service around the engine (HTTP, Postgres, admin, budget redemption) is ordinary Rust
with ordinary tests and is not in the verified scope.

## Why it is verified

Stacking, rounding and allocation are where promotion engines quietly lose or invent money.
Instead of describing the semantics in prose, the engine is specified as a Lean 4 model
with theorems for its invariants (ADR 2):

| # | Invariant | Status |
|---|-----------|--------|
| 1 | Final price is in `[0, original]`, per order and per line | proved |
| 2 | No discount exceeds its cap, budget or the order cap | proved |
| 3 | Stacking is deterministic (list order never matters) | proved for permutations of promotions and groups; commutativity claimed only where true |
| 4 | Exclusive promotions apply alone; first-match groups apply at most one | proved |
| 5 | Allocation is conservative: per-line discounts sum to the order discount exactly | proved |
| 6 | Re-applying the same promotions is idempotent | iteration 2 |
| 7 | Eligibility depends only on its inputs | proved |

The Lean model compiles to a reference executable. `verification/drt/` generates random carts and
promotions with proptest, runs the Rust core and the reference, and requires identical
results, including validation errors. The Rust core mirrors the Lean model function by
function under the same names (ADR 8), so a divergence points at a line.

Precise statements: [docs/invariants.md](docs/invariants.md). Vocabulary and the JSON
contract: [docs/ubiquitous-language.md](docs/ubiquitous-language.md). Decisions:
[docs/adr](docs/adr/README.md).

## Layout

```
src/domain/            Rust: pure domain, std + serde only, structural mirror of the Lean model
src/applicationlication/             use cases (Evaluate, Redeem, Confirm, Cancel) and ports
src/infrastructure/        axum HTTP, Postgres (sqlx), in-memory store, OpenTelemetry
verification/spec/   Lean 4 (lake): the model, one module per concept, and the reference executable
verification/proofs/ Lean 4 (lake): the theorems about the model, one module per invariant
verification/drt/    proptest generators + core vs reference comparison
docs/                ubiquitous-language.md, invariants.md, adr/
openspec/            behaviour specs with scenarios (OpenSpec), archived changes
deploy/              compose stack, Grafana dashboards, fixtures, smoke script
```

Product code lives under `src/`; everything that checks it (the Lean model, its proofs and
the differential tests) under `verification/`. Neither depends on the other direction.

## How to run

Prerequisites: [elan](https://github.com/leanprover/elan) (the toolchain is pinned in
`verification/spec/lean-toolchain`) and a stable Rust toolchain.

Build the specification, then the proofs (a separate lake package that depends on it):

```bash
cd verification/spec && lake build
```

```bash
cd verification/proofs && lake build
```

Confirm that no theorem rests on `sorry`:

```bash
cd verification/proofs && lake env lean AxiomCheck.lean
```

Price a cart with the reference executable (one JSON document per line):

```bash
cd verification/spec && lake exe discount-spec < ../drt/fixtures/samples-1.jsonl
```

Run the Rust tests, the fixture replay and the differential run (the Lean build above
must exist; `DRT_CASES` sets the number of random cases, default 256):

```bash
DRT_CASES=5000 cargo test --workspace
```

CI runs exactly these steps on every push and pull request.

## Process

Spec-driven: ubiquitous language → Lean types and invariants → Rust core → differential
testing. A new domain concept enters `docs/ubiquitous-language.md` first, then `verification/spec/`,
then `src/domain/`. ADRs are living documents and are edited in place.

## HTTP API: spec first

The contract is [api/openapi.yaml](api/openapi.yaml) (OpenAPI 3.0.3). It is written first,
the server is checked against it, and clients are generated from it (ADR 10). A running
service serves the document at `/openapi.yaml` and Swagger UI at `/docs`. Behaviour is
specified with scenarios in `openspec/specs/`; the JSON shapes are the ubiquitous language.

- Rust: the workspace crate `discount-client` (`src/client`) is generated at build time;
  add it as a path dependency and use `discount_client::Client`.
- Other languages, from the same file (requires Docker):

```bash
docker run --rm -v "$PWD:/local" openapitools/openapi-generator-cli generate -i /local/api/openapi.yaml -g typescript-fetch -o /local/clients/typescript
```

Lint the document the way CI does:

```bash
npx --yes @redocly/cli@latest lint api/openapi.yaml
```

`discount-server` runs the engine behind that API. Without `DATABASE_URL` it uses an
in-memory catalogue seeded from `deploy/promotions.json`; with it, catalogue and redemptions
live in Postgres (migrations run at start-up).

## Observability

The service exports traces, metrics and logs over OpenTelemetry (OTLP) and ships Grafana
dashboards as code. Run the whole chain locally:

```bash
docker compose -f deploy/compose.yaml up -d --build
```

Then open [http://localhost:3000](http://localhost:3000) (folder **Discount**: engine
overview, promotions, validation errors) and check that signals arrive end to end:

```bash
deploy/smoke.sh
```

The metric and attribute names are listed under "Telemetry vocabulary" in
[docs/ubiquitous-language.md](docs/ubiquitous-language.md). Instrumentation lives in
`src/infrastructure/` only; `src/domain/` stays telemetry-free.
