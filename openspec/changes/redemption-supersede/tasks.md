# Tasks

- [ ] 1. `api/openapi.yaml`: `409 AlreadyConfirmed`, `/v1/redemptions/{order_id}/history`, `RedemptionVersion`; UL `Redeem` entry; verify lint and `openspec validate redemption-supersede`
- [ ] 2. Ports and stores: `ReserveOutcome::Superseded`, release-then-price inside the transaction, `redemption_history` (migration `0010_redemption_history.sql`); shared scenarios on both backends incl. concurrency with a budget
- [ ] 3. HTTP route and the client; smoke step (redeem, add a code, redeem again, read history)

## Workflow follow-up

- Archive the change after review: `openspec archive redemption-supersede`.
