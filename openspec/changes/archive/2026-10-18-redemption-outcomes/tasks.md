# Tasks

## 1. Metrics (`src/infrastructure/`)

- [x] 1.1 In `src/infrastructure/src/telemetry.rs` add `redemption_outcome()` and use it in `record_redemption` and `record_transition` (`rejected` + `reason`, `error` for `Storage` only); verify `cargo clippy -p discount-infrastructure --all-targets -- -D warnings`
- [x] 1.2 In `src/infrastructure/tests/telemetry.rs` add the two scenarios (second confirm → `rejected/InvalidTransition`, storage failure → `error`) and keep the existing redemption test green; verify `cargo test -p discount-infrastructure --test telemetry`

## 2. Vocabulary, dashboard, alert (`docs/`, `deploy/`)

- [x] 2.1 Update the `discount.redemptions` line in `docs/ubiquitous-language.md`; verify `cargo test -p discount-infrastructure --lib vocabulary`
- [x] 2.2 Add a "rejected" target to the redemptions stat in `deploy/grafana/dashboards/promotions.json` and the rule `discount-redemption-storage-errors` to `deploy/grafana/alerting/discount.yaml`; bump the smoke rule count to 5; verify locally: rebuild the service, `docker compose up -d lgtm`, run `deploy/smoke.sh`, then `discount_redemptions_total{outcome="rejected",reason="InvalidTransition"}` is non-empty in Prometheus and the five rules are listed
