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

## Running as a service

`discount-server` exposes the engine over HTTP. Without `DATABASE_URL` it serves an
in-memory catalogue seeded from `deploy/promotions.json`; with `DATABASE_URL` set the
catalogue and the redemptions live in Postgres (migrations run at start-up).

| Route | Purpose |
|-------|---------|
| `POST /v1/evaluate` `{cart, customer}` | dry run, returns a `PricingResult` |
| `POST /v1/redeem` `{order_id, cart, customer}` | reserve: prices under a budget lock, stores the redemption, decrements budgets; idempotent by `order_id` |
| `POST /v1/redemptions/{order_id}/confirm` / `cancel` | lifecycle `reserved → confirmed`, `reserved \| confirmed → cancelled` (cancel restores budgets) |
| `GET /v1/redemptions/{order_id}` | stored redemption with its replayable input |
| `GET\|PUT /v1/policy`, `/v1/groups[/{id}]`, `/v1/promotions[/{id}]`, `/v1/promotions/{id}/budget` | catalogue management |
| `GET /v1/coupons`, `GET\|PUT\|DELETE /v1/coupons/{code}` | coupon codes that turn a promotion on (`RequiresCoupon`) |
| `GET /v1/exchange-rates`, `PUT\|DELETE /v1/exchange-rates/{from}/{to}` | integer rates used to convert promotion amounts to the cart currency |

Pricing errors are `422 {"error": PricingError}`, malformed bodies `400 {"decode_error"}`,
unknown ids `404`, rejected transitions `409`. The behaviour is specified with scenarios in
`openspec/specs/`.

A cart carries `tags`, `attributes` and `coupon_codes`; the customer context carries
`customer_id`, `completed_orders` and `uses`. The service resolves the codes to stored
coupons and adds the customer's reserved and confirmed redemptions to `uses`, so rules
such as `RequiresCoupon`, `MaxUsesPerCustomer`, `MaxCompletedOrders`, `TagsMatch`,
`AttributeIn`, `AttributeDateWindow` and `OrderTotalRange` are evaluated by the verified
core (see "Rule" in [docs/ubiquitous-language.md](docs/ubiquitous-language.md)).
Promotion amounts in another currency are converted before pricing with the stored rates,
rounding down; the proofs are stated on that normalized input.

The adapter tests run against Postgres when `DATABASE_URL` is set (the compose stack
provides one at `postgres://discount:discount@localhost:5432/discount`); otherwise the
Postgres scenarios are skipped and the in-memory store is tested alone.

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
