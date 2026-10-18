# Tasks

- [ ] 1. `api/openapi.yaml`: listing parameters, `Page`, exports, `Event`, `Webhook`, `Delivery`, routes; UL application-layer `Event`, `Outbox`, `Webhook`; verify lint and `openspec validate events-and-exports`
- [ ] 2. Outbox: `Event` shape, writer inside the reserve/transition/grant/import transactions, `GET /v1/events`; migration `0011_outbox_webhooks.sql`; tests on both backends (rollback writes no event)
- [ ] 3. Listings with cursors and flat exports; tests (paging without duplicates under concurrent writes)
- [ ] 4. Webhooks CRUD, dispatcher with backoff and `SKIP LOCKED`, signature, deliveries log; tests with a local receiver; smoke step with a receiver container

## Workflow follow-up

- Archive the change after review: `openspec archive events-and-exports`.
