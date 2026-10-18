# Design

## Context

See proposal.md, Why. The HTTP adapter wraps the router in `tower_http::trace::TraceLayer`
(`src/infrastructure/src/http.rs`), naming spans from `req.uri().path()`; the telemetry module
(`src/infrastructure/src/telemetry.rs`) builds the OTLP pipelines and the `tracing`
subscriber with `tracing-opentelemetry`. No propagator is installed. Tests already drive the
router with `tower::ServiceExt::oneshot` and an in-memory span exporter
(`src/infrastructure/tests/telemetry.rs`), so every new scenario is testable without the
Docker stack.

## Goals / Non-Goals

Goals: stable, low-cardinality span names; searchable failures; no probe noise; traces that
join a caller's. Non-goals: database spans, sampling, span events for pricing decisions,
changes to metrics or to the `discount.evaluate` span, outgoing propagation (the service
makes no outbound calls).

## Decisions

1. **Route template from `MatchedPath`, read inside `make_span_with`.** axum 0.8 inserts
   `MatchedPath` into request extensions before the layer runs when the layer is applied
   with `Router::layer` after the routes, which is the current order. `req.extensions()
   .get::<MatchedPath>()` gives `/v1/redemptions/{order_id}/confirm`. Alternative considered:
   `tower_http`'s `DefaultMakeSpan`, which cannot see the template; a custom middleware
   per route, which duplicates work. The unmatched case (404 before routing) yields only
   the method, matching the semantic conventions' guidance.

2. **Skip `/healthz` by not creating a span, not by filtering at the collector.** Returning
   `tracing::Span::none()` from `make_span_with` when the matched path is `/healthz` keeps
   the behaviour self-contained and visible in tests. The collector-side alternative
   (`filter` processor in otel-lgtm) would need a custom collector config and would still
   cost export bandwidth. The `tracing` subscriber's HTTP log line also disappears with the
   span, which is what we want.

3. **Status code via `on_response`, status `Error` for 5xx only.** `TraceLayer::on_response`
   receives the response and the span; record `http.response.status_code` as `i64` and call
   `OpenTelemetrySpanExt::set_status(Status::error(..))` when `status.is_server_error()`.
   The attribute is declared `Empty` in `make_span_with` so `record` works. Marking 4xx as
   errors was rejected: `422` is the normal outcome of an invalid cart and would flood
   error searches; the child span's `discount.error_type` already names the cause.

4. **Propagation with `TraceContextPropagator` plus explicit extraction.** Install
   `global::set_text_map_propagator(TraceContextPropagator::new())` in `telemetry::init`.
   `tracing-opentelemetry` does not read headers by itself, so `make_span_with` extracts
   the remote context with `global::get_text_map_propagator(|p| p.extract(&HeaderExtractor(
   req.headers())))` and calls `span.set_parent(cx)` before returning. The extractor is a
   small local type over `HeaderMap` (five lines) rather than a new dependency on
   `opentelemetry-http`. Alternative: `axum-tracing-opentelemetry`, rejected as a new
   crate for a feature already covered by the installed ones.

5. **Vocabulary.** `http.route` and `http.response.status_code` join the Telemetry
   vocabulary in `docs/ubiquitous-language.md` and the `VOCABULARY` constant list so the
   existing agreement test covers them.

## Risks / Trade-offs

- [`MatchedPath` absent for a nested or fallback route] → covered by a test for an unknown
  path; the name degrades to the method, nothing panics.
- [A caller sends a malformed `traceparent`] → the propagator yields an invalid context and
  `set_parent` is a no-op; the request still gets a fresh trace. Test with a garbage header.
- [Dashboards or smoke depend on the old names] → inspected: dashboards query metrics and
  `discount.evaluate`; smoke searches by `service.name`. The smoke script gains, not loses,
  assertions.
- [Health-check spans were used as a liveness signal in Tempo] → metrics and the Docker
  health status remain; nothing in the repo reads those traces.

## Migration Plan

Rebuild and restart the `discount` container (`docker compose -f deploy/compose.yaml up -d
--build discount`); run `deploy/smoke.sh`. Rollback is redeploying the previous image;
no data or schema is involved.
