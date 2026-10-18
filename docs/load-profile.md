# Load profile of the local stack

What the service does under load on a developer machine, measured with the k6 profile in
`deploy/load/` (spec `load-profile`). Numbers are from one machine and one evening; they
set the alert thresholds and the sampling decision, nothing more.

## Setup

- Host: Apple M5 Pro, Docker Desktop with 18 CPUs and 7.7 GiB for containers. The service,
  Postgres, otel-lgtm **and k6** share that budget, so client-side latency includes the
  load generator's own scheduling.
- Stack: `deploy/compose.yaml` as of 2026-10-18, `PgStore` pool of 8 connections,
  always-on tracing (every request, every repository call as a `db.<method>` span).
- Traffic: `deploy/load/discount.js`, four open-model scenarios sharing the rate
  (evaluate 70 %, redeem lifecycle 20 %, idempotent repeat 5 %, invalid cart 5 %).
- Other traffic was present during both runs (the operator console and its checks were
  being exercised in parallel), so absolute span counts in Tempo are not quoted; ratios
  per request come from k6's own counts.

Reproduce: `deploy/load/run.sh` (50 req/s, 2 min) and `RATE=200 deploy/load/run.sh`.

## Run 1: 50 req/s for 2 minutes

k6: 7 686 requests, 0 failures, all checks passed, all thresholds held.

| route (client view, k6) | p50 | p95 | max |
|---|---|---|---|
| `POST /v1/evaluate` | 5.6 ms | 7.8 ms | 12.9 ms |
| `POST /v1/redeem` | 6.7 ms | 11.2 ms | 17.0 ms |
| `POST /v1/redemptions/{id}/confirm` | 0.8 ms | 1.7 ms | 4.3 ms |

| span (server view, Tempo span metrics) | p50 | p95 | p99 |
|---|---|---|---|
| `POST /v1/evaluate` | 5.7 ms | 7.8 ms | 8.0 ms |
| `discount.evaluate` | 5.7 ms | 8.0 ms | 14.1 ms |
| `db.snapshot` | 3.4 ms | 7.4 ms | 7.9 ms |
| `db.uses_for_customer` | 1.1 ms | 2.3 ms | 3.7 ms |
| `db.allowlisted_for` | 1.0 ms | 1.9 ms | 2.0 ms |
| `POST /v1/redeem` | 5.8 ms | 13.0 ms | 15.4 ms |
| `db.reserve` | 5.6 ms | 12.1 ms | 15.2 ms |
| `db.transition` | 1.0 ms | 1.9 ms | 3.5 ms |

Reading: an evaluation is three sequential round trips to Postgres (catalogue snapshot,
customer uses, allowlist) adding up to about 5.5 ms of its 5.7 ms; the pricing itself is
well under a millisecond. A reservation is one transaction (`db.reserve`) that is 97 % of
the request. Client and server views agree within 0.1 ms at this rate: no queueing anywhere.

Telemetry per request (k6 counts against Tempo and Loki): evaluate produces 5 spans
(server, `discount.evaluate`, 3 × `db.*`) and 1 log record; a reservation 4 spans
(`db.allowlisted_for` runs inside `db.reserve`'s pricing closure) and 1 log record;
confirm or cancel 2 spans and no evaluation log. At 50 req/s that is roughly 230 spans/s
and 45 log lines/s into otel-lgtm, which stayed at about 6 % CPU.

## Run 2: 200 req/s for 2 minutes

k6: 30 004 requests, 0 failures, all checks passed; threshold `evaluate p95 < 200 ms`
**crossed** (210.7 ms), redeem p95 229 ms under its 500 ms.

| route (client view, k6) | p50 | p90 | p95 | max |
|---|---|---|---|---|
| `POST /v1/evaluate` | 14.4 ms | 20.5 ms | 210.7 ms | 716 ms |
| `POST /v1/redeem` | 17.1 ms | 66.0 ms | 229.0 ms | 601 ms |
| confirm | 0.6 ms | 1.6 ms | 99.1 ms | 240 ms |

| span (server view) | p50 | p95 | p99 |
|---|---|---|---|
| `POST /v1/evaluate` | 13.8 ms | 29.8 ms | 31.6 ms |
| `db.snapshot` | 9.6 ms | 15.4 ms | 15.9 ms |
| `db.uses_for_customer` | 4.4 ms | 7.7 ms | 7.9 ms |
| `POST /v1/redeem` | 14.4 ms | 51.2 ms | 79.6 ms |
| `db.reserve` | 14.2 ms | 50.8 ms | 79.6 ms |
| `db.transition` | 1.1 ms | 19.6 ms | 38.6 ms |

Reading: inside the service everything is still bounded (evaluate p99 32 ms, redeem
p99 80 ms) and no request failed; the service log has no pool timeouts. The growth is in
the database calls, `db.snapshot` ×2.8 and `db.reserve` ×2.5 at p50, which is the
8-connection pool being shared by 200 req/s × 3 calls. The client-side p95 of 210 ms with
a server-side p95 of 30 ms means ~180 ms spent before the server span starts: accept
queue and scheduling on a host that also runs k6 with 420 virtual users and the whole
LGTM stack. It cannot be attributed to the service from this machine; it is the reason
the thresholds in this document are not tightened further.

## Conclusions

**Alert thresholds** (`deploy/grafana/alerting/discount.yaml`) hold with a wide margin at
50 req/s and the redeem one still holds at 200 req/s on a shared laptop:

- `POST /v1/redeem` p95 > 500 ms: measured 11 ms at 50 req/s, 229 ms at 200 req/s client
  side (51 ms server side). Keep 500 ms; it fires on real saturation, not on noise.
- 5xx ratio > 5 %: zero 5xx in 37 690 requests. Keep.
- The k6 threshold `evaluate p95 < 200 ms` is a client-side target and failed only at
  200 req/s on the shared host; the server-side p95 was 30 ms. Keep it as a run check, do
  not turn it into an alert until measured on a dedicated host.

**Sampling**: not yet. Telemetry volume is ~5 spans and ~1 log line per request; the
collector sat at ~6 % CPU at 50 req/s and the service at under 20 MiB. At 200 req/s the
cost that grew was Postgres time, not export. Revisit when sustained traffic passes a few
hundred requests per second; the knob is `OTEL_TRACES_SAMPLER=parentbased_traceidratio`
in the service environment, with tail sampling of errors at the collector.

**Where the time goes, and the next optimisation if one is needed**: three sequential
Postgres round trips per evaluation. Fetching `uses_for_customer` and `allowlisted_for`
concurrently with the snapshot, or caching the catalogue snapshot for a few seconds,
would take evaluate p50 from ~5.7 ms to ~3.5 ms and lift the pool ceiling by a third.
Neither is needed at today's traffic; both are measurable with this profile.
