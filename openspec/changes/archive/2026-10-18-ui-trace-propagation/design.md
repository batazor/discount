# Design

## Context

`src/ui/src/api/client.ts` wraps `openapi-fetch`; all server-side pages and actions go
through `call()`. `openapi-fetch` supports middleware (`api.use({ onRequest })`) and a
custom `fetch`. The console runs on Node (Astro SSR), so `crypto.randomBytes` is available.

## Decisions

1. **Generate ids in the console, export no spans.** The console is not instrumented with an
   OpenTelemetry SDK; it only mints a trace id and a parent span id. Tempo shows such a
   trace with a missing root span, which is acceptable: the service spans are what the
   operator needs, and the id is on the error panel. A full browser/Node OTel SDK in the
   console was rejected as disproportionate.
2. **One id per `call()`.** The id is created in `call()` and passed to the request through
   a middleware that reads it from a per-call `AsyncLocalStorage`? No: simpler, `call()`
   builds the header value and the middleware reads it from a module-level "next
   traceparent" slot set just before `fn()` runs. Because `fn()` issues exactly one request
   synchronously after the slot is set, the slot is consumed before any other call. The
   test covers two interleaved calls.
3. **`traceId` on every `Outcome` variant**, including `unreachable`, so pages can always
   print it.

## Risks / Trade-offs

- [Interleaved concurrent calls on the server] → the slot is read inside `onRequest`, which
  `openapi-fetch` invokes synchronously within the `fn()` call before any await; verified by
  a test that starts two calls without awaiting between them.
