# Design

## Context

`grafana/otel-lgtm` runs Tempo's metrics generator; Prometheus already holds
`traces_spanmetrics_calls_total{service,span_kind,span_name,status_code}` and
`traces_spanmetrics_latency_bucket`. Dashboards are JSON in `deploy/grafana/dashboards`,
provisioned read-only, with an `$env` variable on `deployment_environment`.

## Decisions

1. **Span metrics, not a new HTTP metric in the service.** The data already exists and the
   span name is now the route template; adding an `http.server.request.duration` histogram
   to the service would duplicate it.
2. **Three timeseries panels in one row at `y=20`**: rate by route (`sum by (span_name)
   (rate(calls[5m]))`), error ratio by route (`ERROR / all`, `percentunit`), p95 by route
   (`histogram_quantile(0.95, sum by (le, span_name) (rate(latency_bucket[5m])))`, unit
   `s`). `GET /healthz` no longer appears since it is not traced.
3. **No `$env` filter on these panels.** Span metrics carry no `deployment_environment`
   label (the generator keeps `service` only); the panel titles say so.

## Risks / Trade-offs

- [Span-metric series lag ingestion by up to a minute] → the smoke retries like the Tempo
  checks.
