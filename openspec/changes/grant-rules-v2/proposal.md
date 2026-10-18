# Proposal

## Why

Grant rules fire on two fixed triggers and know nothing about the event: no cooldown, no way
to pick a rule by channel or domain, no check that the promotion is still live. The
promocode service being replaced throttles cancellation codes per user, matches rules by
domain and refuses inactive campaigns; its generation log also knows birthday and landing
page reasons. We generalise rather than copy: open trigger names, conditions on event
attributes, a per-customer cooldown, and liveness checks.

## What Changes

- `GrantRule` gains `cooldown_seconds` (default `0`) and `conditions: [{key, values}]`;
  `trigger` becomes any name matching `^[a-z][a-z0-9_.-]{0,63}$` instead of the enum.
- `POST /v1/coupons/grant` body gains `attributes: [{key, value}]` describing the event
  (channel, domain, country, …). Rule selection: among the rules for the trigger whose every
  condition is satisfied by the attributes, the highest `priority`, ties by `rule_id`.
- Cooldown: if the same customer received a coupon from this trigger within
  `cooldown_seconds`, that coupon is returned again (`200`) and nothing new is created.
- Liveness: the winning rule's promotion must exist, be non-archived and `active`; otherwise
  `409 {"error": "PromotionPaused"}` or `404`.

Outside the verified scope: everything here.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `coupon-lifecycle`: grant rules and grant by event.

## Impact

- `api/openapi.yaml`: `GrantRule` fields, `GrantTrigger` becomes a pattern string,
  `GrantRequest.attributes`.
- `src/application/coupons.rs`: rule matching, cooldown lookup through `grants`;
  `GrantTrigger` becomes a validated newtype.
- `src/infrastructure`: `grants` gets `customer_id` and `created_at` index (migration
  `0009_grants_customer.sql`); tests on both backends; contract test; smoke step.
