# Design

## Context

See proposal.md for motivation. Current state: `app::Evaluate` exists with sync ports;
`adapters/` holds an in-memory repository, a system clock and a no-op publisher; there is
no binary and no network edge. `deploy/compose.yaml` runs `grafana/otel-lgtm` and already
accepts OTLP (verified by hand: a metric reached Prometheus, a trace reached Tempo).
Constraints: `core/` is std + serde only and must stay telemetry-free (ADR 8); `app/` keeps
its dependency-free ports; `PricingResult` is the single source for everything emitted.

## Goals / Non-Goals

**Goals:**
- Every evaluation observable as a correlated trace, metric samples and a log record.
- Metric and attribute names that are stable and reuse the ubiquitous language.
- Dashboards versioned in the repo and provisioned automatically.
- The whole chain runnable and checkable locally in one command plus one script.

**Non-Goals:**
- Business analytics warehouse, long-term storage, alert rules (later, on top of the same
  metrics).
- Telemetry from the Lean reference or the DRT.
- Authentication on the HTTP API; multi-tenant resource attributes.
- Postgres repository (separate change).

## Decisions

1. **Instrumentation lives in `adapters/`, not `app/`.** The HTTP handler wraps the
   `Evaluate` call in a span and derives metrics from the returned `PricingResult`.
   Alternative: `tracing` spans inside `app::Evaluate`. Rejected for now: it adds a
   dependency to the application layer for no extra signal, since everything we emit is
   derivable from the result at the edge.

2. **Stack: `tracing` + `tracing-opentelemetry` for spans, the `opentelemetry` metrics API
   directly for counters/histograms, `opentelemetry-appender-tracing` for logs,
   `opentelemetry-otlp` (tonic/gRPC) exporter.** Alternative: Prometheus pull endpoint
   plus separate tracing. Rejected: one OTLP pipeline gives traces, metrics and logs with
   shared resource attributes and trace correlation for free, and the target is the
   Grafana OTel stack.

3. **Metric naming follows OpenTelemetry semantic conventions (dotted names, unit in
   instrument), e.g. `discount.evaluate.duration` in seconds.** The collector/Prometheus
   translation yields `discount_evaluate_duration_seconds_bucket` etc.; dashboards use the
   translated names. Alternative: Prometheus-style names in code. Rejected: OTel names are
   backend-neutral.

4. **Cardinality:** `promotion_id` is an attribute on `discount.applied` / `discount.skipped`
   only. It is bounded by the promotion catalogue (tens to low hundreds). `line_id`,
   customer ids and amounts are never attributes; amounts go into histograms.

5. **Discount share as `discount.share.bps` histogram** with explicit bucket boundaries at
   `0, 100, 250, 500, 1000, 1500, 2000, 3000, 5000, 7500, 10000`. Alternative: compute in
   PromQL from two sums. Rejected: ratio-of-sums hides the distribution, and the per-cart
   share is what merchants ask about ("how deep do we discount").

6. **HTTP request shape is `{cart, customer}`, not a full `PricingInput`.** Promotions,
   groups, `now` and the policy come from the service (repository, clock, config), which is
   the `Evaluate` use case as designed in ADR 4/ADR 7. A debug endpoint that accepts a raw
   `PricingInput` is deliberately not added: it would bypass the use case.

7. **Promotion repository for the local stack reads `deploy/promotions.json`** (groups and
   promotions in the ubiquitous-language JSON) into the existing in-memory repository.
   Postgres comes with the CRUD change.

8. **Dashboards as plain JSON** under `deploy/grafana/dashboards/`, provisioned by the file
   provider already configured. Alternative: Grafana Foundation SDK / Grafonnet to generate
   JSON. Deferred: three dashboards do not justify a generator yet; revisit when panels are
   shared across boards.

9. **Compose runs the service as a container built from the repo** (`Dockerfile` with a
   two-stage Rust build), so the smoke check exercises the real binary. The service exports
   to `http://lgtm:4317` inside the compose network.

10. **Exporter failure handling:** batch exporters with bounded queues; export errors are
    logged by the SDK and never surface to the handler. Verified by the "collector down"
    scenario in the adapter tests using an unreachable endpoint.

## Risks / Trade-offs

- [`promotion_id` cardinality grows with auto-generated coupon codes in a later iteration]
  → coupons will carry a `campaign_id`; metrics will switch to it when coupons arrive
  (noted for the coupon ADR).
- [`grafana/otel-lgtm:latest` moves under us] → pin the image tag once the dashboards are
  stable; the smoke script catches breakage.
- [Metric translation names differ between collector versions (`_total`, unit suffixes)]
  → dashboards query the translated names; the smoke script asserts the exact series name
  so a rename is caught immediately.
- [Docker is not available in CI] → the compose smoke stays a documented local check; CI
  covers the adapter crate with an in-memory exporter test instead.

## Migration Plan

No migration: new binary and new files. Rollback is removing the service container from
`deploy/compose.yaml`.

## Open Questions

- Pin `grafana/otel-lgtm` to a specific tag now or after the dashboards settle? Does not
  affect specs or tasks; default is to pin after the first green smoke run.
