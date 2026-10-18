# Spec Delta

## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: HTTP RED metrics come from span metrics
The HTTP panels SHALL be computed from Tempo's metrics-generator series
`traces_spanmetrics_calls_total` and `traces_spanmetrics_latency_bucket` with
`service="discount"` and `span_kind="SPAN_KIND_SERVER"`, grouped by `span_name` (the route
template); errors are `status_code="STATUS_CODE_ERROR"`. No new metric SHALL be added to the
service for this.

#### Scenario: Series after traffic
- **WHEN** the smoke script has posted evaluations
- **THEN** `traces_spanmetrics_calls_total{service="discount",span_name="POST /v1/evaluate"}` is non-empty
