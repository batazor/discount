# Proposal

## Why

Every console call now carries a W3C trace id (change `ui-trace-propagation`) and the
service's spans join that trace in Tempo, but the operator never sees the id: an error
panel shows the status and body, and finding the matching trace means guessing by time.

## What Changes

- Every API error panel and every "service unreachable" panel of the console shows
  `trace <id>` with the id linked to Grafana Explore (Tempo) at `DISCOUNT_GRAFANA_URL`
  (default `http://localhost:3000`).
- Action outcomes and form state carry the trace id through to the components.

Outside the verified scope: everything here; `src/ui` only.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `admin-ui`: "API errors are shown verbatim" gains the trace id and link.

## Impact

- `src/ui/src/api/trace.ts` (new, with test), `src/ui/src/actions/run.ts`,
  `src/ui/src/components/{ApiError,Unreachable,FormStatus,TraceRef}.astro`, the six pages
  that render `ApiError` directly; `deploy/compose.yaml` gets `DISCOUNT_GRAFANA_URL` for
  the `ui` service.
