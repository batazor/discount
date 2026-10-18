# Proposal

## Why

Marketing hands over a spreadsheet: these customers get a personal code each, with these
limits. The promocode service being replaced runs that as an xlsx job with a log file. Today
the only ways in are one `generate` call per customer or a `PUT` per code, and there is no
way to get a campaign's codes back out for a mailing. Voucherify and Talon.One both offer CSV
import and export of codes.

## What Changes

- `POST /v1/coupons/import` accepts `text/csv` (header
  `code,promotion_id,customer_id,starts_at,ends_at,max_uses`; `code` may be empty, then one
  is generated with the optional `?prefix=`), up to 10000 rows, validates every row, stores
  the valid ones and responds `200` with a per-row report `{created, skipped: [{row,
  reason}]}`. A row whose code already exists is skipped, not an error.
- `GET /v1/coupons/export?promotion_id=&format=csv|ndjson` streams a promotion's coupons
  with their derived `uses`; `GET /v1/coupons?promotion_id=` filters the listing.

Outside the verified scope: everything here.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `coupon-lifecycle`: import and export.

## Impact

- `api/openapi.yaml`: two operations with `text/csv` and `application/x-ndjson` bodies,
  schema `ImportReport`, `promotion_id` filter.
- `src/application/coupons.rs`: `ImportCoupons` use case with row validation; `src/infrastructure`:
  routes with body size limit, streaming export; tests on both backends; smoke step.
