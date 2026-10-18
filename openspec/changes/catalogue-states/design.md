# Design

## Decisions

1. **State lives in the catalogue, not in `Promotion`.** Pausing is an operator action on
   stored state, like a budget; the pricing engine receives only what may apply. A paused
   promotion is absent from the snapshot, so `skipped` does not mention it; the explanation
   endpoint (change `eligibility-explanations`) names paused coupons for the storefront.
2. **Two states, not a lifecycle.** `draft`, `scheduled`, `expired` are derivable from
   `DateWindow` rules and `starts_at`/`ends_at`; adding them as stored states would create
   two sources of truth. `active | paused` is the only state that is purely an operator's
   decision.
3. **State is read-only in the entity bodies** and set through its own resource, like the
   budget and the metadata, so a stale `PUT` of a promotion cannot silently resume it.
4. **Paused propagates to coupons in listings**, not in storage: a coupon of a paused
   promotion keeps `state = active` and shows `paused` to the customer; resuming the
   promotion needs no fan-out.

## Risks / Trade-offs

- [Operators expecting `skipped` to show a paused promotion] → the explanation endpoint
  reports `promotion_paused` for a presented coupon; for promotions without a coupon there
  is nothing to explain, which matches archiving today.
