# Design

1. **Outbox, not triggers.** The event is written by the use case in its own transaction;
   nothing can be committed without its event and no event without its change. Database
   triggers would couple the schema to the export and bypass the application's vocabulary.
2. **Pull and push both**, because warehouses backfill (pull) and integrations react
   (push). Cursors are `(updated_at, id)` encoded, never offsets, so pages are stable under
   writes.
3. **Stripe-shaped signatures**: timestamp plus HMAC over `t.body` prevents replay and
   keeps receivers' verification code boilerplate.
4. **Delivery is at-least-once**; `Discount-Event-Id` lets receivers deduplicate.
5. **The dispatcher runs inside the server binary** with `FOR UPDATE SKIP LOCKED` on the
   outbox, so several replicas share the work without a broker.

## Risks / Trade-offs

- [Webhook secret storage] → stored encrypted at rest with a key from `DISCOUNT_WEBHOOK_KEK`;
  without the variable, webhooks cannot be created.
- [Outbox growth] → delivered events older than 30 days are pruned by the dispatcher.
