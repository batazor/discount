# Spec Delta

## ADDED Requirements

### Requirement: Redemptions remember the customer
A `Redemption` SHALL store `customer_id` (from `customer.customer_id`, may be absent). The
repository SHALL answer, for a customer id, the number of reserved or confirmed redemptions
per applied `promotion_id`; cancelled redemptions do not count.

#### Scenario: Counting uses
- **WHEN** customer `c1` has redemptions `o1` (reserved, applied `P1`), `o2` (confirmed,
  applied `P1` and `P2`) and `o3` (cancelled, applied `P1`)
- **THEN** the uses for `c1` are `P1: 2, P2: 1`
