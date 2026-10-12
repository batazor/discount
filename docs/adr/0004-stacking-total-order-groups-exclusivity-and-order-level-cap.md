# 4. Stacking: total order, groups, exclusivity and order-level cap

Date: 2026-10-12

## Status

Accepted

Is depended on by [5. Allocation of order-level discounts to lines by largest remainder](0005-allocation-of-order-level-discounts-to-lines-by-largest-remainder.md)

## Context

Several promotions can be eligible for one cart. The result must be deterministic and
explainable, and the invariants (final price in `[0, original]`, caps respected,
idempotence) must be provable. Four questions have to be answered explicitly: what a second
percentage discount is computed on, how ties are broken, how exclusivity and grouping work,
and whether the order as a whole has a ceiling.

Industry practice is split, so these are genuine choices, not conventions to copy:

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
- Talon.One: campaigns sit in evaluation groups with a mode (stackable, or only the
  highest-priority campaign in the group applies). Voucherify: stacking rules with
  ordering and exclusivity between redeemables.

All of them use an explicit priority; most have a stop-after or exclusive mechanism; the
base for percentages differs; the enterprise engines add grouping, because "one shipping
promotion at most, combinable with one product promotion" cannot be expressed with a single
global flag. WooCommerce shows that offering both bases as a policy is an accepted product
decision.

Options considered:

1. Sequential only. Matches commercetools and "20 % off, then an extra 20 % off"; each
   step has a well-defined base; proofs are stepwise.
2. Additive only. Matches Shopify's order-discount rule; "20 % + 20 % = 40 %" is what
   some merchants expect; the clamp at zero needs an attribution rule anyway.
3. Both, selected by an engine-wide policy. Chosen. The two modes differ only in the base
   used for percentages; clamping, allocation and the total order are shared, so the
   proof surface grows by one case split, not by a second engine.
4. Flat list with a global exclusive flag only. Rejected: cannot express "exclusive
   within group, stackable across groups".
5. Mode on the group, exclusive flag removed. Rejected: Sylius-style "applies alone" is
   still needed and would have to be emulated with a one-promotion group.
6. Mode on the group plus a global exclusive flag on the promotion. Chosen.
7. Best-for-customer exclusive selection (evaluate every exclusive promotion, pick the
   largest discount). Rejected: requires computing all alternatives, makes the winner
   depend on cart contents in a way that is hard to explain, and still needs a tie-break.
8. Tie-break on anything other than id (creation time, name). Rejected: not an input of
   the pricing function.
9. Order cap as a promotion-level rule. Rejected: it is a property of the order or the
   merchant, not of any one promotion, and it must bind the *sum*.

## Decision

### Stacking groups and total order

- `StackingGroup { group_id, priority, mode }` with `StackingMode = Stackable | FirstMatch`.
  Groups are an input: `PricingInput.groups : List StackingGroup`.
- Every `Promotion` names its group: `group_id : GroupId`. A promotion whose group is not
  in `groups` fails validation (`UnknownGroup`). Duplicate group ids fail validation.
- Eligible promotions are processed in the **total order**
  `(group.priority DESC, group_id ASC, promotion.priority DESC, promotion_id ASC)`.
  Ids are unique after validation and compared bytewise, so the order is total and the
  result cannot depend on the order in which the repository returned groups or promotions.
- `FirstMatch`: within the group, only the first *eligible* promotion in the total order
  is applied; the others are skipped with reason `FirstMatchTaken` (ADR 4). Eligibility,
  not discount size, decides: a first-match winner with a zero discount still wins. This
  keeps selection independent of arithmetic and provable without evaluating alternatives.
- `Stackable`: every eligible promotion in the group is applied in order.

### Global exclusivity

`Promotion.exclusive : Bool`. If any eligible promotion is exclusive, the first exclusive
one in the total order is applied alone and every other eligible promotion is skipped with
reason `ExclusiveApplied`. Evaluated **before** group modes, which are then irrelevant.

Pipeline: `validate → eligible → sortPromotions → selectExclusive → selectFirstMatch → fold applyOne`.

### Stacking policy

`StackingPolicy` is a domain type and part of `PricingPolicy` (ADR 8), next to
`RoundingPolicy`. Variants:

- `Sequential` (default): each promotion's percentage is computed on the running
  order subtotal (original subtotal minus all discounts applied so far).
- `Additive`: each promotion's percentage is computed on the original order subtotal.

Everything else is identical in both modes. With `base` chosen by the policy and the
effective rounding from ADR 3:

- `OrderPercentage bps`: `discount = roundDiv(rounding, base * bps, BPS_SCALE)`.
- `OrderFixedAmount m`: `discount = m.amount`.

### Order-level cap

`PricingPolicy.order_cap : OrderCap = { amount : Option Money, bps : Option BasisPoints }`.
Let `S` be the original order subtotal. The order cap is
`min(amount, roundDiv(PricingPolicy.rounding, S * bps, BPS_SCALE))` over the present
parts; both absent means unconstrained. The percentage part always uses the engine-wide
rounding, never a promotion's own policy, because it is not a promotion's computation.

### Clamp

The raw discount is clamped to every present bound, in this fixed order:

`discount := min(raw, cap, remaining_budget, remaining_order_cap, running)`

where `remaining_order_cap` is the order cap minus discounts applied so far and `running`
is the current remaining order subtotal, in both stacking modes. The result is allocated to
lines (ADR 5) and `running` decreases by `discount`. The bound that produced the clamp is
recorded as `clamped_by` (ADR 4).

In `Additive` mode the clamp to `running` means the *last* promotions in the total order
absorb the shortfall when the sum of discounts would exceed the subtotal; attribution stays
per promotion and deterministic.

### Consequences for the invariants

- Invariant 1 holds step by step in both modes: `0 ≤ discount ≤ running`, so `running`
  never goes below zero and never increases.
- Invariant 2 holds by the clamp, by construction, including `total_discount ≤ order cap`.
- Invariant 3 (order-independence) is claimed and proved only where true. Permutation
  invariance of the `promotions` and `groups` lists is proved. Commutativity of adjacent
  promotions: in `Additive` mode all unclamped percentage discounts commute exactly (same
  base); in `Sequential` mode fixed-amount promotions commute exactly while unclamped and
  two percentage promotions commute up to rounding. General commutativity is not claimed.
  Determinism comes from the total order, not from commutativity.
- Invariant 4 is two statements: an exclusive applied promotion is the only applied
  promotion; a `FirstMatch` group has at most one applied promotion.
- Invariant 6 (idempotence): see `docs/invariants.md`; stated, proved in iteration 2.

## Consequences

- `priority`, `group_id` and a totally ordered `promotion_id` are mandatory fields of
  `Promotion`; `PricingInput` carries `groups`; `PricingPolicy` carries `stacking` and
  `order_cap`.
- The default configuration is one `Stackable` group, `Sequential`, no order cap. A
  single-group input behaves like a flat priority list, so simple deployments lose nothing.
- The DRT generators draw the stacking policy, a small number of groups with random modes,
  group membership, and the order cap at random, so every mode is covered by the
  equivalence argument.
- The result carries an ordered list of `AppliedPromotion` records (id, discount, clamp
  reason, per-line allocation) so that the computation can be audited step by step.
- Changing a promotion's priority or group, the stacking policy, or the order cap changes
  prices for carts eligible for several promotions. This is intended and visible; policies
  are versioned with the promotion set, not hard-coded.
- Application-layer naming follows the engines above: `Evaluate` (pure, dry-run; calls
  `apply`) and `Redeem` (commits budgets and usage) are the two use cases in `app/`.
  Only `Evaluate` is in the verified scope.
