# promotion-catalog Specification

## Purpose
Where the pricing policy, stacking groups, promotions and budgets live, how they are edited,
and the snapshot the pricing engine receives.

## Requirements

### Requirement: Persistent catalogue
When `DATABASE_URL` is set the service SHALL store the pricing policy, stacking groups,
promotions and budgets in Postgres, partitioned by tenant (every row carries `tenant_id`,
every key is unique per tenant), running its migrations at start-up. Without `DATABASE_URL`
it SHALL use an in-memory catalogue seeded from `DISCOUNT_PROMOTIONS` into the tenant
`default`.

#### Scenario: Start with a database
- **WHEN** the service starts with `DATABASE_URL` pointing at an empty database
- **THEN** the schema is created and `GET /v1/promotions` returns an empty list for every
  tenant

#### Scenario: Policies per tenant
- **WHEN** tenant `hotels` puts `HalfUp` rounding
- **THEN** tenant `cars` still prices with its own policy

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

### Requirement: Stored coupons carry their limits
`PUT /v1/coupons/{code}` SHALL accept `max_uses` and ignore `uses` (it is derived); `GET`
SHALL return `uses` as counted.

#### Scenario: Uses are read-only
- **WHEN** a coupon is `PUT` with `uses = 5` and nobody used it
- **THEN** `GET` returns `uses = 0`

### Requirement: Promotion metadata
The catalogue SHALL store, per promotion, `PromotionMetadata { title, description, source,
labels }` over `GET|PUT|DELETE /v1/promotions/{promotion_id}/metadata`; pricing never reads
it. Limits: `title` 1–200 characters, `description` 0–2000, `source` `null` or
`^[a-z][a-z0-9_-]{0,31}$`, `labels` at most 20 entries with keys 1–64 and values 0–256
characters.

#### Scenario: Status codes
- **WHEN** metadata is `PUT` for an archived promotion, `GET` for a promotion without
  metadata, or `DELETE` twice
- **THEN** the responses are `404`, `404`, and `204` then `404`

#### Scenario: Round trip
- **WHEN** metadata `{title: "Summer", description: "", source: "hotels", labels: {channel: "app"}}`
  is `PUT` for `P1`
- **THEN** `GET` returns it unchanged and `DELETE` removes it

#### Scenario: Unknown promotion
- **WHEN** metadata is `PUT` for `nope`
- **THEN** the response is `404`

#### Scenario: Limits
- **WHEN** a body has an empty `title` or 21 labels
- **THEN** the response is `400` with `decode_error`

### Requirement: Metadata listing and source filter
`GET /v1/promotion-metadata` SHALL list `{promotion_id, metadata}` for every non-archived
promotion that has metadata, ordered by `promotion_id`. `GET /v1/promotions` and
`GET /v1/promotion-metadata` SHALL accept `?source=<s>` and then return only promotions whose
metadata has `source = s`; a promotion without metadata or with another source is left out.

#### Scenario: Filter by source
- **WHEN** `P1` has `source: "hotels"`, `P2` has `source: "cars"` and `P3` has no metadata
- **THEN** `GET /v1/promotions?source=hotels` returns `P1` only, and without the parameter
  all three

#### Scenario: Archived promotions leave the listing
- **WHEN** `P1` with metadata is archived
- **THEN** `GET /v1/promotion-metadata` no longer lists it, and `GET
  /v1/promotions/P1/metadata` is `404`

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
