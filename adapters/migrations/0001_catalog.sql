-- Promotion catalogue and redemptions (OpenSpec: promotion-catalog, redemption).
-- Bodies are the ubiquitous-language JSON; budgets are the only mutable pricing state.

CREATE TABLE pricing_policy (
    id          SMALLINT PRIMARY KEY CHECK (id = 1),
    body        JSONB    NOT NULL,
    updated_at  BIGINT   NOT NULL
);

CREATE TABLE stacking_groups (
    group_id    TEXT   PRIMARY KEY,
    body        JSONB  NOT NULL,
    updated_at  BIGINT NOT NULL
);

CREATE TABLE promotions (
    promotion_id TEXT    PRIMARY KEY,
    body         JSONB   NOT NULL,       -- Promotion without remaining_budget
    archived     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   BIGINT  NOT NULL,
    updated_at   BIGINT  NOT NULL
);

CREATE TABLE promotion_budgets (
    promotion_id TEXT   PRIMARY KEY REFERENCES promotions (promotion_id) ON DELETE CASCADE,
    remaining    BIGINT NOT NULL,
    currency     TEXT   NOT NULL,
    updated_at   BIGINT NOT NULL
);

CREATE TABLE redemptions (
    order_id    TEXT   PRIMARY KEY,
    status      TEXT   NOT NULL CHECK (status IN ('reserved', 'confirmed', 'cancelled')),
    input       JSONB  NOT NULL,         -- PricingInput, the replay record
    result      JSONB  NOT NULL,         -- PricingResult
    created_at  BIGINT NOT NULL,
    updated_at  BIGINT NOT NULL
);
