# Design

## Decisions

1. **Attributes, not fields.** The customer gets the same `Attribute` list the cart has and
   two generic rules, instead of one typed field per campaign condition. "First booking from
   the app" is `TagsMatch` on the cart plus `CustomerAttributeRange` on a counter the caller
   supplies; partner conditions are attributes too. A new condition needs no change to UL,
   Lean, Rust or DRT.
2. **The allowlist is service state; the core sees only a list of ids.** A campaign list can
   hold tens of thousands of customer ids; it must not enter `PricingInput` (the replay
   record). Like `uses`, the service derives `customer.allowlisted` for the one customer
   being priced, and `RequiresAllowlist` is a rule that reads the context. The replay record
   stays small and complete.
3. **`CustomerAttributeRange` is a closed range on an integer**, like `OrderTotalRange`, and
   unlike `AttributeDateWindow`, which is half-open because it models time.
4. **Validation**: `allowlisted` is checked for duplicates like `uses`; attributes are not
   validated (duplicates allowed, as for the cart). The new rules have no parameters to
   range-check.
5. **Allowlist API** follows the budget and metadata sub-resources; the per-customer
   sub-sub-resource avoids re-sending a large set to add one id.

## Risks / Trade-offs

- [`allowlisted_for(customer_id)` on every evaluation] → one indexed query on
  `(customer_id)`; the set of promotions is small.
- [Caller-supplied `allowlisted`] → trusted like the rest of `CustomerContext` (ADR 7: the
  caller is the system of record for the customer); the merge is a union.
