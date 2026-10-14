# Spec Delta

## MODIFIED Requirements

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
