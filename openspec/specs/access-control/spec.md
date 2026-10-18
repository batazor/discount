# access-control Specification

## Purpose
Who may call what, and whose data they see: bearer JWTs from configured OIDC issuers,
scopes per route family, one tenant per record.

## Requirements

### Requirement: Bearer JWTs from configured issuers
Every route except `GET /healthz`, `GET /openapi.yaml` and `GET /docs` SHALL require
`Authorization: Bearer <jwt>`. The token SHALL be accepted only if its `iss` is one of the
configured issuers, its signature verifies against that issuer's current JWKS (fetched from
OIDC discovery, cached, refreshed on an unknown `kid`), `exp` is in the future, `nbf` (if
present) is in the past, and `aud` contains the configured audience when one is configured.

#### Scenario: Valid token
- **WHEN** a request carries a token signed by a configured issuer's key with a future `exp`
- **THEN** it is handled

### Requirement: Rejected tokens
A missing, malformed, expired or unverifiable token SHALL be answered `401` with the body
`{"type": "about:blank", "title": "Unauthorized", "status": 401, "detail": "<reason>"}` and
the header `WWW-Authenticate: Bearer`, before anything is read or written.

#### Scenario: Unknown issuer or bad signature
- **WHEN** the token's `iss` is not configured, or the signature does not verify
- **THEN** the response is `401` and nothing is read or written

#### Scenario: Expired
- **WHEN** the token's `exp` is in the past
- **THEN** the response is `401` with `detail` naming expiry

### Requirement: Claims map to a tenant and scopes
The service SHALL read the tenant from the claim named by `DISCOUNT_AUTH_TENANT_CLAIM`
(default `tenant_id`; a JSON pointer such as `/realm_access/tenant` is accepted) and the
scopes from the claim named by `DISCOUNT_AUTH_SCOPES_CLAIM` (default `scope`, a
space-separated string or an array; `/realm_access/roles` works for Keycloak). A token
without a tenant claim SHALL be `403` unless it carries `discount:admin`, which may act on
any tenant by naming it in the `X-Tenant` header.

#### Scenario: Keycloak realm roles
- **WHEN** `DISCOUNT_AUTH_SCOPES_CLAIM=/realm_access/roles` and the token has
  `realm_access.roles = ["discount:pricing"]`
- **THEN** the principal has the scope `discount:pricing`

#### Scenario: Admin acting for a tenant
- **WHEN** a token with `discount:admin` and no tenant claim sends `X-Tenant: hotels`
- **THEN** the request runs in tenant `hotels`

### Requirement: Scopes gate route families
Scopes SHALL be `discount:pricing` (`/v1/evaluate`, `/v1/redeem`, `/v1/redemptions/*`),
`discount:customers` (`/v1/customers/*`), `discount:catalog.read` (every `GET` under the
catalogue, metadata, allowlists, coupons, rates, grant rules), `discount:catalog.write`
(their `PUT`, `POST`, `DELETE`, including generation and grants) and `discount:admin`
(everything, plus `/v1/tenants`). A principal without the required scope SHALL get `403`
with a problem body whose `detail` names the missing scope.

#### Scenario: Storefront token
- **WHEN** a token with `discount:pricing discount:customers` calls `PUT /v1/promotions/P1`
- **THEN** the response is `403` naming `discount:catalog.write`

### Requirement: Tenant isolation
Every stored record SHALL belong to exactly one tenant, and every read and write SHALL be
scoped to the principal's tenant: ids are unique per tenant, listings never cross tenants,
`GET` of another tenant's id is `404`.

#### Scenario: Same promotion id in two tenants
- **WHEN** tenants `hotels` and `cars` each `PUT /v1/promotions/P1`
- **THEN** each tenant's `GET /v1/promotions/P1` returns its own body and an evaluation in
  `cars` never sees the `hotels` promotion

### Requirement: Tenant administration
`discount:admin` SHALL manage tenants over `GET /v1/tenants`,
`GET|PUT|DELETE /v1/tenants/{tenant_id}` with `Tenant { tenant_id, name, created_at }`.
`DELETE` SHALL refuse (`409`) a tenant that still has redemptions. A token for an unknown
tenant SHALL be `403` unless `DISCOUNT_AUTH_AUTOCREATE_TENANTS=true`, in which case the
tenant is created on first use.

#### Scenario: Unknown tenant
- **WHEN** autocreation is off and a token names tenant `nope`
- **THEN** the response is `403`

### Requirement: Local mode without authentication
With `DISCOUNT_AUTH=off` the service SHALL accept unauthenticated requests as the
principal `{tenant_id: "default", scopes: all}` and log a warning at start-up. The smoke
stack and the integration tests run in this mode unless they test authentication itself.

#### Scenario: Development
- **WHEN** `DISCOUNT_AUTH=off`
- **THEN** `POST /v1/evaluate` without a header is `200` and the data lives in tenant
  `default`
