# Tasks

## 1. Vocabulary and components (`src/ui/src/model`, `src/ui/src/components`, `src/ui/src/client`)

- [x] 1.1 `glossary.ts` with every option description and term text worded after `docs/ubiquitous-language.md`; `Help.astro` tooltip; `Field.astro` `help` and `optionHelp`; `client/help.ts` switching the shown description on change; verify a built page renders every option's text and only the chosen one unhidden
- [x] 1.2 Rule variants `CustomerAttributeIn`, `CustomerAttributeRange`, `RequiresAllowlist` in `model/promotion.ts` and `RuleRow.astro`; `customer.attributes` and `customer.allowlisted` in `model/evaluate.ts` and the playground; verify vitest round trips every variant and `astro check` passes against the current contract

## 2. Allowlist (`src/ui/src/model`, `src/ui/src/actions`, `src/ui/src/pages/promotions`)

- [x] 2.1 `model/allowlist.ts` with tests (one id per line, dedup), actions `putAllowlist` and `clearAllowlist`, allowlist card with count and the no-rule warning; verify "Replace and clear" against a local server

## 3. Help on every screen (`src/ui/src/pages`, `src/ui/src/components`)

- [x] 3.1 Option help on every select, `?` tooltips on terms and columns, tooltips on status, mode, trigger, clamp and skip badges; priority hints corrected to "higher applies first"; verify by reading the built pages and a browser check of a tooltip
