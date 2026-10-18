# Design

1. **A layer over the result, never inside it.** `PricingResult` is what the Lean model
   produces and what the DRT compares; adding fields there would make the Rust and Lean
   outputs diverge or force a model change for a presentation concern.
2. **Opt-in flag, same routes.** A separate `/explain` endpoint would duplicate the
   request handling; a query flag keeps one code path and lets the storefront ask only when
   it renders the checkout.
3. **Codes are the contract, messages are for operators.** Storefronts localise on
   `code`; the English `message` is never meant for customers.
4. **Coupon diagnostics come from the service**, which knows what the core cannot: whether
   a presented code exists at all, is paused, or belongs to an archived promotion. The
   promotion-level codes are a renaming of `SkipReason` plus the rule type at `rule_index`.
