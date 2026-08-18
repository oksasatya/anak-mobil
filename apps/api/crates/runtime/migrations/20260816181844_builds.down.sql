ALTER TABLE vehicles DROP COLUMN IF EXISTS cost_visibility;

DROP TABLE IF EXISTS modifications;
DROP TABLE IF EXISTS build_photos;
DROP TABLE IF EXISTS builds;

DROP TYPE IF EXISTS visibility;
