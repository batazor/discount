# Spec Delta

## ADDED Requirements

### Requirement: Redemption metrics
The service SHALL record `discount.redemptions` (counter) with attributes `action` in
`reserve | confirm | cancel` and `outcome` in `ok | idempotent | error`, and
`discount.budget.remaining` (observable gauge, minor units) with `promotion_id` and
`currency` for every promotion that has a budget, refreshed from the catalogue at least
every 15 seconds.

#### Scenario: Budget gauge after a reservation
- **WHEN** `P1` had budget 1000 and a redemption reserved 300
- **THEN** within 15 seconds `discount.budget.remaining{promotion_id="P1"}` reads 700
