# Spec Delta

## MODIFIED Requirements

### Requirement: A customer's coupons
`GET /v1/customers/{customer_id}/coupons` SHALL return every stored coupon that is bound to
the customer (`coupon.customer_id` equals `customer_id`), used by the customer (a reserved or
confirmed redemption with that `customer_id` presented the code and applied the coupon's
promotion), or saved by the customer, each once, ordered by `code`, as
`CustomerCoupon { coupon, status, order_id, promotion }` with `promotion` =
`{ promotion_id, action, metadata }` of the coupon's promotion (`metadata` is `null` when
none is stored). With `?source=<s>` only entries whose promotion metadata has `source = s`
are returned.

#### Scenario: Unknown customer
- **WHEN** nothing refers to customer `nobody`
- **THEN** the list is empty with status `200`

#### Scenario: Promotion deleted
- **WHEN** the promotion of a coupon bound to `c1` was deleted
- **THEN** the entry has `promotion = null` and status `expired`

#### Scenario: Three sources, no duplicates
- **WHEN** `GRANT-1` is bound to `c1`, `c1` used public code `WELCOME` in a reserved
  redemption, `c1` saved `SPRING`, and `OTHER` belongs to nobody
- **THEN** the list for `c1` holds `GRANT-1`, `SPRING`, `WELCOME` and nothing else

#### Scenario: Another customer's binding
- **WHEN** `GRANT-2` is bound to `c2`
- **THEN** it is not in the list for `c1`

#### Scenario: Metadata and source filter
- **WHEN** `GRANT-1`'s promotion has metadata `{title: "Summer", source: "hotels"}` and
  `SPRING`'s promotion has none
- **THEN** the entry for `GRANT-1` carries that metadata, the entry for `SPRING` has
  `metadata = null`, and `?source=hotels` returns `GRANT-1` only
