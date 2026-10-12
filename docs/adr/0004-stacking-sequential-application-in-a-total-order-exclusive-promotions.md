# 4. Stacking: sequential application in a total order; exclusive promotions

Date: 2026-10-12

## Status

Accepted

Is depended on by [5. Allocation of order-level discounts to lines by largest remainder](0005-allocation-of-order-level-discounts-to-lines-by-largest-remainder.md)

## Context

Several promotions can be eligible for one cart. The result must be deterministic and
explainable, and the invariants (final price in `[0, original]`, caps respected,
idempotence) must be provable. The open question is what a second percentage discount is
computed on, and how ties and exclusivity are resolved.

Industry practice is split, so this is a genuine choice, not a convention to copy:

- Shopify: when several percentage *order* discounts combine, each is computed on the
  original subtotal (additive). An order discount after a product discount is computed on
  the already reduced subtotal.
- WooCommerce: a store-level switch, "Calculate coupon discounts sequentially". Off (the
  default) computes every coupon on the original price; on computes each coupon on the
  running subtotal left by the previous one.
- commercetools: cart discounts apply in rank order on the already discounted price
  (sequential). A discount can carry `StopAfterThisDiscount` to end processing.
- Magento / Adobe Commerce: cart price rules apply by priority, same-priority rules by id,
  with a per-rule "Discard subsequent rules" flag to stop stacking.
- Sylius (our model reference): priority order, exclusive flag stops processing.

Two of the five expose a stop-after flag (our `exclusive`), all five use an explicit
priority, and the base for percentages differs. WooCommerce shows that offering both
bases as a policy is an accepted product decision.

Options considered:

1. Sequential only. Matches commercetools and "20 % off, then an extra 20 % off"; each
   step has a well-defined base; proofs are stepwise.
2. Additive only. Matches Shopify's order-discount rule; "20 % + 20 % = 40 %" is what
   some merchants expect; the clamp at zero needs an attribution rule anyway.
3. Both, selected by an engine-wide policy. Chosen. The two modes differ only in the base
   used for percentages; clamping, allocation and the total order are shared, so the
   proof surface grows by one case split, not by a second engine.
4. Best-for-customer exclusive selection (evaluate every exclusive promotion, pick the
   largest discount). Rejected: requires computing all alternatives, makes the winner
   depend on cart contents in a way that is hard to explain, and still needs a tie-break.
5. Tie-break on anything other than id (creation time, name). Rejected: not an input of
   the pricing function.

## Decision

### Total order

Eligible promotions are sorted by `(priority DESC, promotion_id ASC)`. `priority` is an
integer on the promotion; `promotion_id` is a unique, totally ordered identifier (string
compared bytewise). Two promotions never share a position, so the order is total and the
result cannot depend on the order in which the repository returned them.

### Stacking policy

`StackingPolicy` is a domain type and part of `PricingPolicy` (ADR 8), next to
`RoundingPolicy`. Variants:

- `Sequential` (default): each promotion's percentage is computed on the running
  order subtotal (original subtotal minus all discounts applied so far).
- `Additive`: each promotion's percentage is computed on the original order subtotal.

Everything else is identical in both modes. With `base` chosen by the policy:

- `OrderPercentage bps`: `discount = roundDiv(rounding, base * bps, 10_000)`.
- `OrderFixedAmount m`: `discount = m.amount`.

The raw discount is then clamped: `discount := min(discount, cap, remaining_budget, running)`
where an absent `cap` or `remaining_budget` does not constrain, and `running` is the
current remaining order subtotal in both modes. The result is allocated to lines (ADR 5)
and `running` decreases by `discount`.

In `Additive` mode the clamp to `running` means the *last* promotions in the total order
absorb the shortfall when the sum of discounts would exceed the subtotal; attribution stays
per promotion and deterministic.

Consequences for the invariants:

- Invariant 1 holds step by step in both modes: `0 ≤ discount ≤ running`, so `running`
  never goes below zero and never increases.
- Invariant 2 holds by the clamp, by construction.
- Invariant 3 (order-independence) is claimed and proved only where true. In `Additive`
  mode all unclamped percentage discounts commute exactly (same base). In `Sequential`
  mode fixed-amount promotions commute exactly while unclamped; two percentage promotions
  commute up to rounding. General commutativity is not claimed. Determinism comes from the
  total order, not from commutativity.
- Invariant 6 (idempotence): applying `apply` to a cart that already carries the
  resulting discounts, with the same promotion set, yields no further discount, because
  the applied promotion set is recorded on the result and re-application is a no-op. The
  precise formulation lives in `docs/invariants.md`.

### Exclusive promotions

If the sorted eligible list contains at least one promotion with `exclusive = true`, the
first exclusive promotion in the total order is applied alone and every other promotion is
ignored. Otherwise all eligible promotions apply in order. The stacking policy is
irrelevant when exactly one promotion applies.

Invariant 4 follows: an exclusive promotion never appears in a result together with
another promotion.

## Consequences

- `priority` and a totally ordered `promotion_id` are mandatory fields of `Promotion`.
- `PricingPolicy` gains a second field; the DRT generators draw the stacking policy at
  random so both modes are covered by the equivalence argument.
- The result carries an ordered list of `AppliedPromotion` records (id, discount,
  per-line allocation) so that the computation can be audited step by step in either mode.
- Changing a promotion's priority, or the stacking policy, changes prices for carts
  eligible for several promotions. This is intended and visible; the policy is versioned
  with the promotion set, not hard-coded.
