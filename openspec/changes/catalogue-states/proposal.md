# Proposal

## Why

An operator can archive a promotion or delete a coupon, and neither is reversible. The
promocode service being replaced lets a campaign be switched off and on and a single code
be deactivated and reactivated; Voucherify and Talon.One expose the same pause/resume.
Without it a mistake in a live campaign means archiving it and rebuilding it.

## What Changes

- `Promotion` and `Coupon` get a catalogue-level `state`: `active | paused`, managed over
  `PUT /v1/promotions/{promotion_id}/state` and `PUT /v1/coupons/{code}/state` with
  `{state}` and returned in the catalogue bodies. Archiving stays the terminal operation.
- Snapshot semantics: paused promotions and paused coupons do not enter the pricing
  snapshot, exactly like archived promotions. The core is untouched.
- Customer coupon listing: a new status `paused` (coupon paused, or its promotion paused),
  ranked after `applied` and before `expired`.
- Grants and generation refuse paused promotions (`409 {"error": "PromotionPaused"}`).

Outside the verified scope: everything here.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `promotion-catalog`: state resource, snapshot excludes paused.
- `customer-coupons`: `paused` status.
- `coupon-lifecycle`: generation and grants refuse paused promotions.

## Impact

- `api/openapi.yaml`: `state` on `Promotion` and `Coupon` catalogue bodies (read-only
  there), schema `StateBody`, two routes.
- `src/application`: ports `set_promotion_state`, `set_coupon_state`; `CouponStatus::Paused`.
- `src/infrastructure`: `state` columns (migration `0008_states.sql`), snapshot queries
  filter on `state = 'active'`, routes, tests on both backends, smoke step.
- Admin UI follow-up (separate change by its owners): pause/resume buttons.
