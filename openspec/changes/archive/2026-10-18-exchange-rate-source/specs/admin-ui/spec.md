# Spec Delta

## ADDED Requirements

### Requirement: Exchange-rate source screen
The console SHALL provide a screen Settings → Exchange rates → Source with a form for the
URL and, in human units (minutes, hours or days, sent as seconds), the fetch interval, the
maximum age before rates count as stale and the fetch timeout, submitted with
`PUT /v1/exchange-rate-source`. It SHALL show the status as the service reports it (last
successful fetch and number of pairs, next due time, the last error verbatim, a `stale`
badge) and offer Try again (`POST /v1/exchange-rate-source/refresh`, showing the `502`
message verbatim) and Turn off (`DELETE`, a destructive action that explains that stored
rates are kept and become manual). The help text SHALL say that a key in the URL is visible
to every operator and that pricing keeps the last stored rates while fetches fail.

#### Scenario: Configure from the console
- **WHEN** an operator enters a URL, chooses "every 1 hour", "stale after 1 day",
  "timeout 10 seconds" and saves
- **THEN** the console sends `PUT /v1/exchange-rate-source {url, interval_seconds: 3600,
  max_age_seconds: 86400, timeout_seconds: 10}` and the screen shows "next fetch" from the
  returned status

#### Scenario: Try again fails
- **WHEN** the operator clicks Try again and the provider is down
- **THEN** the screen shows the service's `502` message verbatim and the rate list is
  unchanged

### Requirement: Source status on the exchange-rates screen
The exchange-rates screen SHALL link to the source screen and, while a source is set, show
its status in one line (last fetch, stale badge, last error verbatim) with a Try again
button, and SHALL show on every rate row its origin (`manual`, or `source` with
`fetched_at`).

#### Scenario: Origin on rows
- **WHEN** the source stored `EUR → USD` and an operator set `USD → EUR` by hand
- **THEN** the first row shows `source` with its fetch time and the second shows `manual`
