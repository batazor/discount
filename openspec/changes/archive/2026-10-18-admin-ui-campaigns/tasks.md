# Tasks

## 1. Units and campaign model (`src/ui/src/model`)

- [x] 1.1 `units.ts`: exponent table, `parseAmount`/`formatAmount`, `parsePercent`/`formatBps`, local time formatting with UTC; vitest for "Fixed amount in decimals", "Currency without decimals", "Percent to basis points", "Three-decimal currency"; verify `npm test`
- [x] 1.2 `fields.ts` money reads decimals through `units.ts`; promotion, policy, evaluate mappings and their tests updated; verify `npm test` and `astro check`
- [x] 1.3 `campaign.ts`: assemble campaigns from promotions, metadata listing and coupons; status classification with vitest for "Exhausted by budget", "Ended wins over exhausted", "No window, budget left", ending-soon; verify `npm test`

## 2. Components (`src/ui/src/components`, `src/ui/src/client`)

- [x] 2.1 `Money.astro`, `When.astro`, `Percent.astro` rendering human units with the integer and UTC in tooltips; `StatusBadge.astro` for campaign status with its meaning; verify a built page shows `5.00 EUR` with `500` in the tooltip
- [x] 2.2 `ConfirmDialog.astro` with `client/confirm.ts` (dialog with consequence text, fallback to the browser confirm) and consequence texts in the glossary; wire it to archive, delete, cancel, clear and remove; verify "Archive dialog" in the browser and that the action still submits without scripts
- [x] 2.3 `client/filter.ts` for list filter boxes and status chips updating a count; verify "Filter coupons by prefix" in the browser

## 3. Campaign screens (`src/ui/src/pages/campaigns`, `src/ui/src/actions`)

- [x] 3.1 `/campaigns` list with status, title, what it gives, when, remaining budget, codes count, filter box and status chips; `/promotions` redirects; verify "Codes of a campaign" and "Old link" against a local server
- [x] 3.2 `/campaigns/[id]` with tabs overview, rules and action, codes (coupons of the promotion plus generation for it), audience, details; hints for what is missing; archive through the dialog; verify every tab against a local server
- [x] 3.3 `/campaigns/new` and the `createCampaign` action running the documented calls in order with step reporting; verify "Full creation" and "Budget step fails" (budget with a bad currency) against a local server
- [x] 3.4 Copy-all and CSV download for generated codes; verify "CSV" by downloading from the browser

## 4. Role, overview, find, explain (`src/ui/src/pages`, `src/ui/src/layouts`)

- [x] 4.1 `/role` endpoint and cookie, role switch in the navigation, navigation order and landing per role, settings group with redirects from the old paths; verify "Support lands on find" with curl and in the browser
- [x] 4.2 Overview for marketing (live, ending soon, lowest budgets, exhausted, scheduled, first-run checklist) and for support (find box, recent lookups in local storage); verify "First run" against an empty in-memory catalogue and "Ending soon" with a fixture
- [x] 4.3 `/find` with the four lookups and links; verify "Order id" and "Nothing matches" against a local server
- [x] 4.4 `/explain` with order-id prefill from the stored redemption input, the form, pasted JSON and the result panel; link from the redemption page; `/evaluate` redirects; verify "Explain a stored order" against a local server

## 5. Spec and docs (`openspec`, `README.md`)

- [x] 5.1 README console section updated for roles, campaigns, explain and units; verify the documented commands run as written
