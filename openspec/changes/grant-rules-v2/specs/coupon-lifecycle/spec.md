# Spec Delta

## MODIFIED Requirements

### Requirement: Grant rules
The catalogue SHALL store `GrantRule { rule_id, trigger, promotion_id, priority,
validity_seconds, max_uses, cooldown_seconds, conditions }` with `trigger` matching
`^[a-z][a-z0-9_.-]{0,63}$`, `cooldown_seconds ≥ 0` (default `0`) and `conditions` a list of
`{key, values}` (at most 10), managed over `GET /v1/grant-rules`,
`GET|PUT|DELETE /v1/grant-rules/{rule_id}`. A body outside these limits is `400`.

#### Scenario: Rule round trip
- **WHEN** a rule is `PUT`
- **THEN** `GET` returns it and it is listed

#### Scenario: Free trigger names
- **WHEN** a rule is `PUT` with `trigger = "birthday"`
- **THEN** it is stored; with `trigger = "Birthday!"` the response is `400`

### Requirement: Grant by event
`POST /v1/coupons/grant` with `{trigger, customer_id, order_id, attributes}` SHALL select,
among the rules for `trigger` whose every condition `{key, values}` is met by some
`attributes` entry with that `key` and a value in `values`, the one with the highest
`priority` (ties by `rule_id`); with no matching rule it SHALL respond
`404 {"error": "NotFound"}`. If the customer already received a coupon from a rule of this
trigger less than the winning rule's `cooldown_seconds` ago, that coupon SHALL be returned
(`200`) and nothing created. The winning rule's promotion SHALL be non-archived and
`active`, else `409 {"error": "PromotionPaused"}` (paused) or `404` (archived or missing).
Otherwise the service SHALL create one coupon for the promotion bound to `customer_id` with
`starts_at = now`, `ends_at = now + validity_seconds` and the rule's `max_uses`, record the
grant, and respond `200` with the coupon.

#### Scenario: Repeated grant
- **WHEN** `grant` is called again with the same `(trigger, order_id)`
- **THEN** it returns the same coupon and creates no other

#### Scenario: Conditions pick the rule
- **WHEN** rules `web` (`conditions: [{channel: ["web"]}]`, priority 5) and `any` (no
  conditions, priority 1) exist for `cancel`, and the request carries
  `attributes: [{channel: "app"}]`
- **THEN** rule `any` wins; with `channel: "web"`, rule `web` wins

#### Scenario: Cooldown
- **WHEN** the rule has `cooldown_seconds = 86400` and `c1` was granted a `cancel` coupon an
  hour ago for order `o1`
- **THEN** a grant for `c1` and order `o2` returns the coupon of `o1` and creates none

#### Scenario: No rule for the trigger
- **WHEN** no grant rule has the requested `trigger`
- **THEN** the response is `404 {"error": "NotFound"}`

#### Scenario: Cancellation grants a code once
- **WHEN** `grant` is called twice for `cancel` and order `o9`
- **THEN** both responses carry the same code and one coupon exists for it
