# Proposal

## Why

With `oidc-auth-and-tenants` the service refuses calls without a bearer token, so the
console cannot talk to it at all once auth is on. Operators need a sign-in, and the demo
stack needs people to sign in as.

## What Changes

- **Sign-in page** at `/login`: username and password posted to the console, which obtains
  tokens from the configured OIDC issuer (resource owner password grant, the demo path;
  the authorization-code redirect is available through `DISCOUNT_UI_OIDC_FLOW=code` when the
  issuer is reachable from the browser). Tokens live in the server-side session, never in
  the browser.
- **Token on every call**: every request the console makes to the service carries
  `Authorization: Bearer`; a token about to expire is refreshed with the refresh token
  before the page renders; a failed refresh sends the operator to sign in again.
- **Who is signed in** in the navigation: name, tenant and scopes, with sign-out; the role
  defaults from the scopes (catalogue write → marketing, else support) until the operator
  switches.
- **Demo users** in the local realm: `marketing` and `support`, tenant `hotels`, plus a
  public console client; the auth overlay points the console at Keycloak and names the
  demo users on the sign-in page.
- Without `DISCOUNT_UI_OIDC_ISSUER` nothing changes: the console runs as today against a
  service with `DISCOUNT_AUTH=off`.

Outside the verified scope: everything in this change; no API operation is added. The
console verifies no token itself: the service does.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `admin-ui`: sign-in, token on every call, session refresh and sign-out, demo users.

## Impact

- `src/ui/src/auth/*`, `src/ui/src/middleware.ts`, `src/ui/src/pages/login.astro`,
  `logout.ts`, `auth/callback.ts`, `api/client.ts` (bearer middleware), `layouts/Layout.astro`.
- `ops/keycloak/realm-discount.json` (client `discount-console`, users), `ops/compose.auth.yaml`
  (console environment), README.
