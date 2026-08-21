-- The public half of an account: the name people address, and the name they
-- show. Both nullable, and both for the same reason — `NOT NULL UNIQUE` cannot
-- be added to a table that already has rows without a backfill nobody has
-- designed. There are no production rows today; the migration is honest anyway.

-- CITEXT for consistency with `email`, so `Budi` and `budi` cannot both be
-- claimed. CITEXT is NOT the validator: case-insensitive uniqueness is a
-- different thing from a character rule, and the rule lives in exactly one
-- place — `anakmobil_domain::identity::username::canonicalise`.
ALTER TABLE users ADD COLUMN username CITEXT;

-- Not unique, not an identifier, and deliberately plain TEXT. Two people may
-- both be "Budi"; what distinguishes them is the username. Collected during
-- onboarding, which by definition happens after the row exists — so an account
-- that has not finished onboarding has NULL here, and that is one of the two
-- facts `GET /me` reports.
ALTER TABLE users ADD COLUMN display_name TEXT;

-- Cheap sanity, exactly like `users_email_shape` — a floor, not the validator.
-- It knows nothing about consecutive dots or edge punctuation; the canonicaliser
-- does, and duplicating those rules here would create a second copy free to
-- drift from the first.
--
-- The `::text` cast is load-bearing. citext overloads `~` to be
-- case-insensitive, so `username ~ '^[a-z0-9._]{3,30}$'` would happily accept
-- 'BUDI' — precisely the value this constraint exists to keep out of the table.
ALTER TABLE users
    ADD CONSTRAINT users_username_shape
    CHECK (username IS NULL OR username::text ~ '^[a-z0-9._]{3,30}$');

-- Named explicitly rather than left to PostgreSQL, because the register handler
-- matches on this string to report which field collided. A unique violation
-- reports the INDEX name, so the name is part of the contract.
CREATE UNIQUE INDEX users_username_key ON users (username) WHERE username IS NOT NULL;

COMMENT ON COLUMN users.username IS
    'The public namespace and profile address (/@username). Canonicalised server-side before it ever reaches this column; CITEXT provides case-insensitive uniqueness, not validation.';
COMMENT ON COLUMN users.display_name IS
    'What a person calls themselves. Not unique, not an identifier, NULL until onboarding collects it.';
