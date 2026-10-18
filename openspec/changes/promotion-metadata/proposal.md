# Proposal

## Why

A promotion is only its pricing today: id, group, rules, action. The storefront's "my
coupons" screen cannot show a title or a description, an operator cannot tell campaigns of
one business line from another, and nothing separates the hotel, car and transfer sources
that the promocode service being replaced partitions everything by.

## What Changes

- New application-layer concept `PromotionMetadata { title, description, source?, labels }`
  stored next to a promotion, never read by pricing. `labels` is a small string map for
  reporting dimensions (issuer, business line, aim, channel, …).
- Sub-resource `GET|PUT|DELETE /v1/promotions/{promotion_id}/metadata`, same pattern as the
  budget; `GET /v1/promotion-metadata` lists the metadata of non-archived promotions.
- `?source=` filter on `GET /v1/promotions`, `GET /v1/promotion-metadata` and
  `GET /v1/customers/{customer_id}/coupons`.
- `CouponPromotion` in the customer's coupon list carries `metadata` (nullable).
- `api/openapi.yaml` first, then scenarios, then code; the generated client follows.

Outside the verified scope: everything in this change. `Promotion` in `PricingInput`, the
Lean model, the Rust domain and the DRT are untouched.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `promotion-catalog`: metadata sub-resource, metadata listing, source filter on the
  promotion listing.
- `customer-coupons`: `metadata` in `CouponPromotion`, source filter.

## Impact

- `api/openapi.yaml`: schema `PromotionMetadata`, parameter `Source`, four operations, one
  field.
- `docs/ubiquitous-language.md`: application-layer name `PromotionMetadata`.
- `src/application`: port methods `get_metadata`, `put_metadata`, `delete_metadata`,
  `list_metadata`; `ListCustomerCoupons` takes an optional source.
- `src/infrastructure`: both stores, migration `0005_promotion_metadata.sql`, routes with a
  query extractor; tests on both backends; smoke; `src/client` contract test.
