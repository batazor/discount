# Spec Delta

## ADDED Requirements

### Requirement: Import coupons from CSV
`POST /v1/coupons/import` with a `text/csv` body whose header is
`code,promotion_id,customer_id,starts_at,ends_at,max_uses` (columns in any order) and at
most 10000 data rows SHALL store one coupon per valid row, generating the code from
`?prefix=` (default empty) when the `code` cell is empty, and respond
`200 {created: <n>, skipped: [{row, reason}]}` in row order. Storing is atomic per row, not
per file.

#### Scenario: Personal codes from a list
- **WHEN** a file with 3 rows names customers `c1`, `c2`, `c3` for `P1` with
  `max_uses = 1` and empty codes, and `?prefix=VIP-`
- **THEN** three coupons `VIP-…` bound to those customers exist and the report says
  `created: 3`

### Requirement: Import rows are rejected or skipped
A malformed row (unparsable integer, unknown or missing column) SHALL reject the whole
request with `400` and store nothing. A row whose `promotion_id` is unknown, archived or
paused SHALL be skipped with reason `promotion_unavailable`; a row whose code already exists
SHALL be skipped with reason `code_exists`; the other rows are stored.

#### Scenario: Existing code is skipped
- **WHEN** a row names a code that is already stored
- **THEN** the report lists that row with `code_exists` and the other rows are created

#### Scenario: Malformed file
- **WHEN** a row has `max_uses = "many"`
- **THEN** the response is `400` and no coupon of the file is stored

### Requirement: Export coupons
`GET /v1/coupons/export?promotion_id=<id>&format=csv|ndjson` SHALL stream every coupon of
the promotion with its derived `uses` (`text/csv` with the import header plus `uses` and
`state`, or one JSON `Coupon` per line), ordered by `code`; an unknown promotion is `404`.
`GET /v1/coupons?promotion_id=<id>` SHALL filter the listing.

#### Scenario: Export for a mailing
- **WHEN** `P1` has 100 coupons and the export is requested as CSV
- **THEN** the body has 101 lines and a round trip through the import creates nothing
  (`created: 0`, 100 `code_exists`)
