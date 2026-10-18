# Proposal

## Why

The stack starts with one group and three promotions; the smoke adds a code and an
allowlist. Nothing in the console has a title, no campaign is scheduled, exhausted or
ended, no customer has a history, no order can be found. The product cannot be judged on
that: the overview, the campaign statuses, Find, Explain-from-an-order and the customer
listing are all empty or trivial.

## What Changes

- `deploy/demo/catalogue.json`: a complete catalogue in the fixture encoding (21 campaigns
  across hotels, cars, transfers and loyalty, every rule and action variant, four stacking
  groups, codes, rates, titles, an allowlist, grant rules) with an `as_of` day.
- `CatalogFixture` gains `as_of`, `metadata`, `allowlists`, `grant_rules`, so the in-memory
  store loads the whole thing; the demo catalogue becomes the default `DISCOUNT_PROMOTIONS`
  and the image's fixture. `deploy/promotions.json` stays the small fixture the tests replay.
- `deploy/demo/seed.py`: loads the catalogue through the API with windows shifted to today
  and plays a fixed, idempotent order history for seven customers.

Outside the verified scope: everything here; `src/infrastructure/` (fixture loader only),
`deploy/demo/`, `README.md`. No change to the engine, the contract or the console.

## Capabilities

### New Capabilities
- `demo-catalogue`: the demo catalogue file and the seed script.

### Modified Capabilities
- `evaluate-http-api`: the default fixture path is the demo catalogue.

## Impact

- `src/infrastructure/src/persistence/memory.rs`, `src/infrastructure/src/config.rs`,
  `Dockerfile`, `deploy/demo/`, `README.md`, `openspec/specs/evaluate-http-api/spec.md`.
