# Design

## Context

Counters follow the budget pattern of ADR 7: state lives in the application layer and
enters the core as snapshot fields, so eligibility stays a pure function and the proofs are
untouched. The reservation transaction already locks budgets and counts `customer.uses`;
coupon and promotion counters are computed in the same place.

## Goals / Non-Goals

**Goals:** single-use and n-use codes, campaign caps by count, bulk generation, grants by
event, all consistent under concurrency. **Non-Goals:** xlsx import UI, code formats beyond
prefix + 8 characters, grant triggers beyond `cancel` and `review` (the type is open for
more), customer-facing coupon listing (next change).

## Decisions

1. **Counters are snapshot fields, not rules with hidden state.** `Coupon.uses` and
   `Promotion.uses_total` are inputs; `RequiresCoupon` and `MaxUsesTotal` read them. The
   Lean congruence for invariant 7 still holds because both are part of the input.
2. **A coupon use is a redemption that presented the code and applied the promotion.** A
   presented code whose promotion did not apply (another rule failed) is not a use.
   Computed from `input.cart.coupon_codes` and `result.applied` of stored redemptions.
3. **`uses_total` counts redemptions, not units.** Budgets cap money (ADR 4); this caps
   count. Both decrement on reserve and restore on cancel by construction, because they are
   recomputed from the redemptions table.
4. **Code alphabet** without `0/O/1/I`, 8 characters: 32^8 ≈ 10^12 codes per prefix;
   generation retries on collision with existing codes and fails after 10 attempts per code.
5. **Grant idempotency** by a unique index on `(trigger, order_id)` in a `grants` table that
   stores the issued code; a repeat reads the row.
6. **Validation errors**: `InvalidCoupon { field }` and `InvalidPromotion { field }` are new
   variants (ADR 6 edited in place); the check order appends them to the existing sequence.

## Risks / Trade-offs

- [Counting through `jsonb` on every reservation] → one aggregate query over the customer's
  and the codes' redemptions; indexes on `customer_id` exist, add a GIN index on
  `input -> 'cart' -> 'coupon_codes'` if it shows in `discount.evaluate.duration`.
- [`Evaluate` reads counters without a lock] → a dry run may say "eligible" and the
  subsequent `Redeem` may skip the promotion; `Redeem` is authoritative, as with budgets.
