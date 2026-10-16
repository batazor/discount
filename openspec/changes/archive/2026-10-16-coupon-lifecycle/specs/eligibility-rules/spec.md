# Spec Delta

## ADDED Requirements

### Requirement: Total usage limit
`Promotion` SHALL carry `uses_total` (`≥ 0`, how many reserved or confirmed redemptions
applied it). The rule `MaxUsesTotal { max }` SHALL hold when `uses_total < max`. `validate`
SHALL reject a negative `uses_total` (`InvalidPromotion { field: "promotions[<i>].uses_total" }`)
and a negative `max` (`InvalidRule { field }`), in the promotion's check sequence after the
action and amounts.

#### Scenario: Campaign capped by count
- **WHEN** `P1` carries `MaxUsesTotal { max: 100 }` and `uses_total` is 100
- **THEN** `P1` is skipped as `NotEligible`; with 99 it applies
