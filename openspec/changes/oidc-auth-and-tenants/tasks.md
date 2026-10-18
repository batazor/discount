# Tasks

## 1. Contract first (`api/`, `docs/`)

- [ ] 1.1 `api/openapi.yaml`: `bearerAuth`, per-operation `security` with scopes, `Problem`, `Tenant`, `/v1/tenants` routes; verify Redocly lint
- [ ] 1.2 `docs/ubiquitous-language.md`: `Tenant`, `Principal`, `Scope`; ADR 11 "Authentication is delegated to OIDC, tenancy is a hard partition" via `adr new`; verify `openspec validate oidc-auth-and-tenants`

## 2. Service

- [ ] 2.1 `auth` module: discovery, JWKS cache with refresh on unknown `kid`, verification (RS256/ES256), claim mapping; unit tests with a locally generated key pair
- [ ] 2.2 Middleware: `Principal` extension, scope checks per route family, problem bodies, `DISCOUNT_AUTH=off`
- [ ] 2.3 Tenancy: migration `0007_tenants.sql`, `tenant_id` in every repository method and table key, in-memory store partitioned; ports take `TenantId`
- [ ] 2.4 Tenant administration routes; autocreation flag
- [ ] 2.5 Tests: port scenarios on both backends for isolation, HTTP tests with signed tokens (valid, expired, wrong issuer, missing scope, admin with `X-Tenant`), contract test with a token
- [ ] 2.6 Compose: Keycloak profile with realm export; smoke step that obtains a token and calls `/v1/evaluate`

## Workflow follow-up

- Archive the change after review: `openspec archive oidc-auth-and-tenants`.
