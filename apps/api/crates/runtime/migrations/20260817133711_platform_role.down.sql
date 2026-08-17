-- The table before the function: dropping the table drops its trigger, and
-- dropping the function first would fail while the trigger still references it.
DROP TABLE IF EXISTS role_changes;
DROP FUNCTION IF EXISTS role_changes_append_only();

-- The column before the type. `DROP TYPE platform_role` while `users` still
-- has a column of that type fails with "cannot drop type platform_role because
-- other objects depend on it", which leaves the revert half-applied.
ALTER TABLE users DROP COLUMN IF EXISTS platform_role;
DROP TYPE IF EXISTS platform_role;
