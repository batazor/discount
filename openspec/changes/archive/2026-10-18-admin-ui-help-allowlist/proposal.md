# Proposal

## Why

Operators reading the console had to know the engine's vocabulary to pick a stacking
mode, a rounding policy or a rule: the screens named the options and explained nothing.
Meanwhile the contract gained promotion allowlists and three rules
(`CustomerAttributeIn`, `CustomerAttributeRange`, `RequiresAllowlist`) plus customer
attributes and `allowlisted` on the customer context, which the console did not offer.

## What Changes

- Every option of every select (rounding, stacking, group mode, rule type, action type,
  grant trigger) shows a one-line description of what it does, worded after
  `docs/ubiquitous-language.md`; the chosen option's description is shown under the
  select and follows the choice without a reload.
- Terms that are not options (priority, exclusive, cap, budget, order cap, uses, coupon
  binding and limits, grant validity, allocation) get a `?` tooltip next to their label or
  column; status badges (coupon status, redemption status, clamp and skip reasons, group
  mode, grant trigger) carry their meaning as a tooltip.
- The promotion form offers the three new rule variants; the playground takes customer
  attributes and allowlisted promotions.
- The promotion screen gets an allowlist card (`GET|PUT|DELETE
  /v1/promotions/{promotion_id}/allowlist`), with a warning when the list has no
  `RequiresAllowlist` rule to read it.
- Fix: the priority hints said "lower applies first"; the vocabulary says higher.

Outside the verified scope: everything in this change; no API operation is added.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `admin-ui`: help texts as a requirement, allowlist screen, and the customer context
  fields in the playground.

## Impact

- `src/ui/src/model/glossary.ts` (all help texts), `Help.astro`, `client/help.ts`,
  `Field.astro` (`help`, `optionHelp`), `model/allowlist.ts` with tests, two actions,
  every page. No change to `api/`, `src/` Rust crates or `deploy/`.
