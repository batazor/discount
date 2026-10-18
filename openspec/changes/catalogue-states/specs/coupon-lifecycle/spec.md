# Spec Delta

## ADDED Requirements

### Requirement: Paused promotions issue no codes
`POST /v1/coupons/generate` for a paused promotion and a grant whose winning rule names a
paused promotion SHALL respond `409 {"error": "PromotionPaused"}` and store nothing.

#### Scenario: Grant on a paused promotion
- **WHEN** the highest-priority rule for `cancel` names `P1` and `P1` is `paused`
- **THEN** the response is `409` and no coupon is created
