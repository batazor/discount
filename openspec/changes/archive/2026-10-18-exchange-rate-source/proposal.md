# Proposal

## Why

Exchange rates are the only pricing input an operator has to type by hand and keep
current: `PUT /v1/exchange-rates/{from}/{to}` or the Settings → Exchange rates form, with
no notion of where a rate came from or how old it is. A campaign priced in USD on a EUR
cart silently uses whatever was typed last. Every comparable service (Talon.One,
Voucherify, commercetools) pulls rates from a provider on a schedule; here nothing can.

## What Changes

- A configurable **exchange-rate source**: the URL of a provider, how often to fetch it,
  how old the rates may get before they count as stale and how long a fetch may take,
  stored in the catalogue and set from the console (`GET|PUT|DELETE
  /v1/exchange-rate-source`, `POST /v1/exchange-rate-source/refresh`). The server fetches
  on that interval, converts the provider's decimal rates into the integer fractions the
  engine already takes, and upserts them as ordinary catalogue rates.
- **Provenance** on stored rates: each pair records whether it was set by an operator or by
  the source and when. A manually set pair is never overwritten by the source until it is
  deleted; the console shows the origin on every row.
- **Freshness** as a first-class signal: the source status reports the last attempt, last
  success, the error verbatim and whether the rates are stale (no success within the
  configured maximum age); a gauge and a provisioned alert rule carry the same.
- A new console screen Settings → Exchange rates → Source: URL, interval, maximum age and
  timeout in human units, the status, and the actions Save, Try again, Turn off. The
  exchange-rates screen links to it and shows the source status and a Try again button
  next to the rates.
- No change to the engine: `ExchangeRate`, `normalize` and the invariants are untouched.
  Pricing keeps using the last stored rates when the provider is down.

Outside the verified scope: everything here. The fetcher, scheduler, storage, HTTP
resource, telemetry, alert rule and console screen live in `src/application/`,
`src/infrastructure/`, `src/ui/` and `deploy/grafana/`. `src/domain/`,
`verification/` and the JSON contract of `PricingInput` do not change.

## Capabilities

### New Capabilities
- `exchange-rate-source`: the source setting, the fetch and conversion, the status and
  freshness of fetched rates.

### Modified Capabilities
- `multi-currency`: stored rates carry an origin; a manual rate is not overwritten by the
  source.
- `admin-ui`: the exchange-rates screen configures the source and shows origin and
  freshness.
- `telemetry`: fetch counter, age and staleness gauges.
- `grafana-dashboards`: one more provisioned alert rule, stale exchange rates.

## Impact

- `api/openapi.yaml`: new paths and schemas (`ExchangeRateSource`,
  `ExchangeRateSourceStatus`), `ExchangeRate` unchanged.
- `src/application/src/ports.rs`: `PromotionCatalog` gains source settings, fetch
  bookkeeping and per-pair origin; a new use case `RefreshExchangeRates`.
- `src/infrastructure/`: an HTTP fetcher (new dependency `reqwest` with rustls, already in
  the workspace for `src/client`), a scheduler task in `discount-server`, a migration adding
  the source row and the origin columns, handlers, metrics.
- `src/ui/`: Settings → Exchange rates.
- `deploy/grafana/alerting/discount.yaml`, `docs/ubiquitous-language.md` (telemetry
  vocabulary), `README.md`.
