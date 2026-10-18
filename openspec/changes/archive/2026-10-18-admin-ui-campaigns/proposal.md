# Proposal

## Why

The console mirrors the API: one screen per resource, engine units on every field, no
notion of what a campaign is or whether it is running. Operators think in tasks: launch
a campaign, see which ones are live or bleeding budget, explain why a customer did not
get a discount. Two kinds of people use it, marketing (creates and runs campaigns, also
ones synced in from another system) and support (finds a customer or an order and
explains), and neither is served by a resource browser.

## What Changes

- **Campaign** as the console's unit: a promotion together with its budget, metadata,
  allowlist and coupons, with a display status (`scheduled`, `live`, `exhausted`,
  `ended`) classified from fields the API returns. Campaign screens replace the
  promotion screens; `/promotions/*` redirects to `/campaigns/*`.
- **One flow to create a campaign**: title and source, what it gives (entered as
  `10 %` or `5.00 EUR`), who and when (rules with plain names), how much to spend, whether
  codes are needed. The group is picked automatically when only one exists. Created with
  the documented operations in sequence; a failing step is reported by name with what was
  already saved.
- **Human units everywhere**: money as decimals by the currency's exponent (ISO 4217,
  several currencies), percentages instead of basis points, local time with UTC in a
  tooltip. The integer the contract receives is always shown next to the input.
  **BREAKING** for the previous spec: money fields no longer take minor units.
- **Role switch** (marketing, support) in the navigation, remembered in a cookie: it
  changes the navigation order, the overview and the landing page, never what is
  reachable.
- **Overview by role**: marketing sees campaign health (live, ending within seven days,
  lowest remaining budgets, exhausted, scheduled) and a first-run checklist; support sees
  a find box and recent lookups kept in the browser.
- **Find**: one box for a customer id, an order id, a coupon code or a campaign id.
- **Explain** replaces the playground as the entry point: an order id loads the stored
  redemption input; the form and pasted JSON stay. Redemption pages link to it.
- **Lists** get a filter box and status chips, applied in the browser; coupon
  generation gets copy-all and a CSV download.
- **Destructive actions** (archive, delete, cancel, clear) open a dialog that states the
  consequences before submitting.
- Settings (policy, groups, exchange rates, grant rules) move under one navigation group.

Outside the verified scope: everything in this change. No API operation is added;
every call is one that already exists. A campaign status is a display classification of
returned fields, never a pricing result; prices, coupon statuses and counters still come
only from the service.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `admin-ui`: campaign screens and status, the creation flow, human units (money,
  percent, local time), the role switch, overview by role, find, explain by order,
  list filters, confirm dialogs, code export.

## Impact

- `src/ui/src/model`: `units.ts` (exponents, money and percent parsing and formatting),
  `campaign.ts` (status, creation steps), `fields.ts` (decimal money), all with vitest.
- `src/ui/src/pages`: `campaigns/*`, `explain.astro`, `find.astro`, `settings` group,
  redirects from the old routes; `layouts/Layout.astro` for the role switch;
  `components`: `ConfirmDialog`, `StatusBadge`, `Money`, `When`.
- `openspec/specs/admin-ui`: money and playground requirements modified, new ones added.
- No change to `api/`, Rust crates, `deploy/` or CI.
