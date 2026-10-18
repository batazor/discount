# Tasks

## 1. Console (`src/ui/`)

- [x] 1.1 Add `src/ui/src/api/trace.ts` (`GRAFANA_URL`, `traceLink(id)`) and `src/ui/src/api/trace.test.ts` (URL contains the id as a Tempo query and starts with `GRAFANA_URL`); verify `npm test`
- [x] 1.2 Carry `traceId` through `ActionOutcome` (`api`, `unreachable`) and `FormState` in `src/ui/src/actions/run.ts`; add `components/TraceRef.astro` and the optional `traceId` prop to `ApiError.astro` and `Unreachable.astro`; pass it from `FormStatus.astro` and from the six pages rendering `ApiError` directly; verify `npm run check` and `npm run build`
- [x] 1.3 Add `DISCOUNT_GRAFANA_URL: http://localhost:3000` to the `ui` service in `deploy/compose.yaml`; verify the rebuilt console shows `trace <id>` linking to Grafana on a `422` from the evaluate playground
