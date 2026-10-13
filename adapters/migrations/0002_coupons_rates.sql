-- Coupons, exchange rates and the customer of a redemption (OpenSpec:
-- promotion-rules-coupons-currency).

CREATE TABLE coupons (
    code        TEXT   PRIMARY KEY,
    body        JSONB  NOT NULL,         -- Coupon
    updated_at  BIGINT NOT NULL
);

CREATE TABLE exchange_rates (
    from_currency TEXT   NOT NULL,
    to_currency   TEXT   NOT NULL,
    numerator     BIGINT NOT NULL,
    denominator   BIGINT NOT NULL,
    updated_at    BIGINT NOT NULL,
    PRIMARY KEY (from_currency, to_currency)
);

ALTER TABLE redemptions ADD COLUMN customer_id TEXT;
CREATE INDEX redemptions_customer_idx ON redemptions (customer_id) WHERE customer_id IS NOT NULL;
