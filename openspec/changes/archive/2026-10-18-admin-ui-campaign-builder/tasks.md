# Tasks

## 1. Model (`src/ui/src/model`)

- [x] 1.1 `builder.ts`: builder field names and `rulesFromBuilder` producing `rules.N.*` fields; `campaignDraftFromForm` applies it when `builder=1`; vitest for "First order and dates" and "Needs a code generates codes"; verify `npm test`
- [x] 1.2 `summary.ts`: `summarize(fields)` sentence from action, builder conditions, cap, budget, codes; vitest for "Sentence"; verify `npm test`
- [x] 1.3 `templates.ts`: templates as field maps with title and description; vitest that each template produces a valid draft; verify `npm test`

## 2. Wizard (`src/ui/src/pages/campaigns`, `src/ui/src/components/campaign`, `src/ui/src/client`)

- [x] 2.1 Move the plain form to `/campaigns/new/form`; `/campaigns/new` renders the stepped form with sections kind (templates), what it gives, who and when (builder), budget and codes, name and review; `client/wizard.ts` for steps and the live summary; verify "Steps keep values", "Without scripts" (curl shows every section) and "Welcome template" against a local server
- [x] 2.2 Creation through the wizard end to end with builder rules and codes; verify "First order and dates" and "Needs a code generates codes" against a local server

## 3. Try it (`src/ui/src/components/campaign`)

- [x] 3.1 Try-it card on the overview tab: sample cart form posting through the evaluate action, focused verdict for this campaign with the rule's plain name, full result on request; verify "Applied" and "Skipped with the reason" against a local server
