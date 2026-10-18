# Spec Delta

## MODIFIED Requirements

### Requirement: Rates in the catalogue
The catalogue SHALL store exchange rates and expose `GET /v1/exchange-rates`,
`PUT|DELETE /v1/exchange-rates/{from}/{to}`; snapshots include every stored rate. Each
stored pair SHALL carry an origin, `manual` for a pair written by `PUT` and `source` for a
pair written by the exchange-rate source (spec `exchange-rate-source`), and the time it was
written. `PUT` sets the origin to `manual`, so the source no longer overwrites that pair;
`DELETE` removes the pair and the next fetch may store it again. The `ExchangeRate` body
and `PricingInput.exchange_rates` are unchanged; the origin is reported by
`GET /v1/exchange-rate-source`.

#### Scenario: Rate set over HTTP
- **WHEN** `PUT /v1/exchange-rates/USD/EUR {numerator: 92, denominator: 100}` is called
- **THEN** a subsequent evaluation of a EUR cart converts USD promotion amounts with it

#### Scenario: Manual overrides the source
- **WHEN** the source stored `USD → EUR` and an operator then `PUT`s `USD → EUR 92/100`
- **THEN** evaluations use `92/100` and later fetches leave it unchanged until it is
  `DELETE`d
