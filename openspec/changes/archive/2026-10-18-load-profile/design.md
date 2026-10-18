# Design

## Context

The compose stack (`deploy/compose.yaml`) runs the service, Postgres and otel-lgtm on the
`deploy_default` network; the service exports every span (always-on sampler) and Tempo's
metrics generator derives RED metrics per span name. No load tool is installed on the host.

## Decisions

1. **k6 from the official image on the compose network.** `docker run --network
   deploy_default grafana/k6` reaches `http://discount:8080` without exposing anything
   new and installs nothing on the host. `oha` was considered for a one-line smoke of
   `/v1/evaluate` but cannot express the reserve → confirm lifecycle.
2. **Open-model scenarios (`constant-arrival-rate`)**, not virtual-user loops: an arrival
   rate is what a checkout produces, and it keeps latency measurements independent of how
   slow the service gets. Four scenarios share the rate 70/20/5/5.
3. **Seeding by the runner, not the script.** `run.sh` PUTs the same group and promotions
   as the smoke (large budget on P1 so the run never hits `Budget` clamps unless asked) and
   checks `/healthz`; the script only sends traffic. The runner also passes `RATE`,
   `DURATION` and the base URL as k6 environment.
4. **Thresholds equal to the alert thresholds**, plus 200 ms p95 on evaluate as the first
   stated target for pure pricing. A failed threshold fails `run.sh`, which is the point:
   the run is a check, not a benchmark.
5. **Numbers from two sides.** k6 measures the client view; the document also reads
   Tempo span metrics (`traces_spanmetrics_latency_bucket`) for the server view and for
   `db.reserve` vs `POST /v1/redeem`, and counts spans and log records per request from
   `traces_spanmetrics_calls_total` and Loki. Telemetry cost per request is what decides
   sampling, so it is measured, not estimated.
6. **Results stay out of git** except the document: `deploy/load/out/` is ignored; the
   document quotes the figures with the parameters that produced them.

## Risks / Trade-offs

- [Numbers depend on the laptop] → the document names the host; the thresholds are
  deliberately loose enough to hold on a developer machine, so a failed run means a real
  regression.
- [Tempo metrics lag a minute] → the runner waits before reading them; the document notes
  the read window.
