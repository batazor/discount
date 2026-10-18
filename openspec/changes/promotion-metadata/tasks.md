# Tasks

## 1. Contract first (`api/`, `docs/`)

- [x] 1.1 Extend `api/openapi.yaml`: schema `PromotionMetadata`, `MetadataEntry`, parameter `Source`, operations `getPromotionMetadata`, `putPromotionMetadata`, `deletePromotionMetadata`, `listPromotionMetadata`, `source` on `listPromotions` and `listCustomerCoupons`, `metadata` in `CouponPromotion`; verify Redocly lint has no errors
- [x] 1.2 `docs/ubiquitous-language.md` application-layer name `PromotionMetadata`; verify `openspec validate promotion-metadata`

## 2. Service (`src/application`, `src/infrastructure`, `src/client`)

- [x] 2.1 Ports `get_metadata`, `put_metadata`, `delete_metadata`, `list_metadata`; `PromotionMetadata` with its validation; `ListCustomerCoupons` with an optional source
- [x] 2.2 In-memory and Postgres stores (migration `0005_promotion_metadata.sql`); verify the shared port scenario on both backends
- [x] 2.3 HTTP routes with the `source` query, the generated client; verify axum tests, `openapi_covers_every_route` and the contract test
- [x] 2.4 Smoke: metadata for `P3`, the customer's listing shows the title and the source filter works; verify `deploy/smoke.sh` on the compose stack

## Workflow follow-up

- Archive the change after review: `openspec archive promotion-metadata`.
