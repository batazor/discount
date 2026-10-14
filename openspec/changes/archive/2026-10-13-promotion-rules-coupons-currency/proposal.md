# Proposal

## Why

The engine can price campaigns but cannot express what the promocode service it replaces
does every day: coupon codes, minimum order totals, tags and attributes of the order (hotel,
region, channel), check-in windows, "first booking only", per-customer usage limits, and
campaigns priced in a currency other than the cart's. Parity means these semantics go
through the verified core: ubiquitous language first, then the Lean model, then the Rust
mirror, then the differential tests.

## What Changes

- Seven new `Rule` variants: `OrderTotalRange`, `TagsMatch`, `AttributeIn`,
  `AttributeDateWindow`, `MaxCompletedOrders`, `MaxUsesPerCustomer`, `RequiresCoupon`.
- `CartSnapshot` gains `tags`, `attributes`, `coupon_codes`; `CustomerContext` gains
  `customer_id`, `completed_orders`, `uses`.
- `Coupon` snapshots and `ExchangeRate`s become inputs of pricing (`PricingInput.coupons`,
  `PricingInput.exchange_rates`).
- A `normalize` stage after validation converts every promotion amount and the order cap to
  the cart currency with an integer rate and `Down` rounding; the pipeline and its proofs
  run on the normalized input. Validation gains the checks for rates, coupons, customer
  context and rule parameters, in a fixed order.
- Lean: types, `validate`, `normalize`, `eligible` extended; every theorem restated on the
  normalized input and re-proved; `eligible_depends_on_input` now ranges over
  `(now, cart, customer, coupons)`.
- Rust core mirrors all of it; DRT generators and mutations cover the new fields and rules.
- Application and adapters: coupons and exchange rates in the catalogue (Postgres and
  in-memory, HTTP CRUD); `pricing_input` resolves `cart.coupon_codes` to coupon snapshots
  and fills `customer.uses` from stored redemptions; redemptions remember `customer_id`.

**BREAKING** (JSON contract): `CartSnapshot`, `CustomerContext` and `PricingInput` gain
required fields. Nothing has shipped, so no migration is provided beyond the fixture
updates.

Outside the verified scope: catalogue storage and HTTP for coupons and rates, the
derivation of `customer.uses` from redemptions.

## Capabilities

### New Capabilities
- `eligibility-rules`: the catalogue of `Rule` variants, their inputs and exact semantics.
- `coupons`: coupon snapshots, how codes in a cart make a promotion eligible, and coupon
  storage.
- `multi-currency`: exchange rates as input, the normalization of promotion amounts, and
  the bounds that keep it in `i64`.

### Modified Capabilities
- `evaluate-http-api`: the request's `cart` and `customer` carry the new fields; coupons
  and rates come from the catalogue.
- `promotion-catalog`: coupons and exchange rates are stored and managed over HTTP; the
  snapshot includes them.
- `redemption`: redemptions record `customer_id`, and `Redeem`/`Evaluate` fill
  `customer.uses` from reserved and confirmed redemptions.

## Impact

- `docs/ubiquitous-language.md`, `docs/invariants.md` (statements on the normalized input),
  ADR 3 (conversion rounding) and ADR 6 (new errors and check order) edited in place.
- `spec/`: `Types`, `Validate`, `Eligibility`, `Apply` (+ `normalize`), every theorem file.
- `core/`: the same modules; `drt/`: generators, mutations, fixtures.
- `app/`, `adapters/`: catalogue ports and Postgres migration `0002_coupons_rates.sql`,
  HTTP routes `/v1/coupons[/{code}]`, `/v1/exchange-rates[/{from}/{to}]`, redemption
  `customer_id`.
