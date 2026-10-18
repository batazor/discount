# Tasks

## 1. Scaffold (`src/ui`)

- [x] 1.1 Create the Astro project in `src/ui` (Node adapter, `output: 'server'`, port `4321`), with `tailwindcss` + `@tailwindcss/vite`, `motion`, `openapi-typescript`, `openapi-fetch`, `zod`, `vitest` as dependencies and a `prebuild`/`predev` script generating `src/api/schema.d.ts` from `../../api/openapi.yaml`; git-ignore the generated file; verify `npm run build` succeeds from a clean checkout
- [ ] 1.2 Add `src/ui/src/api/client.ts` (typed `openapi-fetch` client reading `DISCOUNT_API_URL`, default `http://localhost:8080`) and the `GET /healthz` endpoint; verify `curl localhost:4321/healthz` is `200` with the service stopped and the index page shows the "service unreachable" notice naming the URL
- [ ] 1.3 Add the layout (navigation, Tailwind base styles, dark mode via `prefers-color-scheme`, Astro View Transitions) and the shared `ApiError` component printing status plus verbatim body; verify `astro check` passes and the built CSS contains only used utilities

## 2. View model (`src/ui/src/model`)

- [ ] 2.1 Implement form↔body mapping for `Promotion` (every `Rule` and `Action` variant, `null` optionals, money as integer minor units + currency, timestamps as integers) with vitest cases for "Add a date window rule" and "Fixed amount in minor units"; verify `npm test` passes
- [ ] 2.2 Implement form↔body mapping for `PricingPolicy`, `StackingGroup`, `Coupon`, `GrantRule`, `ExchangeRate`, `GenerateCouponsRequest`, `GrantRequest`, `EvaluateRequest` and result-to-rows for `PricingResult` (applied, skipped with reason, clamps, lines, allocations); verify vitest covers "Uses are shown, not sent" and a round trip of every shape

## 3. Catalogue screens (`src/ui/src/pages`, `src/ui/src/actions`)

- [ ] 3.1 Policy page with `PUT /v1/policy` action; verify a manual edit of rounding against a local server round-trips and a bad body shows the `400` verbatim
- [ ] 3.2 Groups list, group form (`PUT`, `DELETE`); verify create, edit and delete against a local server
- [ ] 3.3 Promotions list with `uses_total` and `remaining_budget` read-only, promotion form with structural rule and action editors (small client script for variant pickers; rule rows enter/exit with `motion`, disabled under `prefers-reduced-motion`), delete with confirmation, and a separate budget form mapped onto `PUT /v1/promotions/{id}/budget` with an explicit "no budget" state; verify scenarios "Create a promotion", "Archive a promotion", "Budget set separately" against a local server
- [ ] 3.4 Coupons list with `uses` read-only, coupon form (`PUT`, `DELETE`), generation form revealing the returned codes once with a staggered `motion` animation that never delays them being in the DOM; verify "Generate a batch" against a local server
- [ ] 3.5 Grant rules list and form (`PUT`, `DELETE`), "grant now" form mapped onto `POST /v1/coupons/grant`; verify a grant for trigger `cancel` creates a code visible in the coupons list
- [ ] 3.6 Exchange rates list and form (`PUT`, `DELETE /v1/exchange-rates/{from}/{to}`); verify a rate round-trips against a local server

## 4. Lookups and playground (`src/ui/src/pages`, `src/ui/src/actions`)

- [ ] 4.1 Redemption lookup by `order_id` rendering the stored `Redemption` with confirm and cancel actions and verbatim `409`; verify "Confirm from the screen" and "Refused transition" against a local server
- [ ] 4.2 Customer lookup by `customer_id` rendering `CustomerCoupon`s with status and a link from `order_id` to the redemption lookup; verify "Applied coupon links to its order" after running `deploy/smoke.sh`
- [ ] 4.3 Evaluate playground: form (cart lines, currency, tags, attributes, coupon codes, customer) or pasted JSON, `POST /v1/evaluate` only, result panel from the view model animated in with `motion` (data present in the DOM before the animation starts), `422` shown verbatim; verify "Explain a skipped promotion", "Pricing error shown verbatim" and that `GET /v1/redemptions` finds nothing for a playground run

## 5. Packaging, stack and CI (`src/ui`, `deploy`, `.github/workflows`)

- [ ] 5.1 `src/ui/Dockerfile` (node:22-alpine, build stage generating types, runtime stage running the Node adapter) and compose service `ui` on `:4321` with `DISCOUNT_API_URL=http://discount:8080`, a health check on `/healthz` and `depends_on: discount`; verify `docker compose -f deploy/compose.yaml up -d --build` brings all three health endpoints to `200`
- [ ] 5.2 Extend `deploy/smoke.sh`: console `/healthz` is `200` and the promotions page body contains `P1`; verify `deploy/smoke.sh` passes on the fresh stack
- [ ] 5.3 Add a `ui` job to `.github/workflows/ci.yml` (`npm ci`, type generation, `astro check`, `vitest`, `astro build` in `src/ui`); verify the job passes on the pull request

## 6. Documentation (`README.md`, `docs/adr`)

- [ ] 6.1 README: `src/ui` in the layout, how to run the console locally (`npm run dev` against `localhost:8080`) and in compose, the no-auth note; ADR 10 edited in place to name the console as a second generated-client consumer; verify the documented commands run as written

## Workflow follow-up

- Archive the change after review: `openspec archive admin-ui`.
