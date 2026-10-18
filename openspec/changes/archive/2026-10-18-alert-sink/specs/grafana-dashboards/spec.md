# Spec Delta

## MODIFIED Requirements

### Requirement: Local observability stack
The repository SHALL provide `deploy/compose.yaml` starting `grafana/otel-lgtm` (collector,
Prometheus, Tempo, Loki, Grafana), the discount service, the admin console and
`alert-sink` (`deploy/alert-sink/sink.py`: receives Grafana's alert webhooks, prints one
JSON line per notification, serves the last 100 on `GET /`), with
Grafana on `:3000` (anonymous admin), OTLP on `:4317`/`:4318`, the service on `:8080` and
the console on `:4321`, the sink on `:9095`, the service exporting to the collector and the console pointed at
the service.

#### Scenario: Stack comes up healthy
- **WHEN** `docker compose -f deploy/compose.yaml up -d` finishes
- **THEN** `GET http://localhost:3000/api/health` returns `200`,
  `GET http://localhost:8080/healthz` returns `200` and
  `GET http://localhost:4321/healthz` returns `200` and `GET http://localhost:9095/`
  returns `200` with a JSON list

### Requirement: Provisioned alert rules
The local stack SHALL provision Grafana-managed alert rules from
`deploy/grafana/alerting/*.yaml` into folder `Discount`, group `discount` evaluated every
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
