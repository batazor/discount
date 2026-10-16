# Spec Delta

## ADDED Requirements

### Requirement: Counting uses per coupon and per promotion
The repository SHALL answer, for a list of codes, how many reserved or confirmed redemptions
presented each code and applied its promotion, and for every promotion how many reserved or
confirmed redemptions applied it. Cancelled redemptions do not count.

#### Scenario: Counts after a cancellation
- **WHEN** `o1` (reserved) and `o2` (cancelled) both presented `ONCE` and applied `P1`
- **THEN** `ONCE` has `uses = 1` and `P1` has `uses_total = 1`
