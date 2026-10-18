# Spec Delta

## ADDED Requirements

### Requirement: Coupon uses of a customer
The repository SHALL answer, for a customer, the codes they used: for every reserved or
confirmed redemption with that `customer_id`, each code in `input.cart.coupon_codes` whose
coupon's promotion is in `result.applied`, with the redemption's `order_id`, ordered by
`created_at` then `order_id`. Cancelled redemptions do not count.

#### Scenario: Two orders, one code
- **WHEN** `c1` redeemed `o1` and `o2` with `WELCOME` and both applied its promotion, and
  `o3` with `WELCOME` where the promotion did not apply
- **THEN** the answer is `[(WELCOME, o1), (WELCOME, o2)]`
