# Spec Delta

## MODIFIED Requirements

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

### Requirement: Smoke script
`deploy/smoke.sh` SHALL post the fixture carts to the running service and then assert,
through Grafana's data source proxies, that `discount_evaluations_total` is present in
Prometheus, that a trace with `service.name=discount` is found in Tempo, and that a log
line is found in Loki; it SHALL also assert that the console answers `GET /healthz` and
that its promotions page lists `P1`; it SHALL exit non-zero otherwise.

#### Scenario: Smoke passes on a fresh stack
- **WHEN** the stack is up and `deploy/smoke.sh` runs
- **THEN** it prints the checks as passed, including the console check, and exits `0`
