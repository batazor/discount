# Spec Delta

## ADDED Requirements

### Requirement: Tenant visible
The console SHALL show the current tenant at the top of the navigation, in the overview
heading and in the description of the campaign, coupon and customer screens: the token's
tenant when signed in, `default` when the token names none, `local` without sign-in. The
console SHALL NOT offer tenant switching and SHALL say so next to `default`.

#### Scenario: Signed in to hotels
- **WHEN** `marketing` (tenant `hotels`) opens the campaigns list
- **THEN** the navigation and the description name `hotels`

#### Scenario: No sign-in
- **WHEN** the console runs without an issuer
- **THEN** the navigation names `local`
