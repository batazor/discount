# Spec Delta

## ADDED Requirements

### Requirement: Sign-in gate
When `DISCOUNT_UI_OIDC_ISSUER` is set, every console page except `/login`, `/healthz`,
`/api-docs` and `/openapi.yaml` SHALL require a signed-in session and SHALL redirect to
`/login` with the requested path kept. Without the variable the console SHALL run without
any sign-in, as before.

#### Scenario: Redirected to sign in
- **WHEN** `/campaigns` is opened without a session
- **THEN** the browser lands on `/login?back=/campaigns`

### Requirement: Signing in
`/login` SHALL obtain tokens from the issuer with the submitted username and password (or,
with `DISCOUNT_UI_OIDC_FLOW=code`, through the issuer's authorization endpoint) and store
them in the server-side session only; the browser SHALL receive nothing but the session
cookie.

#### Scenario: Wrong password
- **WHEN** a wrong password is submitted
- **THEN** the page shows the issuer's error and keeps the username

#### Scenario: Signed in and back
- **WHEN** `marketing` signs in from `/login?back=/campaigns`
- **THEN** the browser lands on `/campaigns` and the page lists the campaigns of the
  user's tenant

### Requirement: Token on every call
Every request the console makes to the service SHALL carry the session's access token as
`Authorization: Bearer`; a `401` from the service SHALL be shown with a link to sign in
again, a `403` verbatim as any API error.

#### Scenario: Support cannot write the catalogue
- **WHEN** `support` saves a stacking group
- **THEN** the form shows the service's `403` problem body

### Requirement: Session refresh and sign-out
A token expiring within sixty seconds SHALL be refreshed with the refresh token before a
page renders; when the refresh fails the session SHALL be cleared and the operator sent to
`/login`. Sign-out SHALL clear the session and revoke the refresh token when the issuer
offers revocation.

#### Scenario: Sign out
- **WHEN** the operator signs out
- **THEN** the browser lands on `/login` and opening `/campaigns` redirects there again

### Requirement: Who is signed in
The navigation SHALL show the signed-in name, tenant and scopes with a sign-out control,
and the role SHALL default from the scopes (catalogue write → `marketing`, otherwise
`support`) until the operator switches it.

#### Scenario: Support role by default
- **WHEN** `support` signs in with no role cookie
- **THEN** the overview is the support overview

### Requirement: Demo users in the local realm
The Keycloak realm of the auth overlay SHALL contain a public client `discount-console`
with direct access grants, tenant `hotels`, and the users `marketing` (catalogue read and
write, pricing, customers) and `support` (pricing, customers, catalogue read); the overlay
SHALL point the console at the realm and name the demo users on the sign-in page.

#### Scenario: Demo users listed
- **WHEN** the auth overlay is up and `/login` is opened
- **THEN** the page names `marketing` and `support` as demo users
