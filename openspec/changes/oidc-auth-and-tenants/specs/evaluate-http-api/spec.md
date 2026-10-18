# Spec Delta

## MODIFIED Requirements

### Requirement: Configuration
The service SHALL read its listen address, the OTLP endpoint, the database URL, the
promotion fixture path, the `PricingPolicy` defaults and the authentication settings from
environment variables with documented names and defaults (`DISCOUNT_LISTEN=0.0.0.0:8080`,
`OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317`, `DATABASE_URL` unset,
`DISCOUNT_PROMOTIONS=deploy/promotions.json`, `DISCOUNT_ROUNDING=Down`,
`DISCOUNT_STACKING=Sequential`, `DISCOUNT_AUTH=oidc|off` default `oidc`,
`DISCOUNT_AUTH_ISSUERS` comma-separated issuer URLs, `DISCOUNT_AUTH_AUDIENCE` optional,
`DISCOUNT_AUTH_TENANT_CLAIM=tenant_id`, `DISCOUNT_AUTH_SCOPES_CLAIM=scope`,
`DISCOUNT_AUTH_AUTOCREATE_TENANTS=false`). With `DATABASE_URL` set the catalogue is Postgres
and the policy comes from the database (seeded from the defaults on first start); without it
the catalogue is in memory, seeded from the fixture. With `DISCOUNT_AUTH=oidc` and no issuer
configured the service SHALL refuse to start.

#### Scenario: Defaults
- **WHEN** the service starts with no environment variables set
- **THEN** it refuses to start, naming `DISCOUNT_AUTH_ISSUERS`

#### Scenario: Development defaults
- **WHEN** the service starts with only `DISCOUNT_AUTH=off`
- **THEN** it listens on `0.0.0.0:8080`, uses the in-memory catalogue from the fixture,
  `Down` rounding and `Sequential` stacking

#### Scenario: Database configured
- **WHEN** `DATABASE_URL` is set
- **THEN** promotions come from Postgres and the fixture file is not read

### Requirement: The OpenAPI document is the contract
`api/openapi.yaml` SHALL describe every route the service serves, with the ubiquitous
language JSON shapes and the documented status codes, a `bearerAuth` security scheme and,
per operation, the scope it requires. The service SHALL serve the same document at
`GET /openapi.yaml` and Swagger UI at `GET /docs`. A client generated from the document
SHALL be able to drive evaluation, redemption lifecycle and catalogue management against a
running service, sending a bearer token.

#### Scenario: Every route is documented
- **WHEN** the router is compared with the document
- **THEN** every path the router serves appears in the document with its security
  requirement

#### Scenario: Generated client round trip
- **WHEN** the generated client, with a token, puts a group and a promotion, evaluates a
  cart, redeems an order, confirms it and reads the redemption back
- **THEN** every call succeeds with the documented status and the decoded bodies match the
  server's
