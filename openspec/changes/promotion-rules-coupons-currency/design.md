# Design

## Context

See proposal.md. The pricing pipeline and all proofs are stated on a `PricingInput` whose
amounts are already in the cart currency and whose eligibility reads only `now`. The Lean
model is the source of truth; the Rust core mirrors it; the DRT compares them.

## Goals / Non-Goals

**Goals:** every new semantic goes through Lean and the DRT; existing theorems stay proved;
the JSON contract stays the single wire format; the service resolves coupons and rates
from its catalogue so callers send only codes.

**Non-Goals:** line-level actions, best-for-customer coupon selection, rate freshness
policies, multi-tenant catalogues.

## Decisions

1. **Normalization is a separate total stage** (`normalize : PricingInput → PricingInput`)
   between `validate` and the existing pipeline. Conversion happens once, with `Down`
   rounding, before any pricing. The invariant theorems are restated on
   `normalize input`; their proofs reuse the existing fold invariant unchanged because
   `Valid (normalize input)` follows from `validate input = ok`. Alternative: convert inside
   `applyOne`; rejected because it would thread rates through every proof.

2. **Rates are integer fractions with bounds `1 ..= 10^12`.** `amount * numerator` fits
   `i128` (`10^14 * 10^12 = 10^26`); the converted amount is checked against
   `MAX_MINOR_UNITS` by `validate`, so after normalization every amount fits `i64` and the
   `no_overflow` argument of ADR 3 is unchanged. Rust converts in `i128`.

3. **Only direct pairs.** A missing `from → cart` pair is `CurrencyMismatch`. The
   application layer may derive cross rates when it loads them; the core does not pivot.

4. **Coupons as snapshots in the input, resolved by the app layer** (decision taken with
   the product owner): the core sees `coupons` filtered to the presented codes; eligibility
   is a pure function of the input, so `RequiresCoupon` is proved like any rule.

5. **Usage counters are input** (`customer.uses`). The app layer sums reserved and
   confirmed redemptions of the customer per applied promotion and adds caller-supplied
   counts. `MaxUsesPerCustomer` compares `< max`, so `max = 1` means single use.

6. **`AttributeDateWindow` parses the attribute as a decimal integer.** Attributes are
   strings to keep `CartSnapshot` simple; a non-integer value fails the rule rather than
   validation, because attribute content is order data, not configuration.

7. **Validation order** (normative, extends ADR 6): cart currency, lines, duplicate line
   ids, duplicate coupon codes in the cart, subtotal, customer context, exchange rates
   (per rate: currencies, numerator, denominator; then duplicates), coupons (per coupon:
   currencies-free; then duplicate codes), policy order cap (with conversion), groups,
   promotions (per promotion: action, cap, budget, rules in order, group), duplicate
   promotion ids. Every money check is: valid currency → rate exists (or same currency)
   → original in range → converted in range.

8. **Postgres:** `coupons(code PK, body jsonb)`, `exchange_rates(from_currency, to_currency
   PK, numerator, denominator)`, `redemptions.customer_id TEXT NULL` with an index;
   `uses_for_customer` is one aggregate query over `result -> 'applied'`.

## Risks / Trade-offs

- [Proof churn] → the normalize stage isolates it to one bridging lemma
  (`validate_ok_normalized`) plus statement edits.
- [Attribute typing is stringly] → acceptable for parity; a typed attribute catalogue can
  come with the admin UI.
- [DRT input space grows] → generators keep each new feature rare enough that most cases
  still exercise pricing; `DRT_CASES` in CI stays 5000.

## Migration Plan

Fixtures and the Postgres migration `0002_coupons_rates.sql`; no deployed data exists.
