# Tasks

## 1. Auth plumbing (`src/ui/src/auth`, `src/ui/src/api`, `src/ui/src/middleware.ts`)

- [x] 1.1 `auth/oidc.ts`: discovery with cache, password grant, refresh, revoke, authorize URL with PKCE, code exchange, JWT payload decode; vitest for decode, expiry and the authorize URL; verify `npm test`
- [x] 1.2 Session shape and `AsyncLocalStorage` bearer middleware on the API client; Astro middleware gating pages, refreshing and redirecting; verify "Redirected to sign in" with curl against a local stack

## 2. Pages (`src/ui/src/pages`, `src/ui/src/layouts`)

- [x] 2.1 `/login` (form, issuer errors, demo users, code-flow redirect), `/logout`, `/auth/callback`; `ApiError` sign-in link on 401; who-is-signed-in chip and role default in the layout; verify "Wrong password", "Signed in and back", "Sign out", "Support role by default" against a local Keycloak

## 3. Realm and stack (`ops/keycloak`, `ops/compose.auth.yaml`, `README.md`)

- [x] 3.1 Client `discount-console` and users `marketing`, `support` in the realm export; console environment in the auth overlay; README section; verify "Demo users listed" and "Support cannot write the catalogue" against a local Keycloak with the realm
