# Proposal

## Why

`discount.redemptions{outcome="error"}` counts every `Err` of a redemption action: a `409`
`InvalidTransition` (confirming twice), a `404` unknown order and a `422` pricing rejection
land in the same bucket as a Postgres outage. The signal is useless for alerting, which is
why the alert set (change `grafana-alerts`) has no rule on redemptions.

## What Changes

- `outcome` of `discount.redemptions` becomes `ok | idempotent | rejected | error`:
  `rejected` for outcomes the caller caused (`NotFound`, `InvalidTransition`, `Pricing`),
  `error` only for storage failures. Rejections carry `reason` with the variant name.
- Promotions dashboard: the redemptions stat shows rejected separately.
- A fifth alert rule: any redemption storage error in 5 minutes (`critical`).
- Vocabulary updated.

**BREAKING** for anyone querying `outcome="error"` as "anything went wrong": those counts
now split. Nothing in the repo did.

Outside the verified scope: everything here; `src/infrastructure/src/telemetry.rs`, its
test, `deploy/grafana/`, `docs/ubiquitous-language.md`.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `telemetry`: "Redemption metrics" (outcome values, `reason`).
- `grafana-dashboards`: unchanged requirement text; the fifth rule and the stat are
  implementation detail within "Provisioned alert rules" and "Provisioned dashboards".

## Impact

- `src/infrastructure/src/telemetry.rs` (`record_redemption`, `record_transition`),
  `src/infrastructure/tests/telemetry.rs`, `deploy/grafana/dashboards/promotions.json`,
  `deploy/grafana/alerting/discount.yaml`, `deploy/smoke.sh` (rule count),
  `docs/ubiquitous-language.md`.
