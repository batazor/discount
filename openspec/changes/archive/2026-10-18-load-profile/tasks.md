# Tasks

## 1. Load profile (`deploy/load/`)

- [x] 1.1 Write `deploy/load/discount.js`: four `constant-arrival-rate` scenarios (evaluate 70 %, redeem 20 %, repeat 5 %, invalid 5 %) tagged by name, checks on status codes and bodies, thresholds (5xx < 5 %, redeem p95 < 500 ms, evaluate p95 < 200 ms), `handleSummary` writing `out/summary.json` and the text summary; verify with `RATE=5 DURATION=20s deploy/load/run.sh`
- [x] 1.2 Write `deploy/load/run.sh` (health check, catalogue seeding, `docker run --network deploy_default grafana/k6` with `RATE`, `DURATION`, `BASE_URL`), add `deploy/load/out/` to `.gitignore`; verify a 20 s run exits zero and leaves `summary.json`

## 2. Measurement (`docs/`)

- [x] 2.1 Run the default profile (`RATE=50 DURATION=2m`), then read Tempo span metrics for p50/p95 per route and for `db.reserve`, and count spans and Loki lines per request; verify every figure is captured in `deploy/load/out/` or the shell log
- [x] 2.2 Write `docs/load-profile.md` with host, parameters, the k6 and Tempo figures, the `db.reserve` share, telemetry volume per request, and conclusions on the alert thresholds and on sampling; add a paragraph to `README.md` under the local stack section; verify the thresholds stated match `deploy/grafana/alerting/discount.yaml`
