-- Saved coupons and the customer binding index (OpenSpec: customer-coupons).

CREATE TABLE saved_coupons (
    customer_id TEXT   NOT NULL,
    code        TEXT   NOT NULL,
    created_at  BIGINT NOT NULL,
    PRIMARY KEY (customer_id, code)
);

CREATE INDEX coupons_customer_idx ON coupons ((body ->> 'customer_id')) WHERE body ->> 'customer_id' IS NOT NULL;
