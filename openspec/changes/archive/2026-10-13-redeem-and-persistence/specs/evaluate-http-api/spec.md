# Spec Delta

## MODIFIED Requirements

### Requirement: Configuration
The service SHALL read its listen address, the OTLP endpoint, the database URL, the
promotion fixture path and the `PricingPolicy` defaults from environment variables with
documented names and defaults (`DISCOUNT_LISTEN=0.0.0.0:8080`,
`OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317`, `DATABASE_URL` unset,
`DISCOUNT_PROMOTIONS=deploy/promotions.json`, `DISCOUNT_ROUNDING=Down`,
`DISCOUNT_STACKING=Sequential`). With `DATABASE_URL` set the catalogue is Postgres and the
policy comes from the database (seeded from the defaults on first start); without it the
catalogue is in memory, seeded from the fixture.

#### Scenario: Defaults
- **WHEN** the service starts with no environment variables set
- **THEN** it listens on `0.0.0.0:8080`, uses the in-memory catalogue from the fixture,
  `Down` rounding and `Sequential` stacking

#### Scenario: Database configured
- **WHEN** `DATABASE_URL` is set
- **THEN** promotions come from Postgres and the fixture file is not read
