# Tasks

## 1. Client (`src/ui/`)

- [x] 1.1 In `src/ui/src/api/client.ts` add `newTraceparent()` (crypto random ids), an `openapi-fetch` middleware setting the `traceparent` header, and `traceId` on every `Outcome` variant produced by `call()`; verify `npm run check` in `src/ui` passes
- [x] 1.2 Add `src/ui/src/api/client.test.ts` (vitest, mocked `fetch`): header format `00-[0-9a-f]{32}-[0-9a-f]{16}-01`, `traceId` equals the header's trace id on ok, 422 and unreachable outcomes, two interleaved calls get distinct ids; verify `npm test` in `src/ui`
