# load-profile Specification

## Purpose
A reproducible load run against the local stack that reports latency per route, telemetry
cost per request and whether the alert thresholds hold, so thresholds and sampling are set
from numbers rather than judgement.

## Requirements

### Requirement: Load profile runs against the local stack
`ops/load/run.sh` SHALL seed the catalogue and run the k6 script `ops/load/discount.js`
from the `grafana/k6` image on the compose network against `discount:8080`, with the
arrival rate and duration taken from `RATE` (requests per second, default 50) and
`DURATION` (default `2m`), and SHALL leave the k6 summary in `ops/load/out/`.

#### Scenario: Default run
- **WHEN** the compose stack is up and `ops/load/run.sh` is executed
- **THEN** it exits zero, `ops/load/out/summary.json` exists and reports the four
  scenarios with their request counts

### Requirement: Traffic mix mirrors production shapes
The script SHALL run four scenarios at once: `evaluate` (valid carts, 70 % of the rate),
`redeem` (reserve, then confirm or cancel, 20 %), `repeat` (an idempotent second reserve of
an existing order, 5 %) and `invalid` (carts rejected with `422`, 5 %), each tagged with
its name so k6 and Tempo report them apart.

#### Scenario: Status codes per scenario
- **WHEN** a run completes
- **THEN** `evaluate` and `redeem` saw only `200`, `repeat` saw `200` with the first
  reservation's body, `invalid` saw only `422`

### Requirement: Thresholds mirror the alert rules
The script SHALL fail the run when 5xx responses exceed 5 % of requests, when p95 of
`POST /v1/redeem` exceeds 500 ms, or when p95 of `POST /v1/evaluate` exceeds 200 ms.

#### Scenario: Thresholds reported
- **WHEN** a run completes
- **THEN** the summary lists each threshold with pass or fail

### Requirement: The run is documented
`docs/load-profile.md` SHALL record the latest run: host, rate, duration, per-route p50/p95
from k6 and from Tempo span metrics, the `db.reserve` share of redeem latency, spans and
log lines per request, and the conclusions on alert thresholds and sampling.

#### Scenario: Document matches a run
- **WHEN** `ops/load/run.sh` is executed with the documented parameters
- **THEN** the p95 figures land within the margin stated in the document
