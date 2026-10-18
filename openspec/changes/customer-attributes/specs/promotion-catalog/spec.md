# Spec Delta

## ADDED Requirements

### Requirement: Allowlists
The catalogue SHALL store, per promotion, a set of customer ids over
`GET|PUT|DELETE /v1/promotions/{promotion_id}/allowlist` (`{customer_ids}`, sorted; `PUT`
replaces the whole set, at most 100000 ids) and
`GET|PUT|DELETE /v1/promotions/{promotion_id}/allowlist/customers/{customer_id}` (membership,
add, remove). Pricing never reads the set: `Evaluate` and `Redeem` put the ids of the
promotions whose allowlist contains `customer.customer_id` into `customer.allowlisted`.

#### Scenario: Status codes
- **WHEN** the promotion is unknown or archived, or a membership check misses
- **THEN** the response is `404`; membership, add and remove answer `204` otherwise

#### Scenario: Allowlist gates a promotion
- **WHEN** `P1` carries `RequiresAllowlist`, its allowlist is `["c1"]`, and carts of `c1`
  and `c2` are evaluated
- **THEN** `P1` applies for `c1` and is skipped as `NotEligible` for `c2`; after
  `DELETE .../allowlist/customers/c1` it is skipped for `c1` too

#### Scenario: Replace and read back
- **WHEN** `["c2", "c1"]` is `PUT` and then `c3` is added
- **THEN** `GET` returns `{customer_ids: ["c1", "c2", "c3"]}`
