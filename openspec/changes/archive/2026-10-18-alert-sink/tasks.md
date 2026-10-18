# Tasks

## 1. Sink and provisioning (`deploy/`)

- [x] 1.1 Write `deploy/alert-sink/sink.py` (POST stores and prints, GET / lists, `/healthz`) and add the `alert-sink` service (`python:3-alpine`, port `9095`, healthcheck) to `deploy/compose.yaml`; verify `curl localhost:9095/` returns `[]` after `docker compose up -d alert-sink`
- [x] 1.2 Write `deploy/grafana/alerting/notifications.yaml` (contact point `discount-sink` → `http://alert-sink:8080/`, root policy with the grouping and timings of design decision 3), mount it; verify after restarting `lgtm` that `GET /api/v1/provisioning/contact-points` lists `discount-sink` and `GET /api/v1/provisioning/policies` has receiver `discount-sink`
- [x] 1.3 Verify delivery end to end: zero `P1`'s budget, see a `firing` notification for `Promotion budget exhausted` with `promotion_id=P1` on `GET localhost:9095/`, restore the budget, see `resolved`
- [x] 1.4 Extend `deploy/smoke.sh`: the sink answers `GET /` with `200`, the contact point `discount-sink` is provisioned; verify `deploy/smoke.sh` passes
