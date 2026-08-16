-- Narrowing rounds any stored two-decimal bore to one place. Postgres does
-- that silently, which is the defect this migration existed to remove — so
-- the reversal is genuinely lossy and worth noticing before running it.
ALTER TABLE parts
    ALTER COLUMN center_bore_mm TYPE NUMERIC(5, 1);
