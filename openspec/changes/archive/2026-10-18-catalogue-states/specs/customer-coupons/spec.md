# Spec Delta

## MODIFIED Requirements

### Requirement: Coupon status
`status` SHALL be `applied` when the customer used the coupon (then `order_id` is the
`order_id` of the earliest such redemption); otherwise `paused` when the coupon or its
promotion is in state `paused`; otherwise `expired` when `ends_at ≤ now`, when `max_uses` is
set and `uses ≥ max_uses`, or when the promotion is archived or missing; otherwise
`scheduled` when `starts_at > now`; otherwise `active`. `order_id` is `null` unless the
status is `applied`.

#### Scenario: Applied wins
- **WHEN** `c1` redeemed order `o1` with single-use `ONCE` (so `uses = 1 = max_uses`)
- **THEN** `ONCE` is listed for `c1` as `applied` with `order_id = "o1"`

#### Scenario: Exhausted by someone else
- **WHEN** `c1` saved public single-use `ONCE` and `c2` used it
- **THEN** `ONCE` is listed for `c1` as `expired`

#### Scenario: Window
- **WHEN** a coupon bound to `c1` has `starts_at = now + 10`
- **THEN** it is `scheduled`; with `ends_at = now` it is `expired`; with both absent it is
  `active`

#### Scenario: Cancelled redemption does not apply
- **WHEN** `c1`'s only redemption with `WELCOME` is cancelled
- **THEN** `WELCOME` is not listed for `c1` unless bound or saved, and if saved its status is
  `active`

#### Scenario: Paused
- **WHEN** a coupon bound to `c1` is `paused`, or its promotion is
- **THEN** it is listed as `paused`; once resumed it is `active` again
