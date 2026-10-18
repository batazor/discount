# Proposal

## Why

The provisioned dashboards show HTTP errors, latency, pricing rejections and remaining
budgets, but nothing notifies anyone: an exhausted promotion budget or a burst of 5xx on
`POST /v1/redeem` is visible only to someone looking at Grafana at that moment.

## What Changes

- Grafana-managed alert rules as code in `deploy/grafana/alerting/discount.yaml`,
  provisioned by the local stack into folder `Discount`, evaluation group `discount` (1m):
  HTTP 5xx ratio per route, p95 latency of `POST /v1/redeem`, pricing error ratio, and
  promotion budget exhausted. Each rule links to the panel it is derived from.
- No contact point is provisioned: notifications go to Grafana's default policy; a
  deployment adds its own receiver. The rules carry `severity` and `service` labels for
  routing.
- The smoke script asserts the rules are provisioned.

Outside the verified scope: everything here; `deploy/` only.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `grafana-dashboards`: one added requirement, "Provisioned alert rules"; the smoke
  requirement is left as is (its new check is additive and covered by the added one).

## Impact

- `deploy/grafana/alerting/discount.yaml` (new), `deploy/compose.yaml` (one mount),
  `deploy/smoke.sh`.
