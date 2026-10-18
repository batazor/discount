# Tasks

- [ ] 1. `api/openapi.yaml`: import and export operations, `ImportReport`, listing filter; UL `ImportCoupons`; verify lint and `openspec validate coupon-import-export`
- [ ] 2. Application `ImportCoupons` with row parsing and reasons; store method `put_coupons` reused, `list_coupons_of(promotion_id)`
- [ ] 3. HTTP routes with 2 MiB body limit and streaming export; tests on both backends; contract test (generated client with raw bodies); smoke step (import 3 rows, export, re-import)

## Workflow follow-up

- Archive the change after review: `openspec archive coupon-import-export`.
