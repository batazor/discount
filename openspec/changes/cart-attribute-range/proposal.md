# Proposal

## Why

A minimum order value is sometimes checked against a second amount: the promocode service
being replaced validates the floor both on the sale amount and on the amount net of a
partner's commission. Our `OrderTotalRange` reads one subtotal, and the only numeric rule
on cart attributes is a half-open time window. The customer side already has a closed
integer range rule (`CustomerAttributeRange`); the cart deserves the same.

## What Changes

- Core (ubiquitous language → Lean → Rust → DRT): rule `AttributeRange { key, min?, max? }`
  on `cart.attributes`, same semantics as `CustomerAttributeRange` (the value parses as a
  decimal integer `t`, `min ≤ t ≤ max` for the present bounds; missing or non-integer fails).
  No validation beyond the existing rule checks.
- The partner case becomes: the caller puts `commission_free_subtotal = "<minor units>"`
  into `cart.attributes` and the promotion carries
  `AttributeRange { key: "commission_free_subtotal", min: 500000 }` next to its
  `OrderTotalRange`.

In the verified scope: the rule (congruence proofs cover it by construction).

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `eligibility-rules`: one more rule in the catalogue.

## Impact

- `docs/ubiquitous-language.md`, `verification/spec` (Types, Eligibility, Json, Examples),
  `src/domain`, `verification/drt` (generator), `api/openapi.yaml` (`Rule` variant).
