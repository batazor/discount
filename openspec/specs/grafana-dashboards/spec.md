# grafana-dashboards Specification

## Purpose
Dashboards as code for the discount service, and the local stack that hosts them so the
whole chain can be checked on a laptop.

## Requirements

### Requirement: Local observability stack
The repository SHALL provide `ops/compose.yaml` starting `grafana/otel-lgtm` (collector,
Prometheus, Tempo, Loki, Grafana), the discount service, the admin console and
`alert-sink` (`ops/alert-sink/sink.py`: receives Grafana's alert webhooks, prints one
JSON line per notification, serves the last 100 on `GET /`), with
Grafana on `:3000` (anonymous admin), OTLP on `:4317`/`:4318`, the service on `:8080` and
the console on `:4321`, the sink on `:9095`, the service exporting to the collector and the console pointed at
the service.

#### Scenario: Stack comes up healthy
- **WHEN** `docker compose -f ops/compose.yaml up -d` finishes
- **THEN** `GET http://localhost:3000/api/health` returns `200`,
  `GET http://localhost:8080/healthz` returns `200` and
  `GET http://localhost:4321/healthz` returns `200` and `GET http://localhost:9095/`
  returns `200` with a JSON list

### Requirement: Provisioned dashboards
Grafana SHALL load the dashboards from `ops/grafana/dashboards/*.json` into a
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
`ops/smoke.sh` SHALL post the fixture carts to the running service and then assert,
through Grafana's data source proxies, that `discount_evaluations_total` is present in
Prometheus, that a trace with `service.name=discount` is found in Tempo, and that a log
line is found in Loki; it SHALL also assert that the console answers `GET /healthz` and
that its promotions page lists `P1`; it SHALL exit non-zero otherwise.

#### Scenario: Smoke passes on a fresh stack
- **WHEN** the stack is up and `ops/smoke.sh` runs
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

### Requirement: Provisioned alert rules
The local stack SHALL provision Grafana-managed alert rules from
`ops/grafana/alerting/*.yaml` into folder `Discount`, group `discount` evaluated every
minute. Each rule SHALL carry labels `service=discount` and `severity` and link to the
dashboard panel it is derived from. No contact point SHALL be provisioned.

#### Scenario: Rules present after start
- **WHEN** Grafana is up
- **THEN** `GET /api/v1/provisioning/alert-rules` lists the five rules with uids prefixed
  `discount-`, all in folder `Discount`

#### Scenario: Rule set
- **WHEN** the rules are listed
- **THEN** they are: HTTP 5xx ratio per route above 5 % for 5 minutes (`warning`); p95
  latency of `POST /v1/redeem` above 500 ms for 5 minutes (`warning`); pricing error ratio
  above 20 % for 10 minutes (`warning`); a promotion with `discount.budget.remaining` at or
  below zero for 1 minute (`critical`); any `discount.redemptions{outcome="error"}` in
  5 minutes (`critical`)

#### Scenario: Exhausted budget fires
- **WHEN** a promotion's remaining budget is set to `0` and the gauge has been exported
- **THEN** within three minutes the rule `Promotion budget exhausted` is `Alerting` with
  label `promotion_id` naming that promotion, and returns to `Normal` after the budget is
  restored

#### Scenario: Firing alert is delivered
- **WHEN** `Promotion budget exhausted` fires for `P1`
- **THEN** within two minutes `GET http://localhost:9095/` lists a notification with
  `status` `firing` and an alert labelled `alertname=Promotion budget exhausted`,
  `promotion_id=P1`; after the budget is restored a `resolved` notification follows
