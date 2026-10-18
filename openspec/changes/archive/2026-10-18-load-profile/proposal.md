# Proposal

## Why

Every threshold in the alert rules (5 % 5xx, 500 ms p95 on redeem) and the decision to
trace every request with every database call were set by judgement, not by measurement.
Nothing in the repo can put load on the stack, so the questions "where does `/v1/redeem`
saturate", "how much does telemetry cost per request" and "do we need sampling" have no
answer.

## What Changes

- A k6 load profile in `deploy/load/` (script + runner) that drives the local compose stack
  with a mix of evaluations, redemption lifecycles, idempotent repeats and invalid carts at
  a configurable arrival rate, with k6 thresholds mirroring the alert thresholds.
- A documented run (`docs/load-profile.md`): the numbers from the stack (k6 summary, Tempo
  span metrics per route, the `db.reserve` share of redeem latency, telemetry volume per
  request) and what they say about the alert thresholds and sampling.

Outside the verified scope: everything here; `deploy/load/` and `docs/`. No service code
changes. k6 runs from the `grafana/k6` image on the compose network; nothing is installed on
the host and nothing runs in CI.

## Capabilities

### New Capabilities
- `load-profile`: the reproducible local load run and what it must report.

### Modified Capabilities
<!-- none -->

## Impact

- `deploy/load/discount.js`, `deploy/load/run.sh`, `deploy/load/out/` (git-ignored
  results), `docs/load-profile.md`, `README.md` (one paragraph).
