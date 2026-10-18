# Proposal

## Why

Templates live only on the wizard's first step and the rule builder only creates: editing
an existing campaign still means the raw rule editor with engine names. Operators copy
campaigns that worked and tweak them, and they edit conditions far more often than they
create from scratch.

## What Changes

- **Templates gallery** at `/campaigns/templates`: every template as a card with its
  sentence, and "start from an existing campaign": `/campaigns/new?from=<id>` prefills the
  wizard from that campaign's action, conditions, cap, budget currency, title and source;
  the campaign page gets a Duplicate action.
- **Rule builder everywhere**: the campaign's rules tab shows the conditions in plain
  words, prefilled from the stored rules; rules the builder cannot express (several date
  windows, tag lists with "all" or "none", a per-customer limit other than one, a
  completed-orders limit other than zero) stay as advanced rules in the raw editor next
  to it and are kept on save. New conditions: cart attribute, cart date attribute,
  customer attribute range, maximum order total.
- **Sanity checks** in the builder: an end before its start, a minimum above the
  maximum, "needs a code" with no codes, a range with neither bound, each a field error
  or a warning before anything is sent.
- **"Who gets it"** as a sentence on the rules tab, live.

Outside the verified scope: everything in this change; no API operation is added.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `admin-ui`: templates gallery and duplication, builder on the rules tab with reverse
  mapping, new conditions, sanity checks.

## Impact

- `src/ui/src/model/builder.ts` (`builderFromRules`, new conditions, checks),
  `templates.ts` (`fromCampaign`), `summary.ts` (`describeConditions`), tests.
- `src/ui/src/pages/campaigns/templates.astro`, `new.astro` (`from`),
  `components/campaign/RulesTab.astro`, `ConditionsEditor.astro`.
