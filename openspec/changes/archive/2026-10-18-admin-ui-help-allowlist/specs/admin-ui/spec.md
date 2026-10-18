# Spec Delta

## ADDED Requirements

### Requirement: Every option explains itself
Each select of the console (rounding, stacking policy, stacking mode, rule type, action
type, grant trigger) SHALL show a one-sentence description of the chosen option beneath it,
worded after `docs/ubiquitous-language.md`, and SHALL update it when the choice changes.

#### Scenario: Stacking mode explained
- **WHEN** an operator picks `FirstMatch` on the group form
- **THEN** the text under the select says only the first eligible promotion of the group
  applies and the rest are skipped as `FirstMatchTaken`

### Requirement: Terms and badges carry their meaning
Terms that are not options (priority, exclusive, cap, budget, order cap, uses, coupon
binding and limits, grant validity, allocation) SHALL carry a one-sentence tooltip next to
their label or column, and status, mode, trigger, clamp and skip badges SHALL carry their
meaning as a tooltip; all worded after `docs/ubiquitous-language.md`.

#### Scenario: Priority says higher first
- **WHEN** the promotion or group form is opened
- **THEN** the priority field says higher applies first, as the vocabulary does

#### Scenario: Reasons explained in a result
- **WHEN** a result shows a promotion clamped by `Budget` or skipped as `ExclusiveApplied`
- **THEN** hovering the badge explains the reason

### Requirement: Promotion allowlist
The promotion screen SHALL show and replace the promotion's allowlist through
`GET|PUT|DELETE /v1/promotions/{promotion_id}/allowlist`, entered one customer id per line
and deduplicated before sending, and SHALL warn when the promotion has no
`RequiresAllowlist` rule, since the list then has no effect on pricing.

#### Scenario: Replace and clear
- **WHEN** an operator enters `c1`, `c2`, `c1` and saves, then clears the list
- **THEN** the console sends `PUT` with `{"customer_ids":["c1","c2"]}` and the card shows
  2, then sends `DELETE` and the card shows 0

### Requirement: Customer context in the playground
The playground form SHALL take the customer's attributes (key and value rows) and the
promotion ids the caller claims the customer is allowlisted for, and send them as
`customer.attributes` and `customer.allowlisted`.

#### Scenario: Allowlisted by the caller
- **WHEN** `P8` requires an allowlist and the playground lists `P8` under allowlisted
  promotions for customer `c9`
- **THEN** `P8` applies in the result
