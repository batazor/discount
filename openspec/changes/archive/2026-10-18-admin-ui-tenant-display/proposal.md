# Proposal

## Why

The service partitions everything by tenant and the token names it, but the console only
whispers it in the navigation footer. An operator in `hotels` should see at a glance which
tenant every list and campaign belongs to, and an administrator whose token names no
tenant should know they are looking at `default`.

## What Changes

- The current tenant is shown at the top of the navigation next to the role switch, in
  the overview heading, and in the descriptions of the campaign, coupon and customer
  screens. Without sign-in it reads `local`; a signed-in token without a tenant claim
  reads `default`, with a note that the console works in one tenant and switching is not
  offered.

Outside the verified scope: everything; no API change, no `X-Tenant` header.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `admin-ui`: tenant visible.

## Impact

- `src/ui/src/layouts/Layout.astro`, `src/ui/src/model/tenant.ts`, overview and list pages.
