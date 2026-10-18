# demo-catalogue Specification

## Purpose
A catalogue and an order history that make a freshly started stack look like a business in
flight, so the operator console, the dashboards and the API can be evaluated on realistic
data instead of three promotions and one cart.

## Requirements

### Requirement: Demo catalogue file
`ops/demo/catalogue.json` SHALL be a `CatalogFixture` (groups, promotions, coupons,
exchange rates, metadata, allowlists, grant rules, `as_of`) that covers every `Rule`
variant, both `Action` variants, both `StackingMode`s, an exclusive promotion, a promotion
with its own `RoundingPolicy`, an amount in another currency than the cart's, and campaigns
that the console classifies as `live`, `scheduled`, `exhausted` (by budget) and `ended` on
the `as_of` day. Every promotion SHALL have metadata with a title and a source; every
coupon, metadata entry, allowlist and grant rule SHALL name a promotion of the file.

#### Scenario: Loads in memory
- **WHEN** `discount-server` starts without `DATABASE_URL` and the default
  `DISCOUNT_PROMOTIONS`
- **THEN** `GET /v1/promotions`, `/v1/promotion-metadata`, `/v1/coupons`,
  `/v1/exchange-rates`, `/v1/grant-rules` and `/v1/promotions/{id}/allowlist` return the
  file's content

#### Scenario: Consistent references
- **WHEN** the file is loaded by the fixture test
- **THEN** every promotion names a group of the file and every satellite names a promotion
  of the file

### Requirement: Seed script
`ops/demo/seed.py` SHALL load the catalogue into a running service through the
documented API operations, shifting every `DateWindow`, `AttributeDateWindow` and coupon
window by `now - as_of` (midnight UTC), generate one batch of single-use codes, and then
play a fixed order history: reservations that are confirmed, left reserved or cancelled,
coupons used, grants on `cancel` and `review`, codes saved by customers, dry-run
evaluations and rejected carts. It SHALL be idempotent: every write is an upsert, order ids
are fixed, a batch is not generated when its codes exist, and a second run leaves the
same state.

#### Scenario: Statuses after seeding
- **WHEN** the script has run against the compose stack
- **THEN** the console's campaign list shows at least one campaign in each of `live`,
  `scheduled`, `exhausted` and `ended`, with one exhausted by uses (`FLASH-48H`) and one by
  budget (`CARS-SPENT`)

#### Scenario: Customer history
- **WHEN** `GET /v1/customers/bob/coupons` is requested after seeding
- **THEN** it lists `WELCOME15` as `applied` with order `DEMO-1003`, the granted apology
  code as `applied` with order `DEMO-1005`, and `EARLYBIRD` as `expired`

#### Scenario: Second run
- **WHEN** the script runs twice on the same day
- **THEN** the second run exits zero and creates no promotion, coupon, redemption or grant
  that the first did not
