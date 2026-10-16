# 6. Pricing is a total function over validated input; errors are values

Date: 2026-10-12

## Status

Accepted

## Context

The Lean proofs quantify over inputs. If `apply` can panic, throw or loop, the theorems
say nothing about those inputs, and the DRT cannot compare behaviours. Real input can be
malformed: mixed currencies, a fixed discount in the wrong currency, negative or oversized
amounts, duplicate promotion ids.

Options considered:

1. Reject at the adapter boundary and make the core assume valid input. Rejected: the
   proofs would carry preconditions that nothing in the core enforces, and the DRT would
   need its own validator, a third implementation.
2. Dependent types in Lean (`Fin`, subtypes) to make invalid input unrepresentable.
   Elegant, but Rust cannot mirror it, and JSON decoding would still need to fail
   somewhere. Kept as an option for internal functions after validation.
3. Skip invalid promotions silently. Rejected: hides configuration bugs behind a smaller
   discount.

## Decision

1. `apply : PricingInput → Except PricingError PricingResult` in Lean,
   `fn apply(input: &PricingInput) -> Result<PricingResult, PricingError>` in Rust.
   Both are total: every input yields exactly one of the two.
2. Validation is a separate, named step inside `apply` that runs before any arithmetic
   and reports the first error found in a fixed check order, so both implementations
   report the same error for the same input. It is followed by `normalize` (ADR 3), which
   converts foreign-currency amounts; the pipeline and the theorems run on the normalized
   input, and `validate = ok` guarantees `Valid (normalize input)`.
3. `PricingError` variants in the first iteration. Locations are reported by list
   index or by a `field` path, never by id, so that errors about duplicate ids are
   unambiguous:
   - `InvalidCurrency { field }`: a currency is not three ASCII uppercase letters.
   - `MixedCurrencies { line_index }`: a line's `unit_price` currency differs from
     `cart.currency`.
   - `CurrencyMismatch { field }`: a fixed amount, cap, budget or order-cap amount is in a
     currency other than the cart's. Loud, not skipped: a promotion the repository
     returned for this cart in another currency is a configuration bug.
   - `AmountOutOfRange { field }`: any amount `< 0` or `> MAX_MINOR_UNITS`, or the order
     subtotal `> MAX_MINOR_UNITS` (ADR 3).
   - `InvalidBasisPoints { field }`: outside `0 ..= 10_000`.
   - `InvalidQuantity { line_index }`: outside `1 ..= MAX_QUANTITY`.
   - `DuplicateLineId { line_id }`, `DuplicateGroupId { group_id }`,
     `DuplicatePromotionId { promotion_id }`, `DuplicateCouponCode { code }` (in the cart's
     codes or in the coupon snapshots), `DuplicateExchangeRate { from, to }`.
   - `UnknownGroup { promotion_index }`: the promotion's `group_id` is not in `groups` (ADR 4).
   - `InvalidCustomerContext { field }`: negative `completed_orders` or use count, or a
     promotion listed twice in `customer.uses`.
   - `InvalidExchangeRate { field }`: numerator or denominator outside `1 ..= MAX_RATE_PART`.
   - `InvalidRule { field }`: negative `max` in `MaxCompletedOrders` / `MaxUsesPerCustomer` /
     `MaxUsesTotal`.
   - `InvalidCoupon { field }`: negative `uses` or `max_uses` on a coupon snapshot.
   - `InvalidPromotion { field }`: negative `uses_total` on a promotion snapshot.
   - `Overflow`: reserved for checked arithmetic in Rust; unreachable for valid input
     (ADR 3). Lean never produces it.
   An empty cart is not an error: it prices to zero with no discounts. The exact check
   order and the `field` path grammar are normative and live in
   `docs/ubiquitous-language.md` under `PricingError`.
4. The invariant theorems are stated on the `ok` branch:
   `apply input = .ok result → <invariant>`. Validation lemmas state that the `ok` branch
   implies the bounds the arithmetic lemmas need.
5. DRT compares the full JSON of `Except`/`Result`, including the error variant and its
   payload. Generators produce invalid inputs at a fixed, small rate so the validation
   order is tested too.

## Consequences

- Validation order is part of the specification and is documented in
  `docs/ubiquitous-language.md` under `PricingError`.
- Every new field needs its validation rule in both implementations; the DRT catches the
  one that is forgotten.
