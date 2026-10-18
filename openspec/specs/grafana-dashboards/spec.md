# grafana-dashboards Specification

## Purpose
Dashboards as code for the discount service, and the local stack that hosts them so the
whole chain can be checked on a laptop.

## Requirements

### Requirement: Local observability stack
The repository SHALL provide `deploy/compose.yaml` starting `grafana/otel-lgtm` (collector,
Prometheus, Tempo, Loki, Grafana), the discount service and the admin console, with
Grafana on `:3000` (anonymous admin), OTLP on `:4317`/`:4318`, the service on `:8080` and
the console on `:4321`, the service exporting to the collector and the console pointed at
the service.

#### Scenario: Stack comes up healthy
- **WHEN** `docker compose -f deploy/compose.yaml up -d` finishes
- **THEN** `GET http://localhost:3000/api/health` returns `200`,
  `GET http://localhost:8080/healthz` returns `200` and
  `GET http://localhost:4321/healthz` returns `200`

### Requirement: Provisioned dashboards
Grafana SHALL load the dashboards from `deploy/grafana/dashboards/*.json` into a
`Discount` folder on start. Three dashboards SHALL exist:
- **Discount engine overview**: evaluations per second by outcome, p50/p95/p99 of
  `discount.evaluate.duration`, error rate, total discount granted per minute, distribution
  of `discount.share.bps`, an exemplar link from the duration histogram to Tempo, and an
  "HTTP by route" row: requests per second, error ratio and p95 latency per server span
  name (route template) from the Tempo span metrics.
- **Promotions**: per `promotion_id` applied count, skipped count by reason, discount
  granted, share of applications clamped and by which bound.
- **Validation errors**: `discount.evaluations{outcome="error"}` by `error_type` over time
  and a log panel (Loki) filtered to `WARN` evaluation records.

#### Scenario: Dashboards present after start
- **WHEN** Grafana is up
- **THEN** `GET /api/search?query=Discount` lists the three dashboards in folder `Discount`

#### Scenario: Panels show data after traffic
- **WHEN** the smoke script has posted evaluations
- **THEN** the overview panels return non-empty series for `discount_evaluations_total`

### Requirement: Smoke script
`deploy/smoke.sh` SHALL post the fixture carts to the running service and then assert,
through Grafana's data source proxies, that `discount_evaluations_total` is present in
Prometheus, that a trace with `service.name=discount` is found in Tempo, and that a log
line is found in Loki; it SHALL also assert that the console answers `GET /healthz` and
that its promotions page lists `P1`; it SHALL exit non-zero otherwise.

#### Scenario: Smoke passes on a fresh stack
- **WHEN** the stack is up and `deploy/smoke.sh` runs
- **THEN** it prints the checks as passed, including the console check, and exits `0`

### Requirement: HTTP RED metrics come from span metrics
The HTTP panels SHALL be computed from Tempo's metrics-generator series
`traces_spanmetrics_calls_total` and `traces_spanmetrics_latency_bucket` with
`service="discount"` and `span_kind="SPAN_KIND_SERVER"`, grouped by `span_name` (the route
template); errors are `status_code="STATUS_CODE_ERROR"`. No new metric SHALL be added to the
service for this.

#### Scenario: Series after traffic
- **WHEN** the smoke script has posted evaluations
- **THEN** `traces_spanmetrics_calls_total{service="discount",span_name="POST /v1/evaluate"}` is non-empty
