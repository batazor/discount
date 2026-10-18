# Spec Delta

## MODIFIED Requirements

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
