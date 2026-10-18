# Spec Delta

## ADDED Requirements

### Requirement: Promotion and coupon states
A non-archived promotion and a coupon SHALL each have a `CatalogState` in `active | paused`,
`active` by default, read and set over `GET|PUT /v1/promotions/{promotion_id}/state` and
`GET|PUT /v1/coupons/{code}/state` with body `{state}`. The promotion and coupon bodies are
the pricing engine's types and stay unchanged; `PUT` of a body never changes the state.
Archiving is unchanged and terminal.

#### Scenario: Status codes
- **WHEN** a state is `PUT` for an unknown or archived promotion, or an unknown coupon
- **THEN** the response is `404`; otherwise `200` with the stored state

### Requirement: Listings show both states
`GET /v1/promotions` and `GET /v1/coupons` SHALL list active and paused records and accept
`?state=active|paused` to narrow the listing.

#### Scenario: Filter by state
- **WHEN** `P1` is paused and `P2` is active
- **THEN** `GET /v1/promotions` lists both, `?state=paused` lists `P1` only

#### Scenario: Pause and resume
- **WHEN** `P1` is set to `paused` and later back to `active`
- **THEN** evaluations in between skip `P1` entirely and afterwards apply it again

#### Scenario: Paused coupon
- **WHEN** coupon `WELCOME` is `paused` and a cart presents it
- **THEN** its promotion is skipped as `NotEligible` on `RequiresCoupon`

## MODIFIED Requirements

### Requirement: Snapshot semantics
The snapshot handed to pricing SHALL contain the current policy, all groups, all
non-archived promotions in state `active`, each with `remaining_budget` taken from the
budget table (`null` when no budget is set) and `uses_total` counted from reserved and
confirmed redemptions, all exchange rates, and the coupons in state `active` whose `code` is
in the cart's `coupon_codes` with their `uses` counted the same way. Paused promotions and
coupons are absent from the snapshot and therefore from `applied` and `skipped`. `Evaluate`
reads without locking; `Redeem` reads under the budget lock.

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

#### Scenario: Paused promotion leaves the snapshot
- **WHEN** `P1` is `paused`
- **THEN** an evaluation lists `P1` neither in `applied` nor in `skipped`
