# Proposal

## Why

Server spans are now named by route template and `grafana/otel-lgtm` already derives
`traces_spanmetrics_*` from them, but no dashboard shows request rate, error ratio and
latency per route; operators still read HTTP health from the evaluation counters only.

## What Changes

- The **Discount engine overview** dashboard gains an "HTTP by route" row: requests per
  second by `span_name`, ratio of `STATUS_CODE_ERROR` by route, p95 latency by route, all
  from `traces_spanmetrics_calls_total` and `traces_spanmetrics_latency_bucket` filtered
  to `service="discount"` and `span_kind="SPAN_KIND_SERVER"`.
- The smoke script checks that `traces_spanmetrics_calls_total` has a series for
  `POST /v1/evaluate`.

Outside the verified scope: everything here; only `deploy/grafana/dashboards/overview.json`
and `deploy/smoke.sh` change.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `grafana-dashboards`: requirement "Provisioned dashboards" modified (overview gains the
  HTTP row), one added requirement on the span-metrics source.

## Impact

- `deploy/grafana/dashboards/overview.json`, `deploy/smoke.sh`.
