# Tasks

## 1. Rules (`deploy/`)

- [x] 1.1 Write `deploy/grafana/alerting/discount.yaml` with the four rules (instant PromQL + threshold expression, labels, panel links, `noDataState: OK`) and mount it in `deploy/compose.yaml` under `/otel-lgtm/grafana/conf/provisioning/alerting/discount.yaml`; verify after `docker compose up -d lgtm` that `GET /api/v1/provisioning/alert-rules` lists the four `discount-*` rules
- [x] 1.2 Verify the budget rule end to end on the local stack: set `P1` budget to `0`, see `Promotion budget exhausted` reach `Alerting` with `promotion_id=P1` in `GET /api/prometheus/grafana/api/v1/rules`, restore the budget and see it return to `Normal`
- [x] 1.3 Add a check to `deploy/smoke.sh` that the provisioning API lists at least the four `discount-*` rules; verify `deploy/smoke.sh` passes
