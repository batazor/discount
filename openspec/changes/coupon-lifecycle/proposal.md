# Proposal

## Why

A coupon today is a permanent key: anyone who knows the code can use it any number of
times, and a promotion has no ceiling on how often it may be redeemed in total. The
promocode service being replaced issues single-use codes in bulk (with a prefix and a
validity period), caps campaigns by number of uses, and grants codes automatically after an
order cancellation or a review. Without these the service cannot run its campaigns.

## What Changes

- Core (ubiquitous language → Lean → Rust → DRT): `Coupon` gains `max_uses` (absent =
  unlimited, `1` = single use) and `uses` (a snapshot counter); `RequiresCoupon` only accepts
  a coupon with remaining uses. `Promotion` gains `uses_total` (snapshot counter) and the rule
  `MaxUsesTotal { max }`. Validation rejects negative counters.
- Service: the counters are derived from reserved and confirmed redemptions and filled into
  the snapshot (`Redeem` under the budget lock, `Evaluate` without), so a single-use code
  cannot be redeemed twice even concurrently.
- Bulk generation: `POST /v1/coupons/generate` creates `count` distinct codes for a promotion
  with a prefix, a validity window, `max_uses` and an optional customer binding.
- Grants by event: `GrantRule`s (trigger `cancel` | `review`, promotion, priority, validity,
  `max_uses`) stored in the catalogue; `POST /v1/coupons/grant` picks the highest-priority
  rule for the trigger and issues one coupon bound to the customer, idempotent per
  `(trigger, order_id)`.
- `api/openapi.yaml` first, then scenarios, then code; the generated client follows.

Outside the verified scope: generation, grants, and the derivation of the counters. In the
verified scope: how the counters gate eligibility.

## Capabilities

### New Capabilities
- `coupon-lifecycle`: bulk generation, grants by event and the usage counters that make a
  code single-use or a promotion capped by count.

### Modified Capabilities
- `coupons`: `max_uses`, `uses`, and the remaining-uses condition in `RequiresCoupon`.
- `eligibility-rules`: `MaxUsesTotal`, validation of the new counters.
- `promotion-catalog`: snapshot fills `uses` and `uses_total`; grant rules are stored and
  managed over HTTP.
- `redemption`: the repository counts coupon uses and promotion uses from redemptions.

## Impact

- `docs/ubiquitous-language.md`, `docs/invariants.md` (invariant 7 unchanged in form),
  `api/openapi.yaml`.
- `verification/spec`: `Types`, `Validate`, `Eligibility`, `Json`, `Examples`, theorem
  plumbing (`validate_ok` destructuring).
- `src/domain`, `verification/drt` (generators and mutations for the counters).
- `src/application` (ports: counters, grant rules, generation), `src/infrastructure`
  (migration `0003_grant_rules.sql`, queries, routes), `src/client` contract test.
