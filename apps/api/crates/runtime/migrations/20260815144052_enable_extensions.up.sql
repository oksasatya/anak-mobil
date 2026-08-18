-- Extensions, and the conventions every later migration follows.
--
-- pgvector is enabled here rather than in the retrieval story so that no
-- migration later has to run CREATE EXTENSION, which needs privileges an
-- application role should not hold in production.

CREATE EXTENSION IF NOT EXISTS vector;

-- Every table carries created_at and updated_at. Keeping updated_at correct
-- from application code means every writer has to remember; a trigger means
-- none of them do, including a manual UPDATE run during an incident.
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

COMMENT ON FUNCTION set_updated_at() IS
    'Trigger function: stamps updated_at on every UPDATE. Attach to every table that has the column.';
