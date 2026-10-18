# Design

## Context

Grafana's provisioned root policy in otel-lgtm routes to the receiver `empty`. Alerting
provisioning files may also define `contactPoints` and `policies`; a provisioned policy
file replaces the whole policy tree of the org. The stack already mounts
`deploy/grafana/alerting/discount.yaml` for the rules.

## Decisions

1. **A sink of our own, ~40 lines of Python standard library**, run from `python:3-alpine`
   with the script mounted. A generic echo image was considered; the own script gives a
   `GET /` that the smoke and the spec scenario can assert on, and prints a compact line
   (`status`, `alertname`, labels) instead of raw request dumps.
2. **Second provisioning file `notifications.yaml`** for the contact point and the policy,
   so the rules file stays what a deployment copies verbatim and the receiver is the one
   file it replaces.
3. **Policy**: root receiver `discount-sink`, `group_by: [alertname, promotion_id,
   span_name]`, `group_wait 10s`, `group_interval 30s`, `repeat_interval 1h`. Short timings
   suit a local loop; a deployment tunes them with its receiver.
4. **Deliberately no retention**: the sink keeps the last 100 notifications in memory. It is
   a verification aid, not an incident log; Grafana's own alert history remains the record.

## Risks / Trade-offs

- [Policy provisioning replaces any policy edited in the UI] → the UI is read-only for
  provisioned objects anyway; the repo is the source.
- [Sink down] → Grafana logs the delivery failure; rules keep evaluating. The smoke checks
  the sink is up.
