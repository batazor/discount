# Proposal

## Why

When a code does nothing the customer wants to know why, and the storefront today gets
`NotEligible { rule_index: 2 }` or nothing at all when the code is unknown, expired or
paused. The promocode service being replaced returned error codes such as
`low_order_price` with hints; Talon.One returns rejection reasons per coupon. We add an
explanation layer in the service without changing `PricingResult`, which the Lean model
owns.

## What Changes

- `POST /v1/evaluate?explain=true` and `POST /v1/redeem?explain=true` respond with the
  usual body plus `explanations`: one entry per skipped promotion and per presented coupon
  code, with a stable machine code, the rule concerned (for `not_eligible.*`) and a
  one-line English `message`. Without the flag the responses are unchanged.
- Codes: `not_eligible.<rule>` with `<rule>` in `date_window`, `order_total_range`,
  `tags_match`, `attribute_in`, `attribute_date_window`, `attribute_range`,
  `max_completed_orders`, `max_uses_per_customer`, `max_uses_total`, `requires_coupon`,
  `customer_attribute_in`, `customer_attribute_range`, `requires_allowlist`;
  `exclusive_applied`, `first_match_taken`; for presented codes `coupon.unknown`,
  `coupon.not_started`, `coupon.expired`, `coupon.exhausted`, `coupon.wrong_customer`,
  `coupon.paused`, `coupon.promotion_paused`, `coupon.promotion_archived`, `coupon.applied`.

Outside the verified scope: everything here. The explanations are derived from the
`PricingResult`, the input and the catalogue; they never change the result.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `evaluate-http-api`: the `explain` flag and the explanation vocabulary.

## Impact

- `api/openapi.yaml`: `explain` parameter, schemas `Explanation`, `ExplainedResult`,
  `ExplainedRedeemResponse`.
- `src/application`: `explain(input, result, catalogue facts) -> Vec<Explanation>` as a
  pure function with unit tests per code; `src/infrastructure`: query flag, response
  shapes; tests; UL vocabulary section.
