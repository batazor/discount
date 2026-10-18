# Spec Delta

## ADDED Requirements

### Requirement: Promotion metadata
The catalogue SHALL store, per promotion, `PromotionMetadata { title, description, source,
labels }` over `GET|PUT|DELETE /v1/promotions/{promotion_id}/metadata`; pricing never reads
it. Limits: `title` 1–200 characters, `description` 0–2000, `source` `null` or
`^[a-z][a-z0-9_-]{0,31}$`, `labels` at most 20 entries with keys 1–64 and values 0–256
characters.

#### Scenario: Status codes
- **WHEN** metadata is `PUT` for an archived promotion, `GET` for a promotion without
  metadata, or `DELETE` twice
- **THEN** the responses are `404`, `404`, and `204` then `404`

#### Scenario: Round trip
- **WHEN** metadata `{title: "Summer", description: "", source: "hotels", labels: {channel: "app"}}`
  is `PUT` for `P1`
- **THEN** `GET` returns it unchanged and `DELETE` removes it

#### Scenario: Unknown promotion
- **WHEN** metadata is `PUT` for `nope`
- **THEN** the response is `404`

#### Scenario: Limits
- **WHEN** a body has an empty `title` or 21 labels
- **THEN** the response is `400` with `decode_error`

### Requirement: Metadata listing and source filter
`GET /v1/promotion-metadata` SHALL list `{promotion_id, metadata}` for every non-archived
promotion that has metadata, ordered by `promotion_id`. `GET /v1/promotions` and
`GET /v1/promotion-metadata` SHALL accept `?source=<s>` and then return only promotions whose
metadata has `source = s`; a promotion without metadata or with another source is left out.

#### Scenario: Filter by source
- **WHEN** `P1` has `source: "hotels"`, `P2` has `source: "cars"` and `P3` has no metadata
- **THEN** `GET /v1/promotions?source=hotels` returns `P1` only, and without the parameter
  all three

#### Scenario: Archived promotions leave the listing
- **WHEN** `P1` with metadata is archived
- **THEN** `GET /v1/promotion-metadata` no longer lists it, and `GET
  /v1/promotions/P1/metadata` is `404`
