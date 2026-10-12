# 9. Evaluation result reports skipped promotions and clamp reasons

Date: 2026-10-12

## Status

Accepted

## Context

With the ubiquitous language as first written, a promotion that did not apply simply did
not appear in the result. Callers of a coupon-style API need to tell the customer *why*
("code expired", "another code already applied"), and merchants need to know when a
discount was cut by a cap. The promocode service being replaced carries about forty error
codes for exactly this. Voucherify's validation response returns applicable, inapplicable
and skipped redeemables each with a reason; Talon.One reports per-campaign effects.

Options considered:

1. Leave it to the application layer to re-derive reasons. Rejected: it would duplicate
   eligibility logic outside the verified core, the exact thing ADR 6 forbids.
2. Report reasons from the core (chosen). Pure data, deterministic, cheap to compute during
   the stages that already make the decisions.

## Decision

- `PricingResult.skipped : List SkippedPromotion`, `SkippedPromotion = { promotion_id, reason }`,
  in total order. `SkipReason`:
  - `NotEligible { rule_index }` — index of the first rule that did not hold.
  - `ExclusiveApplied { by }` — an exclusive promotion `by` was applied alone.
  - `FirstMatchTaken { by }` — another promotion `by` won the `FirstMatch` group.
- Every input promotion appears in exactly one of `applied` and `skipped`
  (`applied_skipped_partition`, a lemma in `docs/invariants.md`). A promotion whose
  clamped discount is zero is still *applied*, with `clamped_by` explaining why.
- `AppliedPromotion.clamped_by : Option ClampReason`,
  `ClampReason = Cap | Budget | OrderCap | Running`. It is `none` when the raw discount
  was not reduced; otherwise the first of `Cap, Budget, OrderCap, Running` (in this fixed
  order) whose bound equals the clamped value. Ties are therefore deterministic.

## Consequences

- The result is self-explaining and replayable; the HTTP adapter maps `SkipReason` to
  user-facing codes without re-evaluating anything.
- Two more types and two more fields in both code bases; the DRT compares them like
  everything else.
