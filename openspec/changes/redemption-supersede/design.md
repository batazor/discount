# Design

## Decisions

1. **Supersede, do not error.** Stripe-style idempotency mismatch errors suit payments,
   where the second request is a bug. At checkout the second request is the normal flow;
   the storefront wants the latest price reserved with no window in between.
2. **Equality of input means equality of `cart` and `customer`** as sent; `now` and the
   catalogue may have moved. A repeat with the same body never re-prices, so a storefront
   retry is safe.
3. **The redemption row stays the single current version**; history is a separate table.
   Counters, budgets and the customer's coupon list keep reading only current rows, so none
   of their requirements change.
4. **A failed re-price keeps the old reservation**: pricing errors on the new input leave
   budgets and the stored row untouched (the transaction rolls back).

## Risks / Trade-offs

- [Budget race between release and re-take] → both happen under the same `FOR UPDATE` on
  the budget and promotion rows, in one transaction.
