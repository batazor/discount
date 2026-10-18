# Design

## Decisions

1. **Open trigger names.** Business events are not ours to enumerate: `cancel`, `review`,
   `birthday`, `landing.football` are all just names; the pattern keeps them URL- and
   metric-safe.
2. **Conditions reuse the attribute vocabulary** of the cart and the customer (`key`,
   `values`), not a new mini-language; a domain or channel is an attribute of the event.
3. **Cooldown returns the recent coupon** rather than an error: the caller wanted a code for
   the customer and there is one; the response is indistinguishable from an idempotent
   repeat, which is what a storefront can handle without new branches.
4. **Liveness uses the catalogue state** from `catalogue-states`; a paused promotion is a
   `409`, not a silent fallback to a lower-priority rule, so operators notice.

## Risks / Trade-offs

- [Cooldown across rules of one trigger] → it is per trigger, not per rule, because the
  customer-facing promise is "one cancellation code per day", whichever rule produced it.
