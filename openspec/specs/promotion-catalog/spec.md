# promotion-catalog Specification

## Purpose
Where the pricing policy, stacking groups, promotions and budgets live, how they are edited,
and the snapshot the pricing engine receives.

## Requirements

### Requirement: Persistent catalogue
When `DATABASE_URL` is set the service SHALL store the pricing policy, stacking groups,
promotions and budgets in Postgres, running its migrations at start-up. Without
`DATABASE_URL` it SHALL use an in-memory catalogue seeded from `DISCOUNT_PROMOTIONS`.

#### Scenario: Start with a database
- **WHEN** the service starts with `DATABASE_URL` pointing at an empty database
- **THEN** the schema is created and `GET /v1/promotions` returns an empty list

### Requirement: Catalogue HTTP API
The service SHALL expose `GET|PUT /v1/policy` (a `PricingPolicy`), `GET /v1/groups`,
`GET|PUT|DELETE /v1/groups/{group_id}` (a `StackingGroup`), `GET /v1/promotions`,
`GET|PUT|DELETE /v1/promotions/{promotion_id}` (a `Promotion` without `remaining_budget`)
and `GET|PUT /v1/promotions/{promotion_id}/budget` (`{remaining: Money | null}`). `PUT`
upserts; `DELETE` archives, so the promotion no longer enters snapshots but existing
redemptions still resolve it. Bodies use the ubiquitous-language JSON; unknown keys are
`400 {"decode_error"}`; unknown ids are `404`.

#### Scenario: Create and price
- **WHEN** a group and a 10 % promotion are `PUT` and a cart is evaluated
- **THEN** the evaluation applies the promotion

#### Scenario: Archive hides from pricing
- **WHEN** a promotion is `DELETE`d
- **THEN** it is absent from `GET /v1/promotions` and no longer applies to new evaluations

### Requirement: Snapshot semantics
The snapshot handed to pricing SHALL contain the current policy, all groups, all
non-archived promotions each with `remaining_budget` taken from the budget table (`null`
when no budget is set), all exchange rates, and the coupons whose `code` is in the cart's
`coupon_codes`. `Evaluate` reads without locking; `Redeem` reads under the budget lock.

#### Scenario: Budget flows into pricing
- **WHEN** `P1` has budget `{remaining: 250}` and would grant 300
- **THEN** evaluation shows `P1` with discount 250 and `clamped_by = Budget`

#### Scenario: Coupons and rates flow into pricing
- **WHEN** coupon `WELCOME → P1` and rate `USD → EUR 92/100` are stored and a EUR cart with
  `coupon_codes = ["WELCOME"]` is evaluated
- **THEN** `P1` (with `RequiresCoupon` and a USD fixed amount) applies with the converted
  amount
