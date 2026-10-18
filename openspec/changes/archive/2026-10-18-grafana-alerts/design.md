# Design

## Context

`grafana/otel-lgtm` loads `conf/provisioning/alerting/*.yaml` on start (Grafana 13,
unified alerting). Dashboards are already mounted the same way. Metrics available:
`discount_*` from the service and `traces_spanmetrics_*` from Tempo's generator.

## Decisions

1. **Grafana-managed rules on the Prometheus data source, not Prometheus rule files.** The
   stack has no Alertmanager of its own and the operators live in Grafana; file
   provisioning gives the same review-by-diff as the dashboards.
2. **Instant queries plus a threshold expression**, no reduce step: each rule is one PromQL
   instant query (`A`) and one `threshold` expression (`B`), the condition. Multi-series
   rules (`span_name`, `promotion_id`) produce one alert instance per label set.
3. **`noDataState: OK`** everywhere: no traffic is the normal state of a local stack, and
   span-metric series disappear when a route is idle.
4. **Thresholds**: 5 % 5xx and 500 ms p95 are starting points for a pricing API called
   inline from checkout; 20 % pricing errors flags a client sending bad carts rather than
   single rejections; budget `< 1` minor unit is exhaustion by definition.
5. **No contact point in the repo**: receivers are deployment-specific; the labels
   `service` and `severity` are what a notification policy routes on.

## Risks / Trade-offs

- [Rule file syntax drifts with Grafana versions] → the smoke asserts the rules exist via
  the provisioning API after start, which fails loudly if provisioning rejected the file.
- [Budget gauge keeps the last value of an archived promotion] → the gauge is refreshed from
  `list_budgets` (non-archived only) every 15 s, so archived promotions drop out.

## Migration Plan

`docker compose -f deploy/compose.yaml up -d lgtm` (re-create to pick up the mount), then
`deploy/smoke.sh`. Rollback: remove the mount.
