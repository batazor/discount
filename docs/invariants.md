# Invariants of the pricing engine

Every statement below is about `apply` from `docs/ubiquitous-language.md`, on the
`ok` branch: for all `input`, if `apply input = .ok result` then the property holds
(ADR 6). `apply` validates, then prices `normalize input` (amounts converted to the cart
currency, ADR 3); where a statement names a promotion's `cap`, `remaining_budget` or the
order cap, it means the normalized value, written `P ∈ (normalize input).promotions`. The Lean theorem names are the ones `verification/spec/` must export; the Rust side is covered
by the differential tests in `verification/drt/` (ADR 2). Nothing is claimed about inputs the generators
do not produce; the generators cover the whole validated domain (ADR 3).

Notation: `S` is `result.original_subtotal`, `D` is `result.total_discount`,
`F` is `result.final_total`, `s_i`, `d_i`, `t_i` are `subtotal`, `discount`, `total` of
`result.lines[i]`, and `p` ranges over `result.applied` with source promotion `P`.

## Status by iteration

| # | Invariant | Status | Lean theorem(s) in `verification/spec/Discount/Theorems/Invariants.lean` |
|---|-----------|--------|------------------------------------------------------------|
| 1 | Bounds per order and per line | proved | `final_total_bounds`, `line_total_bounds` |
| 2 | Caps are respected | proved | `discount_le_cap`, `discount_le_budget`, `total_discount_le_order_cap`, `total_discount_le_subtotal` |
| 3 | Deterministic stacking | stated; permutation part is the next proof, commutativity is iteration 2 | `apply_perm_invariant` (planned) |
| 4 | Exclusive never combines; first-match groups apply at most one | proved | `exclusive_alone`, `first_match_at_most_one` |
| 5 | Allocation is conservative | proved | `allocation_conserves`, `line_discount_sum`, `total_discount_eq_sum`; on `allocate` itself `allocate_sum`, `allocate_le`, `allocate_nonneg`, `allocate_length` |
| 6 | Idempotence | stated, iteration 2 | `apply_idempotent` (planned) |
| 7 | Eligibility is a function of `(now, cart, customer, coupons)` | proved | `eligible_depends_on_input` |

"Proved" means a theorem without `sorry` that `lake build` checks in CI, over the Lean
model and the `ok` branch of `apply`. A proof that turns out to be blocked is reported,
never stubbed.

## 1. Final price is within bounds

Per order: `0 ≤ F ≤ S` and `F = S - D`.
Per line, for every `i`: `0 ≤ t_i ≤ s_i` and `t_i = s_i - d_i`.

Why it holds (ADR 4, ADR 5): every step's discount is clamped to the running subtotal,
so the running total never goes negative; every allocation `a_i ≤ s_i` on the running
line subtotal, so a line never goes negative either. The step argument is the same for
`Sequential` and `Additive` because only the base for percentages differs, not the clamp.
The per-promotion rounding override (ADR 3) does not matter either: the `roundDiv`
bounds hold for every policy.

## 2. Discount never exceeds its cap

For every applied promotion `p` with source promotion `P`:
- `P.cap = some c  → p.discount ≤ c`
- `P.remaining_budget = some b → p.discount ≤ b`
- `p.discount ≤` the running subtotal at the moment `P` was applied, hence `D ≤ S`.

For the order (ADR 4), with `policy.order_cap = { amount, bps }`:
- `amount = some a → D ≤ a`
- `bps = some k → D ≤ roundDiv(policy.rounding, S * k, BPS_SCALE)`

No per-line caps exist in the first iteration (order-level actions only, ADR 8); the
per-line bound is invariant 1.

## 3. Stacking is deterministic

Two separate claims (ADR 4):

**Permutation invariance (stated, proof next).** For any permutations `π`, `σ`,
`apply (input with promotions := π input.promotions, groups := σ input.groups) = apply input`.
The caller's list orders never matter because `sortPromotions` produces the total order
`(group.priority DESC, group_id ASC, promotion.priority DESC, promotion_id ASC)` and ids
are unique after validation. The `skipped` list is in that same total order. The proof
needs two facts not yet in `verification/spec/`: a sorted permutation of a sorted list under a total,
antisymmetric order is equal to it, and `find?` by a unique key is permutation-invariant.
Both are routine and planned before the DRT lands; the DRT itself exercises permutation
invariance on every generated case.

**Commutativity (claimed only where true, proved in iteration 2).**
Swapping the *priorities* of two promotions changes the result in general. It does not
when, for a pair of adjacent promotions in the same `Stackable` group neither of which is
clamped:
- both are `OrderFixedAmount` (either policy);
- both are `OrderPercentage` under `Additive` (same base `S`; each uses its own effective
  rounding, which is independent of order);
- both are `OrderPercentage` under `Sequential`, *up to rounding*: the unrounded
  discounts commute, the rounded ones may differ by at most one minor unit per step.
General commutativity is **not** an invariant of this engine.

## 4. Exclusive promotions never combine; first-match groups apply at most one

- If some `p ∈ result.applied` comes from a promotion with `exclusive = true`, then
  `result.applied.length = 1` (`exclusive_alone`). Every other eligible promotion is in
  `skipped` with reason `ExclusiveApplied { winner := p.promotion_id }` (stated; follows
  from the partition lemma below once proved).
