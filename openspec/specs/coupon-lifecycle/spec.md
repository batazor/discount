# coupon-lifecycle Specification

## Purpose
How coupon codes come into existence in bulk or by event, and how usage counters make a
code single-use or a promotion capped by number of redemptions.

## Requirements

### Requirement: Usage counters are derived from redemptions
The service SHALL compute, from reserved and confirmed redemptions (cancelled ones never
count): for each coupon, `uses` = the number of redemptions whose cart presented the code and
whose result applied the coupon's promotion; for each promotion, `uses_total` = the number of
redemptions whose result applied it. `Redeem` SHALL compute them inside the reservation
transaction; `Evaluate` reads them without a lock.

#### Scenario: Single-use code
- **WHEN** coupon `ONCE` has `max_uses = 1` and order `o1` was redeemed with it
- **THEN** evaluating or redeeming another order with `ONCE` skips the promotion as
  `NotEligible` on its `RequiresCoupon` rule, and cancelling `o1` makes `ONCE` usable again

#### Scenario: Concurrent single use
- **WHEN** two orders are redeemed concurrently with the same single-use code
- **THEN** exactly one of them applies the promotion

### Requirement: Bulk generation
`POST /v1/coupons/generate` with `{promotion_id, count, prefix, starts_at, ends_at, max_uses,
customer_id}` SHALL create `count` coupons (`1 ..= 10000`) whose codes are `prefix` followed
by 8 characters from `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` (no `0/O`, `1/I`), all distinct and
not previously stored, each with the given window, `max_uses` and binding, and respond `200`
with the list of codes. An unknown `promotion_id` SHALL be `404`.

#### Scenario: Generate a batch
- **WHEN** 100 codes are generated with prefix `SUMMER-` and `max_uses = 1`
- **THEN** 100 distinct coupons starting with `SUMMER-` exist, each single-use

### Requirement: Grant rules
The catalogue SHALL store `GrantRule { rule_id, trigger, promotion_id, priority,
validity_seconds, max_uses }` with `trigger` in `cancel | review`, managed over
`GET /v1/grant-rules`, `GET|PUT|DELETE /v1/grant-rules/{rule_id}`.

#### Scenario: Rule round trip
- **WHEN** a rule is `PUT`
- **THEN** `GET` returns it and it is listed

### Requirement: Grant by event
`POST /v1/coupons/grant` with `{trigger, customer_id, order_id}` SHALL pick the rule with the
highest `priority` (ties by `rule_id`) for `trigger`, create one coupon for its promotion
bound to `customer_id` with `starts_at = now`, `ends_at = now + validity_seconds` and the
rule's `max_uses`, record the grant, and respond `200` with the coupon.

#### Scenario: Repeated grant
- **WHEN** `grant` is called again with the same `(trigger, order_id)`
- **THEN** it returns the same coupon and creates no other

#### Scenario: No rule for the trigger
- **WHEN** no grant rule has the requested `trigger`
- **THEN** the response is `404 {"error": "NotFound"}`

#### Scenario: Cancellation grants a code once
- **WHEN** `grant` is called twice for `cancel` and order `o9`
- **THEN** both responses carry the same code and one coupon exists for it
