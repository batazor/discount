# Design

## Decisions

1. **JWT from any OIDC issuer, no API keys of our own.** Operators already run Keycloak or
   Ory; issuing and rotating secrets is their job. We verify, we do not mint. Several
   issuers may be trusted at once (migration between providers, one provider per tenant
   family).
2. **Claims are mapped, not assumed.** Providers differ (`scope` string vs
   `realm_access.roles` array; tenant in a custom claim or in the realm). Two configuration
   knobs with JSON-pointer syntax cover Keycloak, Kratos/Hydra, Auth0 and Azure AD without
   code.
3. **Tenant is a hard partition in the store**, not a filter in the handlers: every table
   gets `tenant_id` in its primary key and every repository method takes the tenant, so a
   forgotten filter is a compile error, not a leak. The core never sees it.
4. **Scopes by route family, five of them.** Fine-grained per-resource scopes are easy to
   add later; five is what a storefront, a back office and an operator need today.
5. **Problem details (RFC 9457)** for 401/403; the existing `{"error": …}` bodies of the
   domain stay as they are to keep the generated client stable.
6. **Auth off for local work**, loudly. The compose stack gains an optional Keycloak
   profile with an exported realm so the smoke run can also exercise a real token.

## Risks / Trade-offs

- [JWKS fetch at start-up] → cached with the issuer's `Cache-Control`; a fetch failure at
  start-up is fatal, a failure at refresh keeps the old keys and logs.
- [Clock skew] → 60 s leeway on `exp`/`nbf`.
- [Large migration: tenant_id everywhere] → one migration adds the column with default
  `'default'`, backfills, then rebuilds primary keys; existing data lands in tenant
  `default`.
