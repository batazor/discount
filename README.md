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
| 3 | Stacking is deterministic (list order never matters) | stated; checked by DRT, proof next |
| 4 | Exclusive promotions apply alone; first-match groups apply at most one | proved |
| 5 | Allocation is conservative: per-line discounts sum to the order discount exactly | proved |
| 6 | Re-applying the same promotions is idempotent | iteration 2 |
| 7 | Eligibility depends only on its inputs | proved |

The Lean model compiles to a reference executable. `drt/` generates random carts and
promotions with proptest, runs the Rust core and the reference, and requires identical
results, including validation errors. The Rust core mirrors the Lean model function by
function under the same names (ADR 8), so a divergence points at a line.

Precise statements: [docs/invariants.md](docs/invariants.md). Vocabulary and the JSON
contract: [docs/ubiquitous-language.md](docs/ubiquitous-language.md). Decisions:
[docs/adr](docs/adr/README.md).

## Layout

```
spec/        Lean 4 (lake): model, theorems, reference executable (JSON lines in/out)
core/        Rust: pure domain, std + serde only, structural mirror of spec/
app/         use cases (Evaluate) and ports (PromotionRepository, Clock, EventPublisher)
adapters/    port implementations (in-memory now; axum and Postgres later)
drt/         proptest generators + core vs spec comparison
docs/        ubiquitous-language.md, invariants.md, adr/
```

## How to run

Prerequisites: [elan](https://github.com/leanprover/elan) (the toolchain is pinned in
`spec/lean-toolchain`) and a stable Rust toolchain.

Build the specification and check the proofs:

```bash
cd spec && lake build
```

Confirm that no theorem rests on `sorry`:

```bash
cd spec && lake env lean AxiomCheck.lean
```

Price a cart with the reference executable (one JSON document per line):

```bash
cd spec && lake exe discount-spec < ../drt/fixtures/samples-1.jsonl
```

Run the Rust tests, the fixture replay and the differential run (the Lean build above
must exist; `DRT_CASES` sets the number of random cases, default 256):

```bash
DRT_CASES=5000 cargo test --workspace
```

CI runs exactly these steps on every push and pull request.

## Process

Spec-driven: ubiquitous language → Lean types and invariants → Rust core → differential
testing. A new domain concept enters `docs/ubiquitous-language.md` first, then `spec/`,
then `core/`. ADRs are living documents and are edited in place.

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
`adapters/` only; `core/` stays telemetry-free.
