DROP INDEX IF EXISTS users_username_key;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_username_shape;
ALTER TABLE users DROP COLUMN IF EXISTS display_name;
ALTER TABLE users DROP COLUMN IF EXISTS username;