- For every group id `g` with `modeOf groups g = FirstMatch`, at most one applied record
  comes from a promotion of group `g` (`first_match_at_most_one`, with `appliedInGroup`
  looking the source promotion up by id). After validation `modeOf groups g` is the mode of
  the unique group with that id. Every other eligible promotion of `g` is in `skipped`
  with reason `FirstMatchTaken` (stated).

Why: `selectExclusive` returns a one-element list whenever an eligible exclusive promotion
exists; `selectFirstMatch` keeps the first promotion of each `FirstMatch` group (ADR 4).

## 5. Allocation is conservative

For every applied promotion `p`: `Σ_i p.allocation[i].amount = p.discount`,
`p.allocation.length = result.lines.length`, and `p.allocation[i].line_id = lines[i].line_id`.
Per line: `d_i = Σ_p p.allocation[i].amount`. Per order: `D = Σ_p p.discount = Σ_i d_i`.
No minor unit is lost or invented at any level.

Why: `allocate` distributes `D - Σ floor(D s_i / S)` units one at a time to the lines with
the largest remainders, so the sum is exact by construction (ADR 5). Line discounts are
sums of allocations; the order discount is the sum of step discounts.

## 6. Applying the same promotion set twice is idempotent

**Determinism (iteration 1):** `apply input = apply input`, and more usefully the
`PricingInput` JSON is a complete replay record: the same document yields the same
result in both implementations. This is what the DRT exercises on every case.

**Idempotence proper (iteration 2):** re-pricing an already priced order with the same
promotions must grant nothing more. With the first-iteration model this is *not* true as
stated: a `CartSnapshot` built from `result.lines[*].total` carries no memory of which
promotions were applied, and a percentage promotion would discount it again. The honest
formulation needs the cart to record the promotions already applied and `eligible` to skip
them. That is a new domain concept (a record of applied promotions on the snapshot) and
per the project rules it is added to the ubiquitous language only after an explicit
decision. Until then invariant 6 is stated, not proved, and ADR 8 schedules it for
iteration 2 together with `Redeem`.

## 7. Eligibility depends only on its inputs

`eligible P input` is a pure function of `(input.now, input.cart, input.customer,
input.coupons)` and `P`. The theorem states it as a congruence: two inputs that agree on
those four fields give the same eligibility for every promotion. The core has no other
inputs: no clock, no repository, no randomness (ADR 7). Coupon resolution and usage
counters are inputs supplied by the application layer, so `RequiresCoupon` and
`MaxUsesPerCustomer` are covered too.

## Structural lemmas

Proved (`verification/spec/Discount/Theorems/`):

- `validate_ok` (`Validate.lean`): `validate input = ok` implies `Valid (normalize input)`:
  after normalization every `unit_price`, `action.amount`, `cap`, `remaining_budget` and
  `order_cap.amount` is in `0 ..= MAX_MINOR_UNITS` and in `cart.currency`, every
  `quantity` in `1 ..= MAX_QUANTITY`, every `bps` in `0 ..= BPS_SCALE`, `line_id`s,
  `group_id`s and `promotion_id`s are pairwise distinct, and every promotion's `group_id`
  names a group. The bridge is `checkMoney_ok`: a money field that passes validation
  normalizes to a bounded amount in the cart currency.
- `roundDiv_nonneg`, `roundDiv_le` (`Rounding.lean`): for `0 ≤ num`, `0 < den`,
  `0 ≤ roundDiv p num den`; and for `0 ≤ base`, `0 ≤ bps ≤ BPS_SCALE`,
  `roundDiv p (base * bps) BPS_SCALE ≤ base`. (The second is why a 100 % discount never
  exceeds the base even with `HalfUp`.)
- `clampDiscount_spec` (`Fold.lean`): the clamped discount is non-negative and below every
  present bound and the running subtotal.
- `FoldInv` (`Fold.lean`): the step invariant of the fold, preserved by `applyOne` and
  established by `initStep`; it carries the per-line bounds, the running total, the
  remaining order cap and the per-record facts the invariants above are read from.
- `selectExclusive_exclusive`, `selectFirstMatch_at_most_one` (`Stacking.lean`).

Stated, not yet proved:

- `applied_skipped_partition`: every `input.promotions[i].promotion_id` occurs exactly
  once across `result.applied` and `result.skipped`, and nothing else occurs (ADR 9).
- `clamped_by_sound`: if `p.clamped_by = some r` then the bound named by `r` equals
  `p.discount` and the raw discount was strictly larger; if `none`, the raw discount was
  not reduced.
- `no_overflow`: under `Valid` every `i64` intermediate in the Rust core is below `2^63`
  and every `i128` intermediate below `2^127`. Stated over the Lean `Int` model as
  explicit bounds; it is what makes `PricingError::Overflow` unreachable (ADR 3).

## What is explicitly not claimed

- Commutativity of percentage promotions under `Sequential` beyond rounding.
- Anything about budget or usage consumption across orders (`Redeem`, ADR 7).
- Anything about inputs that fail `validate` beyond "the first error in the fixed order is
  reported" (which the DRT checks but no theorem states).
- Equivalence of Lean and Rust: it is tested differentially, not proved.
