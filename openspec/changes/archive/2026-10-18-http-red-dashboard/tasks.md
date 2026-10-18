# Tasks

## 1. Dashboard (`deploy/grafana/`)

- [x] 1.1 Add the "HTTP by route" row (three timeseries panels: requests/s, error ratio, p95 latency by `span_name` from `traces_spanmetrics_*`) to `deploy/grafana/dashboards/overview.json`; verify Grafana reloads it (`GET /api/dashboards/uid/discount-overview` lists the panels) and each query returns series through the Prometheus proxy

## 2. Smoke (`deploy/`)

- [x] 2.1 Add a retry check to `deploy/smoke.sh` that `traces_spanmetrics_calls_total{service="discount",span_name="POST /v1/evaluate"}` is non-empty; verify `deploy/smoke.sh` passes
