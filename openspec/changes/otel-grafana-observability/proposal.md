# Proposal

## Why

The product's differentiator is not a UI with charts but first-class observability: every
evaluation of the pricing engine is visible in Grafana as traces, metrics and logs, and
merchants and operators read promotion performance from provisioned dashboards. Today the
engine has no network edge and emits nothing; the local Grafana stack already runs
(`deploy/compose.yaml`) and is waiting for a signal.

## What Changes

- A first HTTP adapter (axum) exposing the `Evaluate` use case as `POST /v1/evaluate`, so
  the engine can be exercised end to end and produce telemetry.
- OpenTelemetry instrumentation in the adapters layer only: one trace per evaluation with
  the pricing outcome as span attributes, metrics for evaluations, applied and skipped
  promotions, discount amounts and validation errors, and logs correlated with traces.
  Export over OTLP to any collector; the local stack is `grafana/otel-lgtm`.
- Grafana dashboards as code, provisioned by the local stack: an engine overview, a
  promotion performance board and a validation-errors board.
- A smoke script that drives the local stack and checks that the signals arrive.

Outside the verified scope: everything here. `core/` stays free of telemetry (ADR 7, ADR 8);
the Lean model and the DRT are untouched. Instrumentation reads the `PricingResult`, it never
influences pricing.

## Capabilities

### New Capabilities
- `evaluate-http-api`: the HTTP contract of the `Evaluate` use case (request, response,
  error mapping, health).
- `telemetry`: what the service emits over OpenTelemetry per evaluation (traces, metrics,
  logs), with stable names and attributes that dashboards rely on.
- `grafana-dashboards`: the provisioned Grafana dashboards and the local observability
  stack that hosts them.

### Modified Capabilities
<!-- none: no existing OpenSpec specs -->

## Impact

- `adapters/`: new `http` and `telemetry` modules, new binary `discount-server`, new
  dependencies (axum, tokio, tracing, tracing-opentelemetry, opentelemetry, opentelemetry-otlp,
  opentelemetry_sdk, opentelemetry-appender-tracing, serde_json).
- `app/`: unchanged API; `Evaluate` is called from the HTTP handler.
- `deploy/`: `compose.yaml` gains the service container, `grafana/dashboards/*.json`,
  `promotions.json` fixture, `smoke.sh`.
- `.github/workflows/ci.yml`: builds and tests the adapters crate; the Docker smoke run
  stays local (documented), not in CI for now.
- `docs/ubiquitous-language.md`: a short "Telemetry vocabulary" section listing the metric
  and attribute names, since they reuse the domain vocabulary (`clamped_by`, `SkipReason`).
