# Spec Delta

## ADDED Requirements

### Requirement: Exchange-rate source metrics
While a source is set, the service SHALL record `discount.exchange_rates.fetches` (counter)
with `outcome` in `ok | error`, `discount.exchange_rates.age` (gauge, seconds since the last
successful fetch; absent until one succeeded) and `discount.exchange_rates.stale` (gauge,
`0 | 1`, the status's `stale`), and SHALL run each fetch in a span
`discount.exchange_rates.fetch` with attributes `discount.source.url` (without its query
string), `discount.pairs` and, on error, `discount.error_type`. Pricing spans and metrics
are unchanged.

#### Scenario: Failed fetch
- **WHEN** a scheduled fetch gets a `503` from the provider
- **THEN** `discount.exchange_rates.fetches{outcome="error"}` increased by 1 and the span
  has `discount.error_type = "SourceUnavailable"`
