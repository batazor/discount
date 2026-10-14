# Spec Delta

## Purpose
Campaign amounts in another currency than the cart's, converted deterministically with
integer arithmetic before pricing.

## ADDED Requirements

### Requirement: Exchange rates as input
`PricingInput.exchange_rates` SHALL be a list of `ExchangeRate { from, to, numerator,
denominator }` meaning one minor unit of `from` is `numerator / denominator` minor units of
`to`. `validate` SHALL reject `numerator ≤ 0`, `denominator ≤ 0`, either above `10^12`
(`InvalidExchangeRate { field }`), malformed currencies (`InvalidCurrency`), and a
`(from, to)` pair listed twice (`DuplicateExchangeRate { from, to }`).

#### Scenario: Duplicate pair
- **WHEN** two rates are given for `USD → EUR`
- **THEN** `validate` reports `DuplicateExchangeRate { from: "USD", to: "EUR" }`

### Requirement: Normalization
After validation and before pricing, every `Money` of a promotion (`action.amount`, `cap`,
`remaining_budget`, `OrderTotalRange.min|max`) and `policy.order_cap.amount` whose currency
differs from the cart's SHALL be replaced by
`roundDiv(Down, amount * numerator, denominator)` in the cart currency using the rate
`from = that currency, to = cart currency`. The pipeline, the result and every invariant
are stated on the normalized input. Rounding of the conversion is always `Down`, independent
of the policy (ADR 3).

#### Scenario: Fixed amount in another currency
- **WHEN** the cart is in EUR, a promotion grants `OrderFixedAmount 1000 USD` and the rate
  `USD → EUR` is `92 / 100`
- **THEN** the applied discount is `920` EUR

#### Scenario: Rounding down
- **WHEN** the amount is `999 USD` and the rate is `92 / 100`
- **THEN** the normalized amount is `919` (919.08 rounded down)

### Requirement: Missing rate and bounds
`validate` SHALL reject a promotion amount in a currency with no rate to the cart currency
(`CurrencyMismatch { field }`, as before), and a converted amount above `MAX_MINOR_UNITS`
(`AmountOutOfRange { field }`). The original amount is checked against the bounds first.

#### Scenario: No rate
- **WHEN** the cart is in EUR and a cap is in GBP with no `GBP → EUR` rate
- **THEN** `validate` reports `CurrencyMismatch { field: "promotions[0].cap" }`

### Requirement: Rates in the catalogue
The catalogue SHALL store exchange rates and expose `GET /v1/exchange-rates`,
`PUT|DELETE /v1/exchange-rates/{from}/{to}`; snapshots include every stored rate.

#### Scenario: Rate set over HTTP
- **WHEN** `PUT /v1/exchange-rates/USD/EUR {numerator: 92, denominator: 100}` is called
- **THEN** a subsequent evaluation of a EUR cart converts USD promotion amounts with it
