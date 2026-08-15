DROP FUNCTION IF EXISTS set_updated_at();

-- `vector` is deliberately not dropped. Dropping an extension cascades to
-- every column using its type, so a rollback of this migration alone would
-- silently destroy embeddings belonging to migrations that are still applied.
-- Removing it is a manual decision, made once, with the data in front of you.
