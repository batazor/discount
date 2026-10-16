-- Grant rules and grants (OpenSpec: coupon-lifecycle).

CREATE TABLE grant_rules (
    rule_id     TEXT   PRIMARY KEY,
    body        JSONB  NOT NULL,         -- GrantRule
    updated_at  BIGINT NOT NULL
);

-- One coupon per (trigger, order_id): the idempotency record of `grant`.
CREATE TABLE grants (
    trigger     TEXT   NOT NULL,
    order_id    TEXT   NOT NULL,
    code        TEXT   NOT NULL,
    created_at  BIGINT NOT NULL,
    PRIMARY KEY (trigger, order_id)
);

-- Counting coupon uses looks up redemptions by the codes their cart presented.
CREATE INDEX redemptions_coupon_codes_idx ON redemptions USING GIN ((input -> 'cart' -> 'coupon_codes'));
