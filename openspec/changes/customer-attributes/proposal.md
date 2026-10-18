# Proposal

## Why

Eligibility can read the cart in detail (tags, attributes, check-in dates) but almost
nothing about the customer: only the order count and the usage counters. The promocode
service being replaced gates campaigns on who the customer is: an uploaded list of customer
ids per campaign, "first booking from the app", partner conditions. Without that the
service cannot run those campaigns.

## What Changes

- Core (ubiquitous language → Lean → Rust → DRT): `CustomerContext` gains
  `attributes : List Attribute` (the same string key/value pairs the cart has) and
  `allowlisted : List PromotionId` (the promotions whose allowlist names this customer,
  filled by the service like `uses`). Three rules: `CustomerAttributeIn { key, values }`,
  `CustomerAttributeRange { key, min?, max? }` (the value parses as a decimal integer in the
  closed range) and `RequiresAllowlist`. Validation rejects a promotion id listed twice in
  `allowlisted`.
- Service: an allowlist per promotion stored in the catalogue and managed over
  `GET|PUT|DELETE /v1/promotions/{promotion_id}/allowlist` and
  `GET|PUT|DELETE /v1/promotions/{promotion_id}/allowlist/customers/{customer_id}`;
  `Evaluate` and `Redeem` add to `customer.allowlisted` every promotion whose allowlist
  contains `customer.customer_id`.
- `api/openapi.yaml` first, then scenarios, then code; the generated client follows.

Non-goals, recorded for a later change ("anonymous customers"): a device id on the
redemption with per-device usage counters, and binding a coupon to an e-mail address.

In the verified scope: the three rules and the validation of `allowlisted`. Outside: the
allowlist storage and the enrichment of the customer context.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `eligibility-rules`: new rules, new customer fields, validation of `allowlisted`.
- `promotion-catalog`: allowlist resource and the enrichment of `customer.allowlisted`.

## Impact

- `api/openapi.yaml`: `CustomerContext` fields, three `Rule` variants, schema `Allowlist`,
  six operations.
- `docs/ubiquitous-language.md`, `verification/spec`, `src/domain`, `verification/drt`
  (generators, one mutation, fixtures).
- `src/application`: ports `get_allowlist`, `put_allowlist`, `delete_allowlist`,
  `add_to_allowlist`, `remove_from_allowlist`, `allowlisted_for`; `Redeem` reads the
  catalogue.
- `src/infrastructure`: both stores, migration `0006_allowlists.sql`, routes; tests on both
  backends; smoke; `src/client` contract test.
