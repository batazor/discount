# Design

## Decisions

1. **Sub-resource, not a field of `Promotion`.** `Promotion` is a core type shared by the
   pricing input and the catalogue; metadata is application state like the budget, so it
   follows the budget pattern (`/v1/promotions/{id}/budget`). Lean, Rust domain and DRT stay
   untouched and the catalogue's `Promotion` bodies do not change.
2. **Three typed fields plus labels.** `title`, `description` and `source` are what the
   storefront and the filters need; everything else promocode-service kept as columns
   (issuer, business line, aim, channel) is a reporting dimension and lives in `labels`, so
   a new dimension needs no schema change. Limits keep a row small and bound the index.
3. **`source` is metadata, not eligibility.** Whether a promotion applies to a cart from a
   given source is a rule (`TagsMatch` on cart tags); `source` only partitions listings.
4. **Filtering happens in the application layer** over the metadata map; the catalogue is
   small (hundreds of promotions). A Postgres expression index on `body->>'source'` is
   added anyway.
5. **Archiving keeps the row** (foreign key to `promotions`, which are archived, not
   deleted); listings join on `NOT archived`, and `GET .../metadata` on an archived
   promotion is `404` like the promotion itself.

## Risks / Trade-offs

- [Clients expecting metadata inside `Promotion`] → the generated client exposes both
  resources; a combined read is one extra call. Revisit if the admin UI needs it.
