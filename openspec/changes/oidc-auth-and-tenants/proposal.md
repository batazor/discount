# Proposal

## Why

The service has no authentication: anyone who reaches it can price, redeem, and rewrite
the catalogue. The promocode service being replaced issued one secret per consumer system
and kept each consumer's data apart. We go the industry way instead of copying its Basic
auth: callers present a JWT from any OIDC provider the operator trusts (Keycloak, Ory
Kratos/Hydra, Auth0, …), and every record belongs to a tenant named in the token.

## What Changes

- New capability `access-control`: bearer JWTs verified against the JWKS of one or more
  configured issuers (OIDC discovery), claims mapped by configuration to a `tenant_id` and
  a set of scopes; scopes gate route families; `401`/`403` bodies follow RFC 9457 problem
  details.
- Hard multi-tenancy: policy, groups, promotions, budgets, metadata, allowlists, coupons,
  rates, grant rules, saved coupons and redemptions carry `tenant_id` and are only ever
  read or written within the caller's tenant. Metadata `source` stays a label inside a
  tenant.
- Platform administration: a token with the `discount:admin` scope manages tenants over
  `GET|PUT|DELETE /v1/tenants/{tenant_id}`; the service creates a tenant row on first
  sight of a token for an unknown tenant only if `DISCOUNT_AUTH_AUTOCREATE_TENANTS=true`.
- `DISCOUNT_AUTH=off` keeps the current unauthenticated single-tenant behaviour for local
  development and the smoke stack (`tenant_id = "default"`).

Outside the verified scope: everything here. `PricingInput` and the Lean model do not know
tenants: the catalogue assembles a tenant's snapshot before pricing.

## Capabilities

### New Capabilities
- `access-control`: JWT verification, claim mapping, scopes, tenant isolation, tenant
  administration.

### Modified Capabilities
- `evaluate-http-api`: configuration variables for auth; the OpenAPI document declares the
  bearer security scheme and the scopes of every operation.
- `promotion-catalog`: persistent catalogue is partitioned by tenant.

## Impact

- `api/openapi.yaml`: `securitySchemes.bearerAuth`, `security` per operation with scope
  names, schemas `Tenant`, `Problem`; `/v1/tenants` routes.
- `src/infrastructure`: `auth` module (discovery, JWKS cache, verification), middleware
  that resolves `Principal { tenant_id, scopes, subject }`, `tenant_id` columns and
  composite keys in every table (migration `0007_tenants.sql`), repository methods take the
  tenant; `src/application` ports take a `TenantId`.
- `ops/compose.yaml`: optional Keycloak profile with a realm export for the smoke run.
- `src/client`: the generated client gains a bearer token hook; contract test runs with a
  locally signed token against a test JWKS.
- `docs/ubiquitous-language.md`: application-layer names `Tenant`, `Principal`, `Scope`.
