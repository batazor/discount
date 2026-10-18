# Design

## Context

Rates are already a plain catalogue resource and an input of `PricingInput`; the engine
converts with `roundDiv(Down, amount × numerator, denominator)` and proves nothing about
where the fraction came from (multi-currency design, decisions 2 and 3). The server
binary already runs one periodic task (the budget gauge refresh). The console already has
a Settings → Exchange rates screen and human units. Nothing in the service makes outbound
HTTP calls today.

**Goals:** rates arrive without typing; the operator sets where and how often from the
console; every stored rate says where it came from and how old it is; pricing never waits
on, nor fails because of, the provider.

**Non-Goals:** several sources; provider-specific authentication beyond what fits in the
URL; cross rates through a pivot (the provider answers with a base, which already yields
both directions for every currency); historical rates; per-promotion rate snapshots.

## Decisions

1. **One setting, stored like the policy.** `exchange_rate_source` is a single row
   (`id = 1`, `body JSONB` with `{url, interval_seconds, max_age_seconds, timeout_seconds}`, `updated_at`, plus the
   bookkeeping columns `last_attempt_at`, `last_success_at`, `last_error`, `last_count`),
   behind `PromotionCatalog::get_source / put_source / delete_source / record_fetch`. The
   in-memory store mirrors it. Alternative: environment variables; rejected because the
   operator must change the interval without a deploy and the console cannot write env.

2. **The provider shape is `{base, rates}`.** It is what the free and the paid providers
   agree on, so no per-provider adapter is needed: the operator pastes the URL including
   the query (`?from=EUR`, `?base=USD&app_id=…`). Both directions are stored from one
   entry because the engine only looks up direct pairs; the inverse is the exact fraction
   flipped, so `USD → EUR → USD` round-trips within one minor unit of rounding.

3. **Decimals become fractions at scale `10^6`, then reduced.** Six places cover every
   quoted rate (JPY, KRW, IDR and their inverses stay within `1 ..= 10^12`, the engine's
   `MAX_RATE_PART`). Reduction by gcd keeps the rows readable (`1.0873` is `10873/10000`).
   A rate that does not fit fails the whole fetch: half a rate table is worse than the old
   one, and the error is loud.

4. **Manual wins.** `PUT /v1/exchange-rates/{from}/{to}` stamps origin `manual`; the fetch
   writes only pairs whose origin is not `manual`. An operator who corrects a rate during
   an incident must not have it undone an hour later; releasing it is an explicit `DELETE`.
   Origin lives in two new columns on `exchange_rates` (`origin`, `fetched_at`), never in
   the `ExchangeRate` body, so `PricingInput` and the DRT fixtures are untouched.

5. **Fetching is a use case over ports.** `RefreshExchangeRates { catalog, fetcher, clock }`
   in `src/application/` takes a `RateFetcher` port (`fetch(url) → Result<RateTable, FetchError>`);
   `src/infrastructure/` implements it with `reqwest` (rustls, the source's timeout, three
   redirects, `Accept: application/json`). The HTTP handler and the scheduler both call the
   use case, and tests use a stub fetcher plus one axum test against a local listener that
   serves a canned body.

6. **Scheduler in the server binary.** A tokio task ticks every 15 seconds and runs the use
   case when `now ≥ last_attempt_at + interval_seconds` or the source has never been tried,
   so a changed interval applies within 15 seconds and no restart is needed. Several
   replicas may fetch concurrently; the writes are upserts of the same values, so the race
   is harmless. Each attempt records `last_attempt_at`; only a success moves
   `last_success_at` and `last_count`.

7. **Freshness is computed, not stored, and its threshold is a setting.** `stale = now −
   last_success_at > max_age_seconds` (or from `updated_at` when nothing has succeeded) is
   derived in the status read model and
   exported as the gauge `discount.exchange_rates.stale`, next to `discount.exchange_rates.age`
   in seconds. The alert rule fires on the gauge, so Prometheus need not know the interval.
   Pricing never reads freshness: a failed fetch changes nothing in the rate table and a
   stale table is still the best table there is (ADR 7: rates enter only as input). The
   operator decides how old is too old, per source, because a daily provider and a
   minute-level feed differ by orders of magnitude.

9. **Own settings screen, status next to the rates.** The source has enough fields and
   state for a screen of its own (`/settings/exchange-rates/source`); the rates screen
   links to it and keeps only what an operator needs while looking at rates: the status
   line, the origin per row and Try again.

8. **The source URL is operator data.** The console has no authentication (README), so a
   URL with an API key is readable by every operator; the help text says so and the span
   attribute drops the query string. The fetcher sends no cookies or credentials and only
   follows `http`/`https`.

## Risks / Trade-offs

- [Provider shape drifts] → the fetch fails as a whole and the status says why; the stored
  rates stay. Adding a second shape is one more branch in the parser, not a new port.
- [SSRF through the URL] → the operator console is already a trusted surface (no auth, same
  gate as the service); documented, not mitigated beyond scheme and redirect limits.
- [Migration numbering] → this branch continues the sequence with `0007`. Work in flight
  elsewhere (`0007_tenants.sql`, uncommitted on main at the time of writing) must take the
  next number when it lands: sqlx refuses a database whose applied `0007` differs from the
  file, so two different `0007`s cannot coexist.
- [Reduced fractions look unlike the provider's decimal] → the console shows the decimal
  next to the fraction, as it already does for manual rates.

## Migration Plan

One migration: `exchange_rate_source` table, `exchange_rates.origin TEXT NOT NULL DEFAULT
'manual'` and `exchange_rates.fetched_at BIGINT NULL`. Existing rows become `manual`, which
is true. Rollback: drop the table and the two columns; the fetched rates remain ordinary
rates.
