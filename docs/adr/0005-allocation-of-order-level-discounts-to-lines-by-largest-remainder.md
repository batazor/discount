# 5. Allocation of order-level discounts to lines by largest remainder

Date: 2026-10-12

## Status

Accepted

Depends on [4. Stacking: total order, groups, exclusivity and order-level cap](0004-stacking-total-order-groups-exclusivity-and-order-level-cap.md)


## Context

Promotions in the first iteration act on the order (ADR 8), but downstream systems
(tax, returns, accounting) need the discount per line. Splitting an integer amount
proportionally produces fractions; naive rounding per line loses or invents minor units.
Invariant 5 demands that the per-line discounts sum exactly to the order-level discount.

Touches invariants 1 (per line) and 5.

Options considered:

1. Largest remainder, ties by position. Chosen. Standard apportionment method, exact,
   O(n log n), explainable.
2. Remainder to the last line. Exact and O(n), but systematically favours one line and
   can give it up to `n - 1` units more than its proportional share.
3. Round each share independently. Loses or invents units; violates invariant 5.
4. Allocate once at the end of all promotions. Loses per-promotion attribution on lines,
   which budgets and refunds need.

## Decision

For an order-level discount `D` on lines with subtotals `s_1 .. s_n` and `S = Σ s_i`,
with `0 ≤ D ≤ S` guaranteed by ADR 4:

1. If `S = 0` then `D = 0` and every line gets `0`.
2. Otherwise each line gets a base share `b_i = floor(D * s_i / S)` and a remainder
   weight `r_i = (D * s_i) mod S`.
3. `R = D - Σ b_i` minor units remain, with `0 ≤ R < n`. They are distributed one unit
   each to the `R` lines with the largest `r_i`; ties are broken by line position
   (earlier line first).
4. The allocation of line `i` is `a_i = b_i + (1 if chosen else 0)`.

Properties, to be proved in Lean:

- Conservation: `Σ a_i = D` (invariant 5).
- Per-line bound: `a_i ≤ s_i`, hence the line price after discount is `≥ 0` (invariant 1
  per line). Sketch: `b_i ≤ s_i * D / S ≤ s_i`. A line is chosen only if `R > 0`, which
  implies its `r_i > 0` (chosen lines have the largest remainders and some remainder is
  positive); `r_i > 0` means `b_i < s_i * D / S ≤ s_i`, so `b_i + 1 ≤ s_i`.
- Lines with `s_i = 0` receive `0`.
- Determinism: the result is a function of `(D, [s_i])` only.

Allocation runs per applied promotion (ADR 4), so the result records, for each
promotion, its allocation to each line. Running line subtotals are decreased by their
allocation after each promotion; by conservation the running order subtotal equals the
sum of running line subtotals, so the two views never diverge.

Integer width: `D * s_i ≤ 10^28` exceeds `i64`; Rust uses `i128` for the product and
the division, Lean uses `Int` (ADR 3).

## Consequences

- Line order is semantically significant (tie-break). The cart snapshot is a list, not a
  set, and the DRT generators must preserve order across the JSON round-trip.
- The per-line bound proof needs `D ≤ S`, which ADR 4's clamp provides; the two ADRs
  are coupled and are accepted together.
