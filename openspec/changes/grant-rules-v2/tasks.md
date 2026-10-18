# Tasks

- [ ] 1. `api/openapi.yaml`: `GrantRule.cooldown_seconds/conditions`, trigger pattern, `GrantRequest.attributes`, `409 PromotionPaused`; UL `GrantRule`; verify lint and `openspec validate grant-rules-v2`
- [ ] 2. Application: `GrantTrigger` newtype with validation, condition matching, cooldown through `recent_grant(trigger, customer_id, since)`; liveness check
- [ ] 3. Stores: `grants.customer_id`, index (migration `0009_grants_customer.sql`); tests on both backends; contract test; smoke step with two rules and a condition

## Workflow follow-up

- Archive the change after review: `openspec archive grant-rules-v2`.
