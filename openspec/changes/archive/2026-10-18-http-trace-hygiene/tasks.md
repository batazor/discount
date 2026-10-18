# Tasks

## 1. Telemetry module (`src/infrastructure/`)

- [x] 1.1 In `src/infrastructure/src/telemetry.rs` install `TraceContextPropagator` as the global text-map propagator in `init`, add the constants `SPAN_ATTR_HTTP_ROUTE` (`http.route`) and `SPAN_ATTR_HTTP_STATUS` (`http.response.status_code`) and list them in `VOCABULARY`; verify `cargo test -p discount-infrastructure vocabulary` still passes once the doc is updated (task 3.1)

## 2. HTTP adapter (`src/infrastructure/`)

- [x] 2.1 In `src/infrastructure/src/http.rs` rewrite `make_span_with`: name from `MatchedPath` (`<METHOD> <template>`, method only when unmatched), record `http.route`, declare `http.response.status_code` as `Empty`, return `Span::none()` for `/healthz`, extract the remote W3C context from headers (local `HeaderExtractor`) and `set_parent` it; verify `cargo build -p discount-infrastructure`
- [x] 2.2 Add `on_response` to the `TraceLayer`: record `http.response.status_code` and set span status `Error` for 5xx; verify `cargo clippy -p discount-infrastructure -- -D warnings`
- [x] 2.3 In `src/infrastructure/tests/telemetry.rs` add scenarios on the in-memory exporter: templated name and `http.route` for `POST /v1/redemptions/{order_id}/confirm`; `POST /v1/evaluate` unchanged with child span; no span for `GET /healthz`; `422` status code with unset status; `500` with status `Error` (failing repository); `traceparent` honoured (trace id and parent span id) and malformed `traceparent` ignored; verify `cargo test -p discount-infrastructure --test telemetry`

## 3. Vocabulary and smoke (`docs/`, `deploy/`)

- [x] 3.1 Add `http.route` and `http.response.status_code` (server span, 5xx → status `Error`, `/healthz` untraced) to the Telemetry vocabulary section of `docs/ubiquitous-language.md`; verify `cargo test -p discount-infrastructure vocabulary`
- [x] 3.2 In `deploy/smoke.sh` send one evaluate request with a fixed `traceparent` and one `POST /v1/redemptions/<id>/confirm`, then assert through the Tempo proxy that the trace with that id exists and that a span named `POST /v1/redemptions/{order_id}/confirm` is searchable (`{name="..."}`); verify `docker compose -f deploy/compose.yaml up -d --build discount && deploy/smoke.sh` passes
- [x] 3.3 Confirm `deploy/grafana/dashboards/*.json` contain no query on raw HTTP span names (`grep -l 'name=' deploy/grafana/dashboards`); adjust any panel found; verify the dashboards still render without query errors in Grafana
