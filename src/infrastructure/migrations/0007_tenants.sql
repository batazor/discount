-- Tenants: a hard partition of every record (OpenSpec: access-control, ADR 12).
-- Existing rows land in the tenant 'default'.

CREATE TABLE tenants (
    tenant_id   TEXT   PRIMARY KEY,
    name        TEXT   NOT NULL,
    created_at  BIGINT NOT NULL
);
INSERT INTO tenants (tenant_id, name, created_at) VALUES ('default', 'Default', 0);

-- pricing_policy: one row per tenant instead of the single row with id = 1.
ALTER TABLE pricing_policy ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES tenants (tenant_id) ON DELETE CASCADE;
ALTER TABLE pricing_policy DROP CONSTRAINT pricing_policy_pkey;
ALTER TABLE pricing_policy DROP COLUMN id;
ALTER TABLE pricing_policy ADD PRIMARY KEY (tenant_id);

ALTER TABLE stacking_groups ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES tenants (tenant_id) ON DELETE CASCADE;
ALTER TABLE stacking_groups DROP CONSTRAINT stacking_groups_pkey;
ALTER TABLE stacking_groups ADD PRIMARY KEY (tenant_id, group_id);

-- promotions and the tables that reference them: rebuild the foreign keys on (tenant_id, promotion_id).
ALTER TABLE promotion_budgets DROP CONSTRAINT promotion_budgets_promotion_id_fkey;
ALTER TABLE promotion_metadata DROP CONSTRAINT promotion_metadata_promotion_id_fkey;
ALTER TABLE promotion_allowlist DROP CONSTRAINT promotion_allowlist_promotion_id_fkey;

ALTER TABLE promotions ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES tenants (tenant_id) ON DELETE CASCADE;
ALTER TABLE promotions DROP CONSTRAINT promotions_pkey;
ALTER TABLE promotions ADD PRIMARY KEY (tenant_id, promotion_id);

ALTER TABLE promotion_budgets ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE promotion_budgets DROP CONSTRAINT promotion_budgets_pkey;
ALTER TABLE promotion_budgets ADD PRIMARY KEY (tenant_id, promotion_id);
ALTER TABLE promotion_budgets ADD FOREIGN KEY (tenant_id, promotion_id) REFERENCES promotions (tenant_id, promotion_id) ON DELETE CASCADE;

ALTER TABLE promotion_metadata ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE promotion_metadata DROP CONSTRAINT promotion_metadata_pkey;
ALTER TABLE promotion_metadata ADD PRIMARY KEY (tenant_id, promotion_id);
ALTER TABLE promotion_metadata ADD FOREIGN KEY (tenant_id, promotion_id) REFERENCES promotions (tenant_id, promotion_id) ON DELETE CASCADE;
DROP INDEX promotion_metadata_source_idx;
CREATE INDEX promotion_metadata_source_idx ON promotion_metadata (tenant_id, (body ->> 'source'));

ALTER TABLE promotion_allowlist ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE promotion_allowlist DROP CONSTRAINT promotion_allowlist_pkey;
ALTER TABLE promotion_allowlist ADD PRIMARY KEY (tenant_id, promotion_id, customer_id);
ALTER TABLE promotion_allowlist ADD FOREIGN KEY (tenant_id, promotion_id) REFERENCES promotions (tenant_id, promotion_id) ON DELETE CASCADE;
DROP INDEX promotion_allowlist_customer_idx;
CREATE INDEX promotion_allowlist_customer_idx ON promotion_allowlist (tenant_id, customer_id);

ALTER TABLE redemptions ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES tenants (tenant_id) ON DELETE CASCADE;
ALTER TABLE redemptions DROP CONSTRAINT redemptions_pkey;
ALTER TABLE redemptions ADD PRIMARY KEY (tenant_id, order_id);
DROP INDEX redemptions_customer_idx;
CREATE INDEX redemptions_customer_idx ON redemptions (tenant_id, customer_id) WHERE customer_id IS NOT NULL;

ALTER TABLE coupons ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES tenants (tenant_id) ON DELETE CASCADE;
ALTER TABLE coupons DROP CONSTRAINT coupons_pkey;
ALTER TABLE coupons ADD PRIMARY KEY (tenant_id, code);
DROP INDEX coupons_customer_idx;
CREATE INDEX coupons_customer_idx ON coupons (tenant_id, (body ->> 'customer_id')) WHERE body ->> 'customer_id' IS NOT NULL;

ALTER TABLE exchange_rates ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES tenants (tenant_id) ON DELETE CASCADE;
ALTER TABLE exchange_rates DROP CONSTRAINT exchange_rates_pkey;
ALTER TABLE exchange_rates ADD PRIMARY KEY (tenant_id, from_currency, to_currency);

ALTER TABLE grant_rules ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES tenants (tenant_id) ON DELETE CASCADE;
ALTER TABLE grant_rules DROP CONSTRAINT grant_rules_pkey;
ALTER TABLE grant_rules ADD PRIMARY KEY (tenant_id, rule_id);

ALTER TABLE grants ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES tenants (tenant_id) ON DELETE CASCADE;
ALTER TABLE grants DROP CONSTRAINT grants_pkey;
ALTER TABLE grants ADD PRIMARY KEY (tenant_id, trigger, order_id);

ALTER TABLE saved_coupons ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default' REFERENCES tenants (tenant_id) ON DELETE CASCADE;
ALTER TABLE saved_coupons DROP CONSTRAINT saved_coupons_pkey;
ALTER TABLE saved_coupons ADD PRIMARY KEY (tenant_id, customer_id, code);
