# exchange-rate-source Specification

## Purpose
Exchange rates fetched from a provider on a schedule the operator sets, converted into
the catalogue's integer rates, with their origin and freshness visible.

## Requirements

### Requirement: Source setting
The catalogue SHALL store at most one exchange-rate source
`ExchangeRateSource { url, interval_seconds, max_age_seconds, timeout_seconds }` exposed as
`GET /v1/exchange-rate-source` (an `ExchangeRateSourceStatus`, with `source: null` when
none is set), `PUT /v1/exchange-rate-source` (body `ExchangeRateSource`; `200` with the
status) and `DELETE /v1/exchange-rate-source` (`204`; stored rates are kept, their origin
becomes `manual`). `url` SHALL be an absolute `http` or `https` URL, `interval_seconds` in
`60 ..= 604800`, `max_age_seconds` in `interval_seconds ..= 2592000` and `timeout_seconds`
in `1 ..= 120`; anything else is `400 {"error": "DecodeError"}`.

#### Scenario: Set and read
- **WHEN** `PUT /v1/exchange-rate-source {url: "https://rates.example/latest?from=EUR",
  interval_seconds: 3600, max_age_seconds: 86400, timeout_seconds: 10}` is called
- **THEN** `GET /v1/exchange-rate-source` returns that `source`, `last_attempt_at` and
  `last_success_at` as `null`, `stale: false` and `pairs: []`

#### Scenario: Bad interval
- **WHEN** `interval_seconds` is `30`, or `max_age_seconds` is below `interval_seconds`
- **THEN** the response is `400` and the stored source is unchanged

#### Scenario: Turn off
- **WHEN** `DELETE /v1/exchange-rate-source` is called after rates were fetched
- **THEN** `GET /v1/exchange-rates` still lists them, `GET /v1/exchange-rate-source` has
  `source: null` and `pairs: []`, and no further fetch happens

### Requirement: Provider format
A fetch SHALL `GET` the URL with `Accept: application/json`, the source's `timeout_seconds`
and at most three redirects, and SHALL accept a JSON object with `base` (a currency) and `rates` (an
object of currency → positive number), the shape served by frankfurter.app,
exchangerate.host, openexchangerates.org and fixer.io. For every entry `X: r` it SHALL
produce the pair `base → X` as `round(r × 10^6) / 10^6` and the pair `X → base` as
`10^6 / round(r × 10^6)`, both reduced by their greatest common divisor. A response that is
not such an object, has no entries, or has a rate whose fraction parts fall outside
`1 ..= 10^12` SHALL fail the whole fetch; no pair of it is stored.

#### Scenario: Pairs from one entry
- **WHEN** the provider answers `{"base": "EUR", "rates": {"USD": 1.0873}}`
- **THEN** `GET /v1/exchange-rates` contains `EUR → USD 10873/10000` and
  `USD → EUR 10000/10873`

#### Scenario: Malformed answer
- **WHEN** the provider answers `{"rates": "n/a"}` or a non-2xx status
- **THEN** no rate changes, `last_error` holds the reason and `last_success_at` is unchanged

### Requirement: Schedule and manual refresh
While a source is set, the server SHALL fetch it when `interval_seconds` have passed since
the last attempt (immediately after the setting is stored or the server starts), without a
restart when the setting changes. `POST /v1/exchange-rate-source/refresh` SHALL fetch at
once and answer `200` with the status on success, `502 {"error": "SourceUnavailable",
"message"}` on failure (the status still records the attempt) and `404` when no source is
set. Each fetch upserts the produced pairs whose stored origin is not `manual`, records
them with origin `source` and `fetched_at = now`, and leaves every other stored pair alone.

#### Scenario: Refresh now
- **WHEN** a source is set and `POST /v1/exchange-rate-source/refresh` is called
- **THEN** the response is `200`, `last_success_at` is the fetch time and `pairs` lists
  every pair the provider produced with that `fetched_at`

#### Scenario: Manual pair survives a fetch
- **WHEN** `PUT /v1/exchange-rates/USD/EUR {92, 100}` was called and a fetch produces
  `USD → EUR`
- **THEN** `GET /v1/exchange-rates/USD/EUR` is still `92/100` and the pair is absent from
  `pairs`

#### Scenario: Provider down
- **WHEN** the scheduled fetch fails
- **THEN** every stored rate, fetched or manual, stays as it was, evaluations keep using
  them, and `GET /v1/exchange-rate-source` shows `last_error` with the attempt time

### Requirement: Freshness
The status SHALL report `stale: true` when a source is set and no fetch has succeeded
within `max_age_seconds` of now (counted from the setting's `updated_at` when nothing has
ever succeeded), `next_due_at` (last attempt plus interval, or now when never attempted),
and `pairs` as `[{from, to, fetched_at}]` for the pairs with origin `source`. Stale rates
are still used for pricing; staleness is a signal to the operator, never a pricing input.

#### Scenario: Stale after the maximum age
- **WHEN** the last success was 25 hours ago and `max_age_seconds` is `86400`
- **THEN** `stale` is `true` and an evaluation still converts with the stored rates

#### Scenario: Fresh again
- **WHEN** a fetch then succeeds
- **THEN** `stale` is `false`
