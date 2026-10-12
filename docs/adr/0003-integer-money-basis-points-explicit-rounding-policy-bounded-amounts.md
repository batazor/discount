# 3. Integer money, basis points, explicit rounding policy, bounded amounts

Date: 2026-10-12

## Status

Accepted

Depends on [2. Lean 4 model and differential random testing instead of verifying Rust directly](0002-lean-4-model-and-differential-random-testing-instead-of-verifying-rust-directly.md)

## Context

Prices and discounts must be exact. Percentage discounts produce fractions of a minor
unit and a rounding rule must be chosen explicitly. The Lean model works on unbounded
`Int`; the Rust core on `i64`. Without a bound on inputs the two can disagree on
overflow, and the equivalence argument of ADR 2 breaks.

Touches invariants 1 (bounds), 2 (caps), 5 (allocation conserves minor units).

Options considered:

1. Decimal type (`rust_decimal`, Lean `Rat`). Rejected: violates the "no deps in core"
   rule, and rounding would be hidden inside library semantics.
2. `Int64` in Lean to match Rust bit-for-bit. Rejected for now: far fewer lemmas, proofs
   by `bv_decide` become slow and opaque. Bounded `Int` gives the same guarantee with
   readable proofs.
3. Rounding policy per promotion only. Rejected: a merchant-wide default is still
   needed, and the order cap (ADR 4) is not any promotion's computation. Chosen instead:
   engine-wide policy plus an optional per-promotion override (below).
4. Percent as integer `0..=100`. Rejected: too coarse for "12.5 % off".

## Decision

### Money

- `Money` is a newtype over a signed 64-bit integer in minor units (cents, pence,
  kopecks) plus a `Currency` (ISO 4217 alphabetic code, uppercase, 3 letters).
  Lean: `structure Money where amount : Int; currency : Currency`.
  Rust: `struct Money { amount: i64, currency: Currency }`.
- No floating point anywhere in `spec/`, `core/`, `app/`. Adapters may format for
  display but never compute.
- Arithmetic on `Money` with different currencies is not defined; the pricing function
  rejects mixed-currency input (ADR 6). The model never has to add two currencies.

### Percentages

- Percentages are `BasisPoints`: an integer in `0 ..= 10_000` (1 bp = 0.01 %).
  A percentage discount of `bps` on `amount` is `round(amount * bps / 10_000)` under
  the active `RoundingPolicy`.

### Rounding

- `RoundingPolicy` is a domain type and an input to the pricing function via
  `PricingPolicy` (ADR 8). A promotion may override it: `Promotion.rounding : Option
  RoundingPolicy`; the **effective** policy for a promotion's percentage computation is
  the override if present, else `PricingPolicy.rounding`. The order-cap percentage (ADR 4)
  always uses `PricingPolicy.rounding`. The policy is never a property of a currency.
- The override does not weaken the proofs: invariants 1, 2 and 5 rest on `roundDiv`
  bounds that hold for every policy, and the commutativity claims of ADR 4 are unaffected
  (under `Additive` each discount is computed independently from the same base; under
  `Sequential` commutativity was never claimed beyond rounding). It exists because the
  promocode service this engine replaces configures campaigns individually.
- Variants in the first iteration: `HalfUp`, `HalfEven`, `Down`. `Down` rounds toward
  zero; all amounts in the engine are non-negative, so `Down` equals floor. `Up` is not
  included; add it only when a use case appears.
- Default policy when nothing is configured: `Down` (`DEFAULT_ROUNDING`). It is what the
  application layer puts into `PricingPolicy.rounding`; the core itself has no defaults.
  `Down` matches the `ROUND_DOWN` used everywhere in the promocode service being replaced.
- Rounding happens once per discount computation, on the final quotient, never on
  intermediate values. Allocation (ADR 5) is integer-exact and does not round.
- Lean defines `roundDiv (policy) (num den : Int) : Int` with `den > 0` and proves the
  bounds invariant 1 needs (`0 ≤ num → 0 ≤ roundDiv p num den`, and
  `roundDiv p num den ≤ num / den + 1`). Rust implements the same three cases with
  integer ops only.

### Bounds

- Every monetary amount on input (line subtotals, fixed discount amounts, caps,
  remaining budgets) must satisfy `0 ≤ amount ≤ MAX_MINOR_UNITS` with
  `MAX_MINOR_UNITS = 10^14`. The order subtotal (sum of lines) must also be
  `≤ MAX_MINOR_UNITS`. Violations are rejected before pricing (ADR 6).
- Rationale: the largest `i64` intermediate is `subtotal * bps ≤ 10^14 * 10^4 = 10^18 < 2^63`.
  Allocation computes `discount * line_subtotal ≤ 10^28`, which does not fit `i64`;
  allocation therefore uses `i128` intermediates in Rust and unbounded `Int` in Lean.
  A product of two bounded `i64` values cannot overflow `i128`.
- Lean stays on `Int`. Rust uses checked arithmetic and maps overflow to
  `PricingError::Overflow`. Under the bounds above this arm is unreachable; a DRT
  divergence on it means the bounds are wrong, which is exactly what we want to hear.
- `10^14` minor units is 10^12 major units. If a real currency needs more, raise the
  bound and redo the arithmetic above in a superseding ADR.

## Consequences

- All generators in `drt/` must respect the bounds; they define the domain the
  equivalence argument covers.
- Formatting (`12.50 EUR`) is an adapter concern and needs the currency's exponent;
  the core does not know about exponents at all.
- Changing a rounding policy on a live system changes prices; it belongs in the
  configuration that is versioned with the promotion set, not in code.
