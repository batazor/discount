-- Exchange-rate source and the origin of each stored rate (OpenSpec: exchange-rate-source).
-- Existing rates were all typed by operators, so `manual` is the true default.

CREATE TABLE exchange_rate_source (
    id              SMALLINT PRIMARY KEY CHECK (id = 1),
    body            JSONB    NOT NULL,       -- ExchangeRateSource
    updated_at      BIGINT   NOT NULL,
    last_attempt_at BIGINT,
    last_success_at BIGINT,
    last_error      TEXT,
    last_count      BIGINT   NOT NULL DEFAULT 0
);

ALTER TABLE exchange_rates ADD COLUMN origin TEXT NOT NULL DEFAULT 'manual' CHECK (origin IN ('manual', 'source'));
ALTER TABLE exchange_rates ADD COLUMN fetched_at BIGINT;
