-- Allowlists: customers a promotion is open to (OpenSpec: customer-attributes).
-- Pricing never reads the set; the service puts the matching promotion ids into
-- customer.allowlisted.

CREATE TABLE promotion_allowlist (
    promotion_id TEXT   NOT NULL REFERENCES promotions (promotion_id) ON DELETE CASCADE,
    customer_id  TEXT   NOT NULL,
    created_at   BIGINT NOT NULL,
    PRIMARY KEY (promotion_id, customer_id)
);

CREATE INDEX promotion_allowlist_customer_idx ON promotion_allowlist (customer_id);
