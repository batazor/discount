# Design

## Context

`Instruments::record_redemption` (reserve) and `record_transition` (confirm, cancel) in
`src/infrastructure/src/telemetry.rs` label any `Err` as `error`. `RedemptionError` has
four variants: `NotFound`, `InvalidTransition`, `Pricing`, `Storage`.

## Decisions

1. **One classifier for both paths.** `redemption_outcome(&RedemptionError) -> (outcome,
   Option<reason>)` maps `Storage` to `error` and the other three to `rejected` with the
   variant name as `reason`; both record functions use it. A pricing rejection during
   reserve still feeds the evaluation metrics as before (`discount.evaluations` error with
   `error_type`), so nothing is lost there.
2. **`reason` only on `rejected`.** Keeps the `ok`/`error` series free of an empty label
   and the cardinality at three extra series per action.
3. **Alert on `error` rate > 0 for 5 m**, critical: with `error` meaning storage only, a
   single occurrence is worth a page; `for: 5m` avoids waking on one reconnect blip.
4. **Dashboard**: the by-outcome timeseries picks the new label up by itself; the stat row
   gains "rejected" so the split is visible at a glance.

## Risks / Trade-offs

- [Dashboards elsewhere summing `outcome="error"`] → none in the repo; the proposal marks
  the change breaking for external consumers.
