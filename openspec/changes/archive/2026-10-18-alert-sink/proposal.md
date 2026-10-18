# Proposal

## Why

Five alert rules are provisioned and verified to reach `Alerting`, but the root
notification policy of the local Grafana points at the receiver `empty`: a firing alert goes
nowhere, and the path rule → policy → contact point → delivery has never been exercised.

## What Changes

- A local alert sink in the stack: a small HTTP service (`deploy/alert-sink/sink.py`, Python
  standard library, `python:3-alpine`) that accepts Grafana webhook notifications, prints one
  JSON line per notification and serves the last 100 on `GET /` for verification.
- Grafana provisioning gains a contact point `discount-sink` (webhook to the sink) and a
  notification policy routing `service=discount` to it, grouped by `alertname` and
  `promotion_id`, with short local timings.
- The smoke script asserts the sink answers and the contact point is provisioned.

Outside the verified scope: everything here; `deploy/` only. A deployment replaces the
contact point with its own receiver; the policy and labels stay.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `grafana-dashboards`: "Provisioned alert rules" (a contact point is now provisioned),
  "Local observability stack" (the sink is part of the stack).

## Impact

- `deploy/alert-sink/sink.py`, `deploy/compose.yaml` (service `alert-sink`, port 9095),
  `deploy/grafana/alerting/notifications.yaml` (contact point, policy), `deploy/smoke.sh`.
