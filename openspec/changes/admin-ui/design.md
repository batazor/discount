# Design

## Context

The service already has a complete HTTP contract (`api/openapi.yaml`, ADR 10) and a
compose stack with Swagger UI; what is missing is a human-shaped front over it. The Rust
side stays as it is: the console is a second generated client, in a second language, with
the same discipline (nothing hand-written about the wire format). See proposal.md for
motivation and `specs/admin-ui/spec.md` for the screens.

## Goals / Non-Goals

**Goals:**
- Zero duplicated semantics: every value shown is a response field; every submitted body
  is the ubiquitous-language shape, typed from the contract.
- A build that fails when the contract changes under the UI (type generation at build,
  `astro check` in CI), the way `src/client` fails the Rust build.
- Runs with one `docker compose up` next to the service; dev loop with `astro dev` against
  a local server.

**Non-Goals:**
- Authentication, roles, audit. The service has none (`security: []`); the console is
  deployed behind whatever protects the service. Recorded as a follow-up, not designed here.
- A storefront widget or a public "my coupons" page.
- Dashboards or metrics in the console: Grafana already does that.
- Bulk import/export of the catalogue.

## Decisions

1. **Astro in SSR mode with the Node adapter, not a static build.** Every page fetches
   from the service on request (server to server), so no CORS on the Rust side, no secret
   or base URL shipped to the browser, and the HTML a page shows is always current. A
   static build would need client-side fetching plus CORS and would cache stale lists.
   Alternative considered: a static Astro site behind nginx proxying `/v1`; rejected
   because it moves the API URL into an nginx config and still needs client JS for every
   list.
2. **Mutations through Astro Actions, forms work without JavaScript.** Each `PUT`,
   `POST`, `DELETE` is an action with a `zod` input schema derived from the form fields;
   the action calls the service and returns either the response body or the API error
   `{status, body}` for the page to render next to the form. Progressive enhancement:
   plain `<form method="post">` first, a small script only where the form is dynamic
   (rule and action variant pickers, cart lines in the playground).
3. **Types generated from `api/openapi.yaml` with `openapi-typescript`, requests through
   `openapi-fetch`.** Generated file `src/ui/src/api/schema.d.ts` is produced by the
   `prebuild`/`predev` script and git-ignored, like `src/client`'s output. `openapi-fetch`
   is a thin typed `fetch`, so the path, method and body of every call are checked
   against the contract at compile time, which is how the "only documented operations"
   requirement is enforced rather than tested. Alternative: `openapi-generator
   typescript-fetch` (the README's command); rejected because it needs Docker and emits a
   large class-based client; the README keeps that command for other consumers.
4. **One module of pure view-model functions, unit-tested with vitest.** Form-to-body
   mapping (variant tagging, `null` for empty optionals, integer parsing of money), body-
   to-form, and result-to-rows for the playground live in `src/ui/src/model/*.ts` with no
   Astro import, so they are testable without a browser. These functions carry the
   scenarios "Add a date window rule", "Fixed amount in minor units", "Uses are shown, not
   sent".
5. **No client-side price or status logic.** The playground posts and renders; the
   customer screen renders `CouponStatus` from the API. The temptation to "preview" a
   discount in the browser is refused by design: that would be a third, unverified
   implementation of the engine.
6. **Layout: `src/ui` as a sibling of the Rust crates.** Product code lives under `src/`
   (README layout); the Cargo workspace lists members explicitly, so the directory is
   invisible to cargo. Dockerfile in `src/ui/Dockerfile` (node:22-alpine, two stages),
   compose service `ui` with `DISCOUNT_API_URL=http://discount:8080`.
7. **Error presentation is literal.** The API's error bodies are the contract (`ErrorBody`
   in the document); the UI prints status and JSON in a `<pre>` next to the form, plus the
   `decode_error` or `error.type` as a one-line summary. No translation table to maintain.
8. **Health is the console's own.** `GET /healthz` is an Astro endpoint returning `200`
   without calling the service, so compose health checks and the smoke step distinguish
   "console down" from "service down" (the latter is the per-page notice).
9. **Styling with Tailwind CSS v4, no component library.** Utilities in the `.astro`
   templates through the `@tailwindcss/vite` plugin (the separate Astro integration is
   deprecated for v4); dark mode via `prefers-color-scheme`; a `<details>` JSON view for
   raw bodies. A component library (daisyUI, shadcn) would add a theme to maintain and a
   framework runtime; plain CSS was considered and rejected because the forms are
   repetitive and utilities keep them consistent without naming a class per field.
10. **Animation with the vanilla `motion` package, only in islands.** Pages are
    server-rendered and forms work without JavaScript, so animation applies only where a
    client script already exists: the playground result panel appearing, cart lines and
    rules being added or removed, generated codes being revealed. Page-to-page
    transitions use Astro View Transitions, no library. `motion` is imported as
    `animate`/`stagger` from `motion` (not the React bindings) inside those scripts.
    Rules: `prefers-reduced-motion` disables every animation, and an animation never
    delays data being in the DOM, so a scenario checked by reading the page is unaffected.
    Alternatives: CSS transitions only (rejected: list enter/exit needs JS anyway);
    `motion/react` with a React island (rejected: adds a framework for three effects).
    Dependency list becomes Astro, the Node adapter, `@tailwindcss/vite`, `tailwindcss`,
    `motion`, `openapi-typescript`, `openapi-fetch`, `zod`, `vitest`.

## Risks / Trade-offs

- [Contract drift between a deployed console and a deployed service] → both are built
  from the same commit in compose; CI generates types from the checked-in document, so a
  mismatch fails `astro check` before merge. At runtime the console shows the API's
  `400` verbatim, so drift is visible, not silent.
- [Forms for `oneOf` variants are the largest piece of UI code] → the variant list is
  read from the generated types once, in the view-model module, and tested there; the
  Astro components are thin over it.
- [Timestamps are integers in the contract] → the form offers a datetime input and
  converts to the integer the contract uses in the view-model; the raw integer is always
  shown alongside so an operator can check it.
- [SSR means every page load hits the service] → acceptable for an operator tool with a
  handful of users; the service's list endpoints are cheap and unauthenticated anyway.
- [No auth] → documented non-goal; the compose stack binds `:4321` on localhost only, and
  the README says to put the console behind the same gate as the service.

## Migration Plan

Additive: a new directory, a new compose service, a new CI job. Rollback is removing the
service from compose; nothing in the Rust service or the database changes.
