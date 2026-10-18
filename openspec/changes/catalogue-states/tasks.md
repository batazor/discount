# Tasks

- [ ] 1. `api/openapi.yaml`: `state` fields, `StateBody`, two routes, `409 PromotionPaused`; UL application-layer `state`; verify lint and `openspec validate catalogue-states`
- [ ] 2. Ports and stores: `set_promotion_state`, `set_coupon_state`, state columns (migration `0008_states.sql`), snapshot filters; `CouponStatus::Paused`; generation and grants refuse paused
- [ ] 3. HTTP routes; tests on both backends; contract test; smoke step (pause P3, evaluate, resume)

## Workflow follow-up

- Archive the change after review: `openspec archive catalogue-states`.
