# Proposal

## Why

`docs/invariants.md` row 3 still reads "commutativity claimed only where true, proof is
iteration 2". It is the last invariant in the table without a theorem. The claim matters
to operators: when two promotions in one stackable group have equal standing, swapping
their priorities must not change what the order pays, up to the rounding the document
already admits. Until it is proved, "formally verified" has an asterisk.

## What Changes

- Proofs only, in `verification/proofs`: `Proofs/Invariants/Commutativity.lean` with
  - `applyOne_unclamped`: one step with no cap, no budget, no order cap and a raw discount
    that fits the running total subtracts exactly the raw discount;
  - `commute_of_running_independent`: two such steps whose raw discounts do not depend on
    the running total give the same order total in either order; corollaries
    `fixed_amounts_commute` (any stacking policy) and `additive_percentages_commute`;
  - `sequential_percentages_commute_upto_rounding`: two percentage promotions under
    `Sequential` give order totals that differ by at most 2 minor units (one per rounded
    step), for any rounding policies.
- `docs/invariants.md` row 3 and section 3 updated; AxiomCheck lists the new theorems.

No behaviour changes: no spec delta (`skip_specs: true`), no code outside
`verification/proofs` and the documents.

## What is not claimed

General commutativity is false for this engine and stays out: swapping priorities of two
promotions changes the result whenever a cap, a budget or the order cap binds, whenever the
groups differ, and in general under `Sequential` beyond the rounding slack. The theorems
are stated at the level of one fold step pair (`applyOne`), which is where the claim is
precise; lifting to `apply` on full inputs would need the two promotions to be adjacent in
the total order, which the stated hypotheses cannot express without the rest of the input.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
_None._ (`skip_specs: true`: a proof about existing behaviour.)

## Impact

- `verification/proofs/Proofs/Invariants/Commutativity.lean`, `Proofs/Invariants.lean`,
  `AxiomCheck.lean`, `Proofs/Rounding.lean` (a two-sided rounding error lemma).
- `docs/invariants.md`, `README.md` row 3.
