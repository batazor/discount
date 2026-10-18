# Proposal

## Why

The service now honours W3C `traceparent` (change `http-trace-hygiene`), but the operator
console (`src/ui`) sends none, so a failed action in the console cannot be found in Tempo
and the console's own round trip never joins the service trace.

## What Changes

- Every API call the console makes carries a freshly generated W3C `traceparent`
  (random 32-hex trace id, random 16-hex parent span id, flags `01`).
- Every `Outcome` of the typed client exposes `traceId`, so pages can print "trace
  `<id>`" next to an API error for lookup in Tempo. Rendering the id is left to the pages
  of the `admin-ui` change, which owns those components.

Outside the verified scope: everything here; only `src/ui/src/api/client.ts` and its test.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `telemetry`: one added requirement, "Console calls carry trace context".

## Impact

- `src/ui/src/api/client.ts`: an `openapi-fetch` middleware adding the header and recording
  the id on the outcome; `src/ui/src/api/client.test.ts` (vitest).
