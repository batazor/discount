# Tasks

## 1. HTTP adapter and binary (`adapters/`)

- [x] 1.1 Add axum, tokio, serde_json and a `discount-server` binary to `adapters/Cargo.toml`; verify `cargo build -p discount-adapters` succeeds
- [x] 1.2 Implement `adapters/src/config.rs` reading `DISCOUNT_LISTEN`, `OTEL_EXPORTER_OTLP_ENDPOINT`, `DISCOUNT_PROMOTIONS`, `DISCOUNT_ROUNDING`, `DISCOUNT_STACKING`, `DISCOUNT_ENV` with the documented defaults; verify with a unit test that an empty environment yields the defaults
- [x] 1.3 Implement `adapters/src/http.rs`: `POST /v1/evaluate` (`{cart, customer}` → `PricingResult`), `422 {"error": PricingError}`, `400 {"decode_error"}`, `GET /healthz`; verify with axum integration tests covering the four scenarios of `evaluate-http-api`
- [x] 1.4 Add a JSON-file loader for the in-memory repository (`adapters/src/memory.rs`) and `deploy/promotions.json` with the fixture promotions; verify a test loads the file and `Evaluate` prices the sample cart to `749`

## 2. Telemetry (`adapters/`)

- [x] 2.1 Add tracing, tracing-opentelemetry, opentelemetry, opentelemetry_sdk, opentelemetry-otlp, opentelemetry-appender-tracing to `adapters/Cargo.toml` and implement `adapters/src/telemetry.rs` (resource attributes, OTLP traces/metrics/logs pipelines, shutdown); verify the binary starts with an unreachable endpoint and still serves `/healthz`
- [x] 2.2 Instrument the handler: `discount.evaluate` span with the attributes of the `telemetry` spec and the structured log record; verify with a test using an in-memory span exporter that the attributes and the trace id on the log match
- [x] 2.3 Implement the metrics (`discount.evaluations`, `discount.evaluate.duration`, `discount.applied`, `discount.skipped`, `discount.amount`, `discount.share.bps` with the fixed buckets); verify with an in-memory metric reader test for both scenarios of "Evaluation metrics"
- [x] 2.4 Add the "Telemetry vocabulary" section to `docs/ubiquitous-language.md` listing metric names, attributes and allowed values; verify the names in the section match the constants in `adapters/src/telemetry.rs` (a unit test compares against a list in the module)

## 3. Local stack and dashboards (`deploy/`)

- [x] 3.1 Add a two-stage `Dockerfile` for `discount-server` and the `discount` service to `deploy/compose.yaml` (exports to `http://lgtm:4317`, `DISCOUNT_PROMOTIONS` mounted); verify `docker compose -f deploy/compose.yaml up -d --build` ends with both containers healthy
- [x] 3.2 Write `deploy/grafana/dashboards/overview.json` (evaluations by outcome, latency percentiles, error rate, discount per minute, share distribution, exemplars to Tempo); verify it appears in folder `Discount` via `GET /api/search?query=Discount` and panels render without query errors
- [x] 3.3 Write `deploy/grafana/dashboards/promotions.json` (per-promotion applied/skipped/discount, clamp breakdown); verify as in 3.2
- [x] 3.4 Write `deploy/grafana/dashboards/validation-errors.json` (errors by type, Loki panel of WARN records); verify as in 3.2
- [x] 3.5 Write `deploy/smoke.sh` (post fixtures, assert Prometheus series, Tempo trace, Loki line through Grafana proxies, non-zero exit on failure); verify it exits `0` on the running stack and non-zero when the service is stopped

## 4. Integration

- [x] 4.1 Add `discount-adapters` tests to the CI matrix (`.github/workflows/ci.yml`, already `cargo test --workspace`) and document the local smoke run in `README.md` ("Observability" section); verify CI stays green and the README commands run as written
- [x] 4.2 Pin the `grafana/otel-lgtm` image tag in `deploy/compose.yaml` after the first green smoke run; verify `docker compose pull` fetches the pinned tag

## Workflow follow-up

- Archive the change after review; `openspec archive otel-grafana-observability` merges the
  three capability specs into `openspec/specs/`.
