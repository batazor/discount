# 8. Spec/core parity rule and the scope of the first iteration

Date: 2026-10-12

## Status

Accepted

## Context

The kickoff brief lists more Rust `core` types (`Rule`, `Eligibility`, `PricingPolicy`)
than Lean types for the first iteration (`Money`, `Promotion`, `apply`). Under the rules
"Lean is the source of truth" and "Rust mirrors Lean structurally" that is a
contradiction: Rust would contain concepts the specification does not have.
`PricingPolicy` is also not defined in the ubiquitous language.

Options considered:

1. Rust first, Lean catches up. Rejected by the brief's process order.
2. No rules at all in iteration 1. Would make `now` an unused parameter; a single
   `DateWindow` rule exercises eligibility, `Option` handling and `now` for almost no
   cost.
3. Order-level and line-level actions together. Doubles the allocation surface
   (line-level discounts bypass allocation) and the proof surface before the basics are
   proved.

## Decision

### Parity rule

1. No type, variant or field exists in `src/domain/` without a counterpart of the same name in
   `verification/spec/`, and vice versa. The ubiquitous language (`docs/ubiquitous-language.md`) is
   the single list of names; both implementations are checked against it in review.
2. Function decomposition is mirrored too: `validate`, `eligible`, `sortPromotions`,
   `selectExclusive`, `selectFirstMatch`, `applyOne`, `clampDiscount`, `allocate`,
   `roundDiv` exist in both with the same signatures modulo
   language. Proofs are per function, so the DRT can also run per function later.
3. JSON encodings are defined once, in `docs/ubiquitous-language.md` under each type,
   and derived in both languages. Enums encode as `{"type": "<Variant>", ...fields}`.

### Definitions that were missing

- `PricingPolicy`: the bundle of engine-wide policies that are inputs, not promotion
  data. First iteration: `{ rounding : RoundingPolicy, stacking : StackingPolicy,
  order_cap : OrderCap }` (ADR 3, ADR 4). Allocation (ADR 5) has a single method
  and is not configurable yet; when a second method appears it becomes
  `allocation : AllocationPolicy` here.
- `Rule`: a predicate on `(CartSnapshot, CustomerContext, now)`. `Eligibility` is the
  function that evaluates all rules of a promotion; a promotion is eligible when every
  rule holds (conjunction). A promotion with no rules is always eligible.

### Scope of the first iteration

In both `verification/spec/` and `src/domain/`:

- Types: `Money`, `Currency`, `BasisPoints`, `Timestamp`, `Quantity`, `RoundingPolicy`,
  `StackingPolicy`, `OrderCap`, `PricingPolicy`, `CartLine { line_id, unit_price, quantity }`,
  `CartSnapshot`, `CustomerContext` (empty), `Rule` with the single variant
  `DateWindow { starts_at, ends_at }` (half-open `[starts_at, ends_at)`, either side
  optional), `Action` with `OrderPercentage { bps }` and `OrderFixedAmount { amount }`,
  `StackingGroup { group_id, priority, mode }`, `StackingMode`,
  `Promotion { promotion_id, group_id, priority, exclusive, cap, remaining_budget, rounding, rules, action }`,
  `PricingInput`, `PricingError`, `AppliedPromotion` (with `clamped_by : Option ClampReason`),
  `SkippedPromotion`, `SkipReason`, `ClampReason`, `LineAllocation`, `LineResult`,
  `PricingResult` (with `skipped`).
- Line subtotal is `unit_price * quantity`. Discounts are computed and allocated on line
  subtotals; there is no per-unit allocation.
- Only order-level actions. Line-level targeting (`LinePercentage`, item filters,
  "buy X get Y") is the second iteration and gets its own ADR.
- Proofs: invariants 1, 2, 5 (as the brief requires), plus 4 (per group and global,
  ADR 4), 7 and the permutation part of 3 (`apply_perm_invariant`). Commutativity (rest of
  3) is claimed only where it holds and is proved in iteration 2. Idempotence (6) is not a
  theorem of the core: the core is deterministic by construction (ADR 7) and the "never
  discounted twice" guarantee is the service's idempotent `Redeem`, tested, not proved
  (decision of 2026-10-18). `docs/invariants.md` is the authoritative status table.

## Consequences

- The "ask before adding a new domain concept" rule applies to both code bases; a
  concept is added to the ubiquitous language first, then to Lean, then to Rust.
- `Rule` with one variant is deliberately an enum from day one so that adding variants
  does not change the shape of the model.
