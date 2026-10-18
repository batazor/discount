# Spec Delta

## Purpose
Getting data out: incremental listings for pull, flat exports for warehouses, and signed
webhooks fed by a transactional outbox for push.

## ADDED Requirements

### Requirement: Incremental listings with cursors
`GET /v1/redemptions`, `GET /v1/coupons` and `GET /v1/grants` SHALL accept
`updated_after` (Unix seconds, default 0), `cursor` (opaque, from a previous page) and
`limit` (1–1000, default 100) and respond `{items, next_cursor}` ordered by
`(updated_at, id)` ascending; `next_cursor` is `null` on the last page. A record updated
after a page was read SHALL appear in a later page.

#### Scenario: Paging through redemptions
- **WHEN** 250 redemptions exist and the caller requests pages of 100 following
  `next_cursor`
- **THEN** three pages return 100, 100 and 50 items with no duplicates and the last
  `next_cursor` is `null`

### Requirement: Flat export
`GET /v1/exports/redemptions?updated_after=<ts>&format=ndjson|csv` SHALL stream every
matching redemption: as NDJSON one `Redemption` per line; as CSV one line per applied
promotion with columns `order_id,customer_id,status,currency,original_subtotal,
total_discount,final_total,promotion_id,discount,clamped_by,coupon_codes,source,
created_at,updated_at` (`source` from the promotion's metadata, empty when none).

#### Scenario: Warehouse load
- **WHEN** an order applied two promotions and the CSV export is requested
- **THEN** the order appears on two lines sharing `order_id` and totals, each with its own
  `promotion_id` and `discount`

### Requirement: Transactional outbox
Every state change SHALL write an event row in the same transaction as the change, with
`id` (ULID), `type`, `occurred_at`, `tenant_id` and `data` (the affected record). Types:
`redemption.reserved`, `redemption.superseded`, `redemption.confirmed`,
`redemption.cancelled`, `coupon.granted`, `coupon.imported`, `promotion.paused`,
`promotion.resumed`. `GET /v1/events?after=<id>&limit=` SHALL list them in `id` order.

#### Scenario: Reserve writes its event
- **WHEN** `Redeem` reserves order `o1`
- **THEN** an event `redemption.reserved` with `data.order_id = "o1"` exists, and if the
  reservation rolls back no event exists

### Requirement: Webhook registration
`GET|PUT|DELETE /v1/webhooks/{webhook_id}` and `GET /v1/webhooks` SHALL manage
`Webhook { webhook_id, url, events, secret, active }`: `url` must be `https`, `events` is a
list of event types or `["*"]`, `secret` is write-only and 32–128 characters, `active`
defaults to `true`.

#### Scenario: Round trip
- **WHEN** a webhook is `PUT` with a secret
- **THEN** `GET` returns it without the secret and it is listed

### Requirement: Webhook delivery
A dispatcher SHALL deliver each outbox event once per matching active webhook as
`POST <url>` with the event JSON and headers `Discount-Event-Id`, `Discount-Event-Type` and
`Discount-Signature: t=<unix>,v1=<hex hmac-sha256 of "<t>.<body>">`. A non-2xx response or
a 10 s timeout SHALL be retried with exponential backoff (1 min to 1 h) for 24 h, then
marked `failed`. `GET /v1/webhooks/{webhook_id}/deliveries?limit=` SHALL list attempts with
status code and timestamps.

#### Scenario: Signed delivery
- **WHEN** a webhook for `redemption.confirmed` is registered and an order is confirmed
- **THEN** the receiver gets one POST whose signature verifies with the secret and the raw
  body

#### Scenario: Retry
- **WHEN** the receiver answers `503` twice and then `200`
- **THEN** the deliveries log shows three attempts and the event is marked delivered
