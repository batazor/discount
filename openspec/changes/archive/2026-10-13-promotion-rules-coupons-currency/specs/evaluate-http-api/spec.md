# Spec Delta

## MODIFIED Requirements

### Requirement: Evaluate endpoint
The service SHALL expose `POST /v1/evaluate` accepting a JSON body with `cart`
(a `CartSnapshot`, including `tags`, `attributes` and `coupon_codes`) and `customer`
(a `CustomerContext` with `customer_id`, `completed_orders` and `uses`), both encoded exactly
as in `docs/ubiquitous-language.md`. The service SHALL load the policy, groups,
promotions, the coupons matching `cart.coupon_codes` and the exchange rates from its
catalogue, take `now` from its clock, fill `customer.uses` from reserved and confirmed
redemptions of `customer.customer_id` (adding to any counts the caller supplied), and
respond `200 OK` with the `PricingResult` JSON on success.

#### Scenario: Successful evaluation
- **WHEN** a valid cart is posted
- **THEN** the response is `200` with a `PricingResult` whose `final_total` equals
  `original_subtotal - total_discount`, and the body is identical to what
  `discount_core::apply` returns for the same assembled input

#### Scenario: Uses come from redemptions
- **WHEN** customer `c1` has two reserved redemptions that applied `P1`, `P1` carries
  `MaxUsesPerCustomer { max: 2 }`, and `c1` evaluates a cart with `uses` empty
- **THEN** `P1` is skipped as `NotEligible`
