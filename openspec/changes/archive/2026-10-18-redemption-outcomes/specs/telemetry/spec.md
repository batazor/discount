# Spec Delta

## MODIFIED Requirements

### Requirement: Redemption metrics
The service SHALL record `discount.redemptions` (counter) with attributes `action` in
`reserve | confirm | cancel`, `outcome` in `ok | idempotent | rejected | error` and, for
`rejected`, `reason` in `NotFound | InvalidTransition | Pricing`; `error` SHALL mean a
storage failure only. It SHALL also record
`discount.budget.remaining` (observable gauge, minor units) with `promotion_id` and
`currency` for every promotion that has a budget, refreshed from the catalogue at least
every 15 seconds.

#### Scenario: Budget gauge after a reservation
- **WHEN** `P1` had budget 1000 and a redemption reserved 300
- **THEN** within 15 seconds `discount.budget.remaining{promotion_id="P1"}` reads 700

#### Scenario: Second confirm is a rejection, not an error
- **WHEN** an order is confirmed and then confirmed again
- **THEN** `discount.redemptions{action="confirm",outcome="ok"}` and
  `discount.redemptions{action="confirm",outcome="rejected",reason="InvalidTransition"}`
  each increased by 1 and `outcome="error"` did not change

#### Scenario: Storage failure is an error
- **WHEN** the redemption store is unavailable during a reserve
- **THEN** `discount.redemptions{action="reserve",outcome="error"}` increased by 1
