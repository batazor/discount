# 12. Authentication is delegated to OIDC issuers and tenancy is a hard partition

Date: 2026-10-18

## Status

Accepted

## Context

Until this decision the service had no authentication and one implicit tenant: anyone who
could reach it could price, redeem and rewrite the catalogue, and the hotel, car and
transfer storefronts that the replaced promocode service kept apart with one shared secret
each would have seen one another's promotions. Three options were considered:

1. API keys issued by this service (Stripe-style `sk_live_…`), hashed at rest, one key per
   consumer. Simple, but a second identity system next to the one every operator already
   runs (Keycloak, Ory, Auth0), with its own rotation and revocation to build.
2. Basic auth with one static token per source, as in the replaced service. Not an identity
   at all: no expiry, no audience, no scopes, shared by every caller of a source.
3. Bearer JWTs from configured OIDC issuers, verified against their JWKS, with the tenant
   and the scopes read from claims. The provider owns identities, rotation and revocation;
   this service only verifies.

Tenancy could be a filter applied in handlers, or a partition of the storage itself.

## Decision

1. **Authentication is delegated.** Every route except health and the API document
   requires `Authorization: Bearer <jwt>`. A token is accepted when its issuer is one of
   `DISCOUNT_AUTH_ISSUERS`, its signature verifies against that issuer's JWKS (found through
   OIDC discovery, cached, refreshed on an unknown key id), it has not expired, and it
   carries the configured audience when one is configured. Several issuers may be trusted
   at once. This service never issues or stores credentials.
2. **Claims are mapped by configuration.** The tenant comes from the claim named by
   `DISCOUNT_AUTH_TENANT_CLAIM` (default `tenant_id`), the scopes from
   `DISCOUNT_AUTH_SCOPES_CLAIM` (default `scope`); both accept a JSON pointer, so Keycloak's
   `realm_access.roles` and a custom claim work without code.
3. **Five scopes gate route families**: `discount:pricing`, `discount:customers`,
   `discount:catalog.read`, `discount:catalog.write`, `discount:admin`. Admin satisfies
   everything and may act for any tenant by naming it in `X-Tenant`.
4. **Tenancy is a hard partition.** Every stored record carries `tenant_id` in its primary
   key; a store handle is bound to one tenant when it is created, so there is no way to run
   a query without choosing a tenant. The pricing engine does not know tenants: the
   catalogue assembles one tenant's snapshot before `apply`.
5. **Local mode.** `DISCOUNT_AUTH=off` runs everything as tenant `default` with every scope
   and logs a warning at start-up; the compose stack and most integration tests use it.
   With `DISCOUNT_AUTH=oidc` and no issuer configured the service refuses to start.
6. **Error shape.** `401` and `403` use RFC 9457 problem details; the domain's existing
   `{"error": …}` bodies are unchanged.

## Consequences

- Operators plug in the identity provider they have; the service's contract is "a JWT with
  these claims", documented in `api/openapi.yaml` (`bearerAuth`, `x-required-scope`).
- A forgotten tenant filter is impossible by construction: the repository has no method
  that is not already scoped.
- JWKS fetching makes start-up depend on the issuer being reachable; a failed refresh keeps
  the cached keys. Clock skew is tolerated with 60 s of leeway.
- Existing data migrates into the tenant `default`; the fixture-seeded in-memory catalogue
  lives there too.
- Row-level security in Postgres is a possible second layer later; it was not chosen first
  because the compose database user is a superuser, which bypasses RLS and would give false
  confidence in tests.
