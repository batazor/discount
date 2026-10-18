# Spec Delta

## REMOVED Requirements

### Requirement: Role switch
**Reason**: It only reordered the navigation and changed the overview; the token's scopes
now say what an operator does.
**Migration**: The role cookie is ignored; navigation and overview follow the scopes
("Navigation follows scopes", "Overview").

### Requirement: Overview by role
**Reason**: Replaced by one overview whose sections follow the scopes.
**Migration**: See "Overview".

## ADDED Requirements

### Requirement: Navigation follows scopes
The navigation SHALL lead with the campaign screens when the token carries
`discount:catalog.write` (or `discount:admin`) and with the support screens otherwise, and
SHALL leave out screens whose scope the token lacks: campaigns, coupons and settings need
`discount:catalog.read`, Explain and redemptions `discount:pricing`, customers
`discount:customers`, Find either of the last two. Without sign-in every screen SHALL be
offered.

#### Scenario: Support account
- **WHEN** `support` (pricing, customers, catalogue read) signs in
- **THEN** the navigation leads with Find and Explain, lists campaigns and settings after,
  and offers no role switch

### Requirement: Overview
The overview SHALL show the find box and recent lookups when the token can look customers
or orders up, the campaign health sections (live, ending within seven days, lowest
remaining budgets, exhausted, scheduled) when it can read the catalogue, and the first-run
checklist when it can write the catalogue and the catalogue has no group or no campaign.

#### Scenario: First run
- **WHEN** the catalogue has no groups and the token can write it
- **THEN** the overview shows the checklist with "create a group" as the next step

#### Scenario: Ending soon
- **WHEN** a campaign's window ends in three days
- **THEN** it is listed under "ending soon" with the end shown in local time

### Requirement: Actions follow scopes
Without `discount:catalog.write` the console SHALL render catalogue forms read-only with a
note and SHALL NOT show create, save, archive, delete or generate controls; without
`discount:pricing` it SHALL NOT offer Explain, "try it" or the redemption transitions;
without `discount:customers` it SHALL NOT offer the customer screens. A direct request
without the scope SHALL still show the service's `403` verbatim.

#### Scenario: Support sees the catalogue read-only
- **WHEN** `support` opens a campaign's rules tab
- **THEN** the fields are disabled, a read-only note is shown and there is no Save

#### Scenario: Budget edited from the overview
- **WHEN** `marketing` sets a campaign's budget on its overview tab
- **THEN** the console sends the budget `PUT` and the remaining budget updates
