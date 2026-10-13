# grafana-dashboards Specification

## Purpose
Dashboards as code for the discount service, and the local stack that hosts them so the
whole chain can be checked on a laptop.

## Requirements

### Requirement: Local observability stack
The repository SHALL provide `deploy/compose.yaml` starting `grafana/otel-lgtm` (collector,
Prometheus, Tempo, Loki, Grafana) and the discount service, with Grafana on `:3000`
(anonymous admin), OTLP on `:4317`/`:4318` and the service on `:8080`, the service
exporting to the collector.

#### Scenario: Stack comes up healthy
- **WHEN** `docker compose -f deploy/compose.yaml up -d` finishes
- **THEN** `GET http://localhost:3000/api/health` returns `200` and
  `GET http://localhost:8080/healthz` returns `200`

### Requirement: Provisioned dashboards
Grafana SHALL load the dashboards from `deploy/grafana/dashboards/*.json` into a
`Discount` folder on start. Three dashboards SHALL exist:
- **Discount engine overview**: evaluations per second by outcome, p50/p95/p99 of
  `discount.evaluate.duration`, error rate, total discount granted per minute, distribution
  of `discount.share.bps`, and an exemplar link from the duration histogram to Tempo.
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
line is found in Loki; it SHALL exit non-zero otherwise.

#### Scenario: Smoke passes on a fresh stack
- **WHEN** the stack is up and `deploy/smoke.sh` runs
- **THEN** it prints the three checks as passed and exits `0`
