# Spec Delta

## ADDED Requirements

### Requirement: Customer attributes and allowlists in the context
`CustomerContext` SHALL carry `attributes : List Attribute` (string key/value pairs the
caller supplies, such as `tier = "gold"` or `completed_app_orders = "0"`) and
`allowlisted : List PromotionId`, the promotions whose allowlist names this customer. The
service fills `allowlisted` from the catalogue and merges it with what the caller supplied,
each id once.

#### Scenario: Caller and service both contribute
- **WHEN** the caller sends `allowlisted = ["P1"]` and the catalogue lists the customer on
  the allowlist of `P2`
- **THEN** pricing sees `allowlisted = ["P1", "P2"]`

### Requirement: Customer rules
Three rules SHALL read the customer: `CustomerAttributeIn { key, values }` holds when some
`customer.attributes` entry has that `key` and a `value` in `values`;
`CustomerAttributeRange { key, min?, max? }` holds when the attribute `key` exists, its value
parses as a decimal integer `t`, and `min ≤ t` (or absent) and `t ≤ max` (or absent); a
missing or non-integer value fails the rule; `RequiresAllowlist` holds when the promotion's
`promotion_id` is in `customer.allowlisted`.

#### Scenario: Tier
- **WHEN** a promotion carries `CustomerAttributeIn { key: "tier", values: ["gold"] }` and
  the customer has `tier = "gold"`
- **THEN** the rule holds; with `tier = "silver"` or no `tier` it does not

#### Scenario: First booking from the app
- **WHEN** a promotion carries `TagsMatch { any: ["#ios", "#android"] }` and
  `CustomerAttributeRange { key: "completed_app_orders", min: null, max: 0 }`, the cart is
  tagged `#ios` and the customer has `completed_app_orders = "0"`
- **THEN** the promotion is eligible; with `completed_app_orders = "1"` or `"many"` it is not

#### Scenario: Allowlist
- **WHEN** `P1` carries `RequiresAllowlist` and `customer.allowlisted` is `["P1"]`
- **THEN** `P1` is eligible; with `allowlisted = []` it is skipped as `NotEligible`

## MODIFIED Requirements

### Requirement: Customer context is validated
`validate` SHALL reject `customer.completed_orders < 0`, a negative count in
`customer.uses`, a `promotion_id` listed twice in `customer.uses`, and a `promotion_id`
listed twice in `customer.allowlisted` (`InvalidCustomerContext { field }` with
`customer.uses[<k>]` or `customer.allowlisted[<k>]`, `<k>` the second occurrence), in that
order.

#### Scenario: Duplicate use entry
- **WHEN** `customer.uses` lists `P1` twice
- **THEN** `validate` reports `InvalidCustomerContext { field: "customer.uses[1]" }`

#### Scenario: Duplicate allowlist entry
- **WHEN** `customer.allowlisted` is `["P1", "P2", "P1"]`
- **THEN** `validate` reports `InvalidCustomerContext { field: "customer.allowlisted[2]" }`
