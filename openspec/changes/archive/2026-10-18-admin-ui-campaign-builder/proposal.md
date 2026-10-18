# Proposal

## Why

Creating a campaign is one long form with every engine concept on it. Marketing wants to
be led: pick what kind of campaign, say what it gives in their words, see the campaign
described back as a sentence while they type, and try it on a cart before anyone else
does. The rule types are still engine names, even with descriptions.

## What Changes

- **Wizard** at `/campaigns/new`: steps (kind, what it gives, who and when, budget and
  codes, name and review) over one form, so without scripts it is the full form; the
  plain form stays at `/campaigns/new/form`.
- **Live summary**: a sentence built from the fields ("10 % off the order for first
  orders, from 1 Nov to 30 Nov, up to 1 000.00 EUR, with 500 single-use codes"), rendered
  by the same function on the server (review step) and in the browser as the operator
  types.
- **Templates**: starting points (everyone, welcome, code-only, date-bound sale, VIP
  allowlist, win-back) that prefill the wizard through the URL, so they work without
  scripts and can be linked.
- **Rule builder** in plain words: toggles such as "first order only", "once per
  customer", "needs a code", "only allowlisted customers", "dates", "minimum order
  total", "cart tags", "customer attribute", "limit total uses", each revealing its inputs;
  the view model maps them to the contract's `Rule` variants. The raw rule editor stays on
  the campaign's rules tab for everything the builder does not cover.
- **Try it**: after creation the campaign page offers a sample cart (currency, lines,
  optional code and customer facts) and shows, through `POST /v1/evaluate`, whether this
  campaign applied, what it granted, or why it was skipped. A dry run; nothing is stored.
  Before saving nothing can be tried: the engine prices only the catalogue it holds.

Outside the verified scope: everything in this change; no API operation is added and the
console computes no eligibility or price.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `admin-ui`: wizard, summary, templates, rule builder, try-it card.

## Impact

- `src/ui/src/model`: `summary.ts`, `builder.ts`, `templates.ts` with vitest;
  `campaign.ts` creation form accepts builder fields.
- `src/ui/src/pages/campaigns/new.astro` (wizard), `new/form.astro` (plain form),
  `src/ui/src/components/campaign/*` (steps, summary, try-it), `src/ui/src/client/wizard.ts`.
