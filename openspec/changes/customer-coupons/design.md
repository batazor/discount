# Design

## Context

Everything here is a read model over data the service already stores (coupons, promotions,
redemptions) plus one new record, the saved code. The pricing engine is not involved: a
status is a statement about the coupon itself, not about a cart.

## Decisions

1. **Status is cart-free.** `expired`, `scheduled`, `active` follow from the coupon's window,
   its counters and its promotion's existence; whether the promotion would apply to a given
   cart is `Evaluate`'s answer. The storefront calls `Evaluate` at checkout anyway.
2. **`applied` is per customer and wins.** A coupon the customer used is `applied` even if it
   has uses left (an n-use code), because the question is "did I use it", and `order_id`
   points at the earliest redemption so the screen can link to the order.
3. **"Used" reuses the counter definition** (spec `redemption`): presented the code and
   applied its promotion. A code presented on a cart where another rule failed was not used.
4. **Saved codes are application state**, table `saved_coupons (customer_id, code)` with a
   composite primary key; saving is idempotent, forgetting a code never touches the coupon.
   Saving a code bound to another customer is refused (`409`), saving one's own bound code is
   a no-op that still returns the listing entry.
5. **Promotion details are the `action` only.** Description, title and other metadata are
   the next change (`promotion-metadata`); the shape `CouponPromotion { promotion_id, action }`
   grows there without breaking this contract.
6. **One query per source in Postgres**, merged in Rust by code: bound coupons by a GIN or
   expression index on `body->>'customer_id'`, used codes through the redemptions table
   (existing `customer_id` index), saved codes by primary key.

## Risks / Trade-offs

- [A customer who used many public codes gets a long list] → listing is bounded by their own
  redemptions; no pagination in this change.
- [Status computed at read time with the server clock] → `now` comes from `Clock`, as for
  grants; tests use `FixedClock`.
