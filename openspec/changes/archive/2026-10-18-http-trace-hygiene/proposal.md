# Proposal

## Why

The `telemetry` spec is met for `POST /v1/evaluate`, but the HTTP server spans as a whole are
hard to use in Tempo: span names carry raw request paths (`PUT /v1/promotions/P3`,
`POST /v1/redemptions/smoke-1792324595/confirm`), so RED metrics per route are impossible and
name cardinality grows with every id; the Docker health check produces a trace every 10
seconds (285 `GET /healthz` traces per hour against 6 evaluations); server spans carry no
response status code and no error status, so failed requests cannot be searched; and no
incoming `traceparent` is honoured, so a caller's trace (admin UI, a shop backend) never
joins the service's.

## What Changes

- Server span names use the matched axum route template (`POST /v1/redemptions/{order_id}/confirm`),
  with the template also exported as `http.route`. The span name of the evaluation stays
  `POST /v1/evaluate`, so nothing the current spec or dashboards rely on moves.
- `GET /healthz` produces no span (and therefore no trace and no HTTP log line).
- Every server span records `http.response.status_code`; responses `5xx` set the span status
  to `Error`. Client errors (`4xx`, including the `422` pricing rejections) keep the status
  unset, as the OpenTelemetry HTTP semantic conventions prescribe for servers.
- W3C Trace Context propagation: an incoming `traceparent`/`tracestate` makes the server span
  a child of the caller's span; without it a new trace starts, as today.
- The Telemetry vocabulary in `docs/ubiquitous-language.md` gains `http.route` and
  `http.response.status_code`; the smoke script asserts a templated span name and a
  propagated trace id.

Outside the verified scope: everything here. Only `src/infrastructure/` (the HTTP adapter and
the telemetry module), `deploy/smoke.sh` and the vocabulary document change. The Lean model,
`src/domain/`, `src/application/` and pricing results are untouched; the `discount.evaluate`
span and all metrics keep their names and attributes.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `telemetry`: four added requirements on the HTTP server span (route-templated names, no
  health-check traces, response status, trace context propagation). Existing requirements
  are unchanged.

## Impact

- `src/infrastructure/src/http.rs`: the `TraceLayer` configuration (`make_span_with`,
  `on_response`), skipping `/healthz`, extracting the remote context.
- `src/infrastructure/src/telemetry.rs`: install the `TraceContextPropagator`, new vocabulary
  constants.
- `src/infrastructure/tests/telemetry.rs`, `tests/http.rs`: new scenarios on the in-memory
  span exporter.
- `deploy/smoke.sh`: templated-name and propagation checks against Tempo.
- `docs/ubiquitous-language.md`: Telemetry vocabulary section.
- Dashboards: `deploy/grafana/dashboards/*.json` reference `discount.evaluate` and metric
  names only; no panel changes expected (to be confirmed during implementation).
- No new crates: `axum::extract::MatchedPath`, `tower_http::trace` and
  `opentelemetry_sdk::propagation::TraceContextPropagator` are already available.
