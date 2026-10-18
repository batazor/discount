# Proposal

## Why

The role switch changed the menu order and the overview and nothing else: a cosmetic
layer from before sign-in existed. Now every operator's token names scopes, and the
console can follow them instead of asking.

## What Changes

- **No role switch.** The navigation, the overview and the actions follow the signed-in
  token's scopes; without sign-in everything is available, as before.
- **Navigation by scope**: campaign screens first for someone who can write the
  catalogue, support screens first otherwise; screens whose scope the token lacks are
  left out.
- **One overview**: the find box for those who can look customers or orders up, campaign
  health and the first-run checklist for those who can read, respectively write, the
  catalogue; recent lookups stay in the browser.
- **Actions follow scopes**: without `discount:catalog.write` forms are read-only and the
  create, save, archive, delete and generate controls are not shown; without
  `discount:pricing` Explain, "try it" and the redemption transitions are not offered;
  without `discount:customers` the customer screens are not offered. The service still
  decides: a direct call without the scope still shows its `403`.
- Fix: the budget form, lost when the campaign page moved to tabs, returns to the overview tab.

Outside the verified scope: everything; no API change.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `admin-ui`: role switch and overview-by-role removed; navigation, overview and actions
  by scope added.

## Impact

- `src/ui/src/model/access.ts` (new), `model/role.ts` and `pages/role.ts` (removed),
  `middleware.ts`, `layouts/Layout.astro`, overview, campaign tabs, settings, coupon,
  explain, redemption and customer pages.
