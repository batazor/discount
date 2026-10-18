# Proposal

## Why

Everything the service stores (policy, groups, promotions, budgets, coupons, grant rules,
exchange rates, redemptions) is editable only through `curl` or Swagger UI, and a pricing
question ("why did `P2` not apply to this cart?") takes a hand-written JSON body. The
promocode service being replaced has a back office; people who run campaigns need one
here before the switch, and the `PricingResult` with its skipped promotions and clamp
reasons (ADR 9) deserves a screen that shows them.

## What Changes

- New Astro application `src/ui` (server-rendered, Node adapter): an admin console over
  the existing HTTP API, nothing else. It is a client of `api/openapi.yaml` exactly as
  `src/client` is (ADR 10): its TypeScript types are generated from the document at build
  time and never written by hand.
- Screens: policy; stacking groups; promotions with budget and the derived `uses_total`;
  coupons with generation and the derived `uses`; grant rules; exchange rates; a
  redemption lookup by `order_id` with confirm and cancel; a customer lookup showing
  `CustomerCoupon`s with status; an evaluate playground that posts a cart and renders the
  `PricingResult` (applied, skipped with `SkipReason`, clamps with `ClampReason`, per-line
  allocation) or the structured `PricingError`.
- Promotion metadata (`title`, `description`, `source`, `labels`, change `promotion-metadata`)
  is edited in its own card on the promotion screen through the metadata resource; titles
  appear in the promotions list and the customer lookup, which both offer the API's
  `source` filter.
- Editing stays faithful to the contract: forms submit the ubiquitous-language JSON,
  derived fields are shown read-only, the API's `400`/`404`/`409`/`422` bodies are shown
  verbatim next to the form. The UI never computes a price or a status itself.
- `deploy/compose.yaml` gains a `ui` service on `:4321`; `deploy/smoke.sh` checks it
  answers; CI builds and tests the UI.
- Assumption recorded here: the UI is for operators of the service, not for shoppers.
  The storefront's "my coupons" screen remains an external consumer of the API.

Outside the verified scope: everything in this change. The pricing engine (ubiquitous
language → Lean → Rust → DRT) is untouched: no new domain type, rule, error or API
operation. The UI only calls operations that already exist in `api/openapi.yaml`.

## Capabilities

### New Capabilities
- `admin-ui`: the operator console: which screens exist, how they map onto the API,
  how errors and derived fields are presented, how it is configured and run.

### Modified Capabilities
- `grafana-dashboards`: the local stack also starts the UI, and the smoke script checks
  it is up.

## Impact

- `src/ui`: new Astro project (`package.json`, `astro.config.mjs`, pages, actions, a
  generated `api.d.ts`); its own `npm test` (vitest on the pure view-model functions).
- `api/openapi.yaml`: unchanged; it becomes the build input of a second generated client.
- `deploy/compose.yaml`: service `ui` built from `src/ui/Dockerfile`, `DISCOUNT_API_URL`
  pointing at `discount:8080`; `deploy/smoke.sh`: one check on the UI.
- `.github/workflows/ci.yml`: a `ui` job (`npm ci`, generate types, `astro check`,
  `vitest`, `astro build`).
- `README.md`: layout line for `src/ui`, how to run it; `docs/adr`: ADR 10 gains a line
  that the UI is a generated-client consumer (edited in place).
- `src/`, `verification/`: no Rust or Lean change. The Cargo workspace lists members
  explicitly, so a non-Rust directory under `src/` is harmless.
