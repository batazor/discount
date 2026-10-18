# Design

## Context

`call()` in `src/ui/src/api/client.ts` already returns `traceId` on every `Outcome`.
Pages render errors through `FormStatus` (from `FormState`, built by `formState()` from an
`ActionOutcome`) or directly with `<ApiError status body />` when a page load fails; the
layout shows `Unreachable` from a URL string only.

## Decisions

1. **Thread the id, do not re-fetch it.** `ActionOutcome` kinds `api` and `unreachable`
   gain `traceId`; `FormState` gains `traceId` (of whichever outcome failed); `ApiError` and
   `Unreachable` take an optional `traceId` prop rendered by one `TraceRef` component.
   The layout's global `Unreachable` keeps the URL-only form: it is fed by
   `unreachableUrl()` from 15 pages, and the same unreachable call is also shown with its
   trace id by `FormStatus` where an action was involved.
2. **Explore link, not a Tempo deep link.** `traceLink(id)` builds a Grafana Explore URL with
   a `panes` payload (`datasource: tempo`, `queryType: traceql`, `query: <id>`), which opens
   the trace directly. `DISCOUNT_GRAFANA_URL` names the Grafana an operator can reach from
   the browser (the console's own API URL is container-internal), default
   `http://localhost:3000`.
3. **Plain text when the id is unknown.** Components render nothing for a missing id, so
   older callers stay valid.

## Risks / Trade-offs

- [Trace with no root span in Tempo] → expected: the console exports no spans; the view
  shows the service's spans under a missing parent, which is still the request.
