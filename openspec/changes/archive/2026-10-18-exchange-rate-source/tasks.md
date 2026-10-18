# Tasks

## 1. Contract (`api/openapi.yaml`, `docs/`)

- [x] 1.1 Add `ExchangeRateSource {url, interval_seconds, max_age_seconds, timeout_seconds}`, `ExchangeRateSourceStatus {source, last_attempt_at, last_success_at, last_error, last_count, next_due_at, stale, pairs: [{from, to, fetched_at}]}`, the paths `GET|PUT|DELETE /v1/exchange-rate-source` and `POST /v1/exchange-rate-source/refresh` with `400`, `404`, `502 SourceUnavailable`; verify `npx @redocly/cli lint api/openapi.yaml` and `cargo build -p discount-client`
- [x] 1.2 Add the metric names and the span to "Telemetry vocabulary" in `docs/ubiquitous-language.md` and a paragraph to `README.md` under multi-currency / operator console; verify the names match the code in 4.1

## 2. Ports and storage (`src/application/`, `src/infrastructure/`)

- [x] 2.1 In `ports.rs` add `ExchangeRateSource`, `FetchRecord`, `RateOrigin`, the `RateFetcher` port and `PromotionCatalog::{get_source, put_source, delete_source, record_fetch, list_rate_origins, put_rates_from_source}`; `put_rate` stamps `manual`; verify `cargo build --workspace`
- [x] 2.2 Migration `src/infrastructure/migrations/0007_exchange_rate_source.sql`: table `exchange_rate_source`, columns `exchange_rates.origin`, `exchange_rates.fetched_at`; implement the new methods in `persistence/postgres.rs` and `persistence/memory.rs`; verify the Postgres scenario `source_round_trip_and_manual_wins` in `tests/ports.rs` on both stores
- [x] 2.3 Use case `RefreshExchangeRates` in `src/application/src/exchange_rates.rs`: parse `{base, rates}`, build both directions at scale `10^6` reduced by gcd, reject out-of-range parts, skip `manual` pairs, call `record_fetch` on both outcomes; verify unit tests for `1.0873`, `JPY 162.4`, an entry below `10^-6`, a non-object body

## 3. Fetcher, scheduler and HTTP (`src/infrastructure/`)

- [x] 3.1 `fetch/reqwest.rs`: `RateFetcher` over `reqwest` (rustls, the source's timeout, 3 redirects, `Accept: application/json`, `http`/`https` only); add the dependency to `src/infrastructure/Cargo.toml`; verify an axum test that serves a canned body from a `tokio::net::TcpListener` and one that answers `503`
- [x] 3.2 Handlers for the four operations in `http.rs` with validation of `url`, `interval_seconds`, `max_age_seconds` and `timeout_seconds` (`400`), `404` without a source, `502` on fetch failure; the status read model computes `stale` and `next_due_at` from the clock; verify axum tests for the scenarios of spec `exchange-rate-source`
- [x] 3.3 Scheduler task in `bin/discount-server.rs`: tick every 15 s, run the use case when due; verify with the in-memory server pointed at a local stub that a changed interval is honoured without restart

## 4. Telemetry and alerting (`src/infrastructure/`, `deploy/grafana/`)

- [x] 4.1 Instruments `discount.exchange_rates.fetches`, `.age`, `.stale` and the span `discount.exchange_rates.fetch` (URL without query, `discount.pairs`, `discount.error_type`) in `telemetry.rs`; verify `tests/telemetry.rs` sees the counter and the span after a failed fetch
- [x] 4.2 Alert rule `discount-exchange-rates-stale` (`warning`, `stale == 1` for 5 min) in `deploy/grafana/alerting/discount.yaml` and a stat panel on the overview dashboard; raise the expected rule count in `deploy/smoke.sh`; verify `deploy/smoke.sh` passes on the compose stack

## 5. Console (`src/ui/`)

- [x] 5.1 Source screen `pages/settings/exchange-rates/source.astro`: URL, interval, maximum age and timeout in human units (`model/units.ts` gains `parseDuration`/`formatDuration`), status, Save / Try again / Turn off actions in `actions/index.ts` with `CONSEQUENCE` and `TERM` help texts; verify `npm run check && npm test` and the two admin-ui scenarios by hand against the in-memory server with a stub provider
- [x] 5.2 On `pages/settings/exchange-rates/index.astro`: link to the source screen, status line with Try again, origin badge and `fetched_at` on rate rows; verify `npm run build` and the origin scenario by hand

## 6. Demo and docs (`deploy/demo/`, `docs/`)

- [x] 6.1 Let `deploy/demo/seed.py` set the source when `RATE_SOURCE_URL` is given (default off, so the demo needs no internet); mention it in `README.md`; verify a seed run with and without the variable
