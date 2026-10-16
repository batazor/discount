# Spec Delta

## MODIFIED Requirements

### Requirement: Snapshot semantics
The snapshot handed to pricing SHALL contain the current policy, all groups, all
non-archived promotions each with `remaining_budget` taken from the budget table (`null`
when no budget is set) and `uses_total` counted from reserved and confirmed redemptions, all
exchange rates, and the coupons whose `code` is in the cart's `coupon_codes` with their
`uses` counted the same way. `Evaluate` reads without locking; `Redeem` reads under the
budget lock.

#### Scenario: Budget flows into pricing
- **WHEN** `P1` has budget `{remaining: 250}` and would grant 300
- **THEN** evaluation shows `P1` with discount 250 and `clamped_by = Budget`

#### Scenario: Coupons and rates flow into pricing
- **WHEN** coupon `WELCOME → P1` and rate `USD → EUR 92/100` are stored and a EUR cart with
  `coupon_codes = ["WELCOME"]` is evaluated
- **THEN** `P1` (with `RequiresCoupon` and a USD fixed amount) applies with the converted
  amount

#### Scenario: Counters flow into pricing
- **WHEN** coupon `ONCE` (`max_uses = 1`) was used by one reserved redemption
- **THEN** the snapshot shows `ONCE` with `uses = 1` and its promotion with `uses_total = 1`

## ADDED Requirements

### Requirement: Stored coupons carry their limits
`PUT /v1/coupons/{code}` SHALL accept `max_uses` and ignore `uses` (it is derived); `GET`
SHALL return `uses` as counted.

#### Scenario: Uses are read-only
- **WHEN** a coupon is `PUT` with `uses = 5` and nobody used it
- **THEN** `GET` returns `uses = 0`
