# Design

## Context

Everything here is presentation over calls the console already makes; the contract and
the service do not change. See proposal.md for motivation and the delta spec for the
behaviour. Both user groups exist (marketing creates and runs campaigns, some of which
are synced in from another system; support finds and explains), currencies differ in
precision, so units are converted at the edge and the integers of the contract are
always visible.

## Goals / Non-Goals

**Goals:**
- Task-shaped navigation and a campaign entity without inventing server state.
- Human units with the contract integer always one hover away; conversions pure and
  tested.
- Nothing hidden by role; the role only reorders and chooses a landing page.

**Non-Goals:**
- Server-side pagination, redemption listings, initial budget for burn rate: API work,
  separate changes.
- Authentication and per-role authorisation: there is none in the service.
- Rewriting the engine vocabulary: plain names sit next to the contract names, never
  instead of them.

## Decisions

1. **Campaign is a view, not a resource.** `model/campaign.ts` assembles `{promotion,
   metadata?, allowlist count?, coupons[]}` from the list endpoints (promotions, metadata
   listing, coupons) in one page load, and classifies the status with the precedence
   `ended > scheduled > exhausted > live` from `DateWindow` rules, `remaining_budget` and
   `MaxUsesTotal` against `uses_total`, with `now` from the server clock at render. The
   function is pure and unit-tested per scenario. Archived promotions never appear, as
   the API hides them. Alternative: a status endpoint on the service; rejected because the
   classification is presentational and would duplicate what the spec already forbids the
   console to own (prices, statuses, counters come from the service, and they still do).
2. **Units at the edge.** `model/units.ts`: an ISO 4217 exponent table (0 for JPY, KRW,
   VND, CLP, ISK, UGX...; 3 for BHD, IQD, JOD, KWD, LYD, OMR, TND; 2 otherwise),
   `parseAmount("5.00", "EUR") → 500` refusing more decimals than the exponent,
   `formatAmount(500, "EUR") → "5.00 EUR"`, `parsePercent("12.5") → 1250` (at most two
   decimals), `formatBps(1250) → "12.5 %"`. `Form.money()` reads the decimal form; the
   `<Money>` and `<When>` components render with the raw integer and UTC in `data-tip`.
   Local time: `datetime-local` inputs are interpreted in the viewer's zone, which the
   browser reports in a hidden field (`tz_offset_minutes`) filled by script; without
   scripts the form says UTC. Alternative: keep UTC everywhere; rejected, operators plan
   campaigns in local time.
3. **Creation is a sequence of documented calls with step reporting.** The create action
   runs promotion `PUT`, then budget, metadata, generation, stops at the first failure
   and returns `{saved: [...], failed: {step, status, body}}`. The page shows that
   verbatim. No rollback: a saved promotion without metadata is a valid state and the
   operator lands on its page to finish. Alternative: a composite endpoint; rejected, it
   is not in the contract and the partial state is honest.
4. **Role in a cookie, set by a GET endpoint with redirect.** `/role?set=support` sets
   `discount_role` and redirects back; works without scripts. The layout reads it to
   order the navigation and the index page to choose its content. Nothing is hidden.
5. **Find is four lookups in parallel.** `GET` redemption, customer coupons, coupon,
   promotion for the same string; render whichever answered `200` (a customer with an
   empty list counts as no match). No new endpoint; four calls per search is fine for an
   operator tool.
6. **Explain by order id prefills from `Redemption.input`.** The stored input is the
   whole `PricingInput`; the form takes its `cart` and `customer` parts and the rest
   (policy, promotions, coupons, rates) is what the service holds now, so the result is
   "what would happen today", which is what support asks. The page says so.
7. **Filters in the browser.** A small script filters rows by text and status chips and
   updates the count; the server still renders the full list, so the page works without
   scripts and the spec's "lists every record" holds. Pagination is deferred to an API
   change.
8. **Confirm dialogs with `<dialog>`.** One `ConfirmDialog` component bound to a form:
   with scripts the submit is intercepted and the dialog's confirm submits; without
   scripts the browser's `confirm()` on `onsubmit` stays. Consequence texts live in the
   glossary.
9. **Routes.** `/campaigns`, `/campaigns/new`, `/campaigns/[id]` with `?tab=`;
   `/explain`, `/find`; `/settings/policy|groups|exchange-rates|grant-rules` keep the
   existing pages under the new path; the old paths redirect with `301`.

## Risks / Trade-offs

- [Status depends on the server clock] → `now` is taken once per render and shown in the
  footer; the spec's scenarios use relative days.
- [Exponent table drifts from reality] → unknown currencies fall back to two decimals and
  show the raw integer in the tooltip; the table is one array to edit.
- [Local time without scripts] → the form states UTC; with scripts the offset is sent
  and the hint shows the zone.
- [Four calls per find] → acceptable at operator scale; the page shows which lookups
  were made.
- [Partial creation] → the step report says exactly what exists; the campaign page then
  shows what is missing (no metadata, no codes) as hints.

## Migration Plan

Additive routes with redirects from the old ones; no data or API change. Rollback is
reverting the UI commit.
