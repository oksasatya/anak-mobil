DROP TABLE IF EXISTS users;

-- `citext` is deliberately not dropped, for the same reason as `vector`:
-- dropping an extension cascades to every column using its type, so
-- reverting this migration alone would break any later table that adopted it.
