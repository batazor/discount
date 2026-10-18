# Tasks

## 1. Model (`src/ui/src/model`)

- [x] 1.1 `builder.ts`: conditions `cart_attr`, `cart_date`, `attr_range`, `max_order`; `builderFromRules` with leftovers; `applyBuilder` keeps advanced rules after builder rules; checks for end-before-start and min-above-max; vitest for "Round trip", "Both bounds", "End before start"; verify `npm test`
- [x] 1.2 `summary.ts` `describeConditions`; `templates.ts` `fromCampaign`; vitest for "Duplicate" fields and "Sentence of conditions"; verify `npm test`

## 2. Screens (`src/ui/src/pages/campaigns`, `src/ui/src/components/campaign`)

- [x] 2.1 `ConditionsEditor.astro` shared by the wizard and the rules tab; rules tab with prefilled conditions, advanced rules and the live sentence; verify "Round trip" against a local server
- [x] 2.2 `/campaigns/templates` gallery, `?from=` in the wizard, Duplicate on the campaign page; verify "Duplicate" against a local server
