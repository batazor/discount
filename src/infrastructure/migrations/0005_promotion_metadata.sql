-- Promotion metadata (OpenSpec: promotion-metadata). Never read by pricing.

CREATE TABLE promotion_metadata (
    promotion_id TEXT   PRIMARY KEY REFERENCES promotions (promotion_id) ON DELETE CASCADE,
    body         JSONB  NOT NULL,         -- PromotionMetadata
    updated_at   BIGINT NOT NULL
);

CREATE INDEX promotion_metadata_source_idx ON promotion_metadata ((body ->> 'source'));
