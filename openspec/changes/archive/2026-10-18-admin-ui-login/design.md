# Design

## Context

The service verifies JWTs from configured issuers (`oidc-auth-and-tenants`); scopes are
realm roles under `/realm_access/roles`, the tenant a claim `tenant_id`. In the compose
stack Keycloak is reachable as `http://keycloak:8080` from containers only, so a browser
on the host cannot complete a redirect to it without a hosts entry; the console can.

## Decisions

1. **Password grant for the demo, code flow as an option.** The console's own form posts
   to Keycloak's token endpoint server to server (client `discount-console`, public,
   direct access grants on). The browser never needs to reach the issuer. The
   authorization-code flow with PKCE is implemented behind `DISCOUNT_UI_OIDC_FLOW=code` for
   deployments where the issuer has a browser-facing hostname; it is not exercised in the
   local stack.
2. **Tokens in the server session.** Astro's session (filesystem store of the Node
   adapter, httpOnly cookie) holds access token, refresh token, expiry and the decoded
   claims used for display. The console decodes the payload for display only; it never
   verifies signatures, the service does.
3. **Bearer through AsyncLocalStorage.** The Astro middleware runs the request inside an
   `AsyncLocalStorage` scope holding the token; the API client's request middleware reads
   it. No call site changes, actions included.
4. **Refresh before render.** The middleware refreshes when `expires_at - now < 60 s`;
   failure clears the session and redirects to `/login`. A `401` that still reaches a page
   (revoked token) is rendered with a sign-in link by `ApiError`.
5. **Role defaults from scopes** when no role cookie exists: `discount:catalog.write` or
   `discount:admin` → marketing, else support.
6. **Demo users are realm data**, not console configuration: two users with passwords and
   realm roles in `realm-discount.json`; `DISCOUNT_UI_DEMO_USERS` only tells the sign-in
   page what to show.

## Risks / Trade-offs

- [Password grant is discouraged for production] → documented as the demo path; the code
  flow is the production path and shares every other piece.
- [Session store on the filesystem] → one instance per volume; fine for an operator tool.
- [Scopes claim path differs per provider] → the console reads roles from
  `realm_access.roles` or `scope` for display only; authorisation stays with the service.
