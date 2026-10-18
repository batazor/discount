# Design

## Statement

Work on `applyOne i S st p`, the one step of the fold. A step is *unclamped* when the
promotion has no `cap` and no `remaining_budget`, the state has no order cap and the raw
discount is at most the running total. Then (`applyOne_unclamped`) the step subtracts the
raw discount exactly: `lineRunning.sum` drops by `rawDiscount`, every line stays
non-negative, the order cap stays `none`. Everything else follows from this and the
allocation lemmas (`allocate_sum`, `allocate_le`, `allocate_length`).

1. **Running-independent raw discounts.** If `rawDiscount` of both promotions is the same
   for every running total, the two orders give the same `lineRunning.sum`: both are
   `R - da - db`. Fixed amounts are such under either policy; percentages are such under
   `Additive` (they read the original subtotal `S`).
2. **Sequential percentages.** With `da = rd(R·pa)`, `db' = rd((R − da)·pb)` and the
   mirror image, the integer identity
   `den²(da + db') = den·R·(pa + pb) − R·pa·pb + ε₁(den − pb) + den·ε₂`
   with `ε = den·rd(x) − x` holds in both orders, and each `ε` lies in `(−den, den/2]`
   for every rounding policy (`roundDiv_err`). The pair term lies in `(−2den², den²]`, so
   the two totals differ by less than `3den²/den²`: at most 2 minor units.

## Why not `apply` on whole inputs

"Swap the priorities of two adjacent promotions" changes `sortPromotions`, so the fold
visits them in the other order; everything before and after the pair is identical. The
pair-level theorem is exactly that statement, without carrying the rest of the input and
without needing the (false in general) claim for non-adjacent or clamped promotions. The
per-line allocation is not claimed commutative: largest-remainder allocation of the second
discount runs on a different running vector in the two orders, so lines may differ by a
unit while the order total does not.

## Risks / Trade-offs

- [Nonlinear arithmetic] → core `omega` treats products of variables as atoms; the proof
  supplies the needed product bounds through `Int.mul_le_mul_of_nonneg_right` and the
  distributivity rewrites explicitly. No Mathlib.
