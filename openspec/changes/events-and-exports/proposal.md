# Proposal

## Why

Finance and marketing need every redemption in the warehouse, and other systems want to
know when a code was granted or an order confirmed. The promocode service being replaced
solved this with database triggers that stamp row versions for a replication job. We do it
at the API: incremental listing with cursors for pull, and signed webhooks fed by an outbox
for push, the way Stripe and Shopify do.

## What Changes

- New capability `events-and-exports`:
  - `GET /v1/redemptions?updated_after=<ts>&cursor=<c>&limit=<n>` and the same for
    `/v1/coupons` and `/v1/grants`: stable cursor pagination ordered by `(updated_at, id)`.
  - `GET /v1/exports/redemptions?updated_after=&format=ndjson|csv`: a flat row per
    redemption with one line per applied promotion in the CSV form, for warehouse loads.
  - Outbox: every state change writes an event row in the same transaction
    (`redemption.reserved|superseded|confirmed|cancelled`, `coupon.granted`,
    `coupon.imported`, `promotion.paused|resumed`); a dispatcher delivers to registered
    webhooks with `Discount-Signature: t=<ts>,v1=<hmac-sha256>`, retries with exponential
    backoff for 24 h, and `GET /v1/webhooks/{id}/deliveries` shows the log.
  - `GET|PUT|DELETE /v1/webhooks/{webhook_id}` with `{url, events, secret, active}`.

Outside the verified scope: everything here.

## Capabilities

### New Capabilities
- `events-and-exports`: incremental listings, flat exports, outbox and webhooks.

### Modified Capabilities
_None._ (New routes only; existing requirements are unchanged.)

## Impact

- `api/openapi.yaml`: listing parameters and `Page` envelope, export operations,
  `Webhook`, `Delivery`, `Event` schemas and routes.
- `src/application`: `EventPublisher` becomes the outbox writer; `Event` gains a stable
  JSON shape with `id`, `type`, `occurred_at`, `tenant_id`, `data`.
- `src/infrastructure`: tables `outbox`, `webhooks`, `deliveries` (migration
  `0011_outbox_webhooks.sql`), a dispatcher task in the server binary, `updated_at` indexes;
  tests with a local receiver; smoke step with a receiver container.
