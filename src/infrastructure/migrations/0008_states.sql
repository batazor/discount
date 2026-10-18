-- Operator switch on promotions and coupons (OpenSpec: catalogue-states). Paused records
-- stay in the catalogue and leave every pricing snapshot; archiving remains terminal.

ALTER TABLE promotions ADD COLUMN state TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'paused'));
ALTER TABLE coupons ADD COLUMN state TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'paused'));
