-- Accounts.
--
-- The table holds what authentication needs and nothing else. A profile —
-- display name, photo, preferences — belongs to the story that renders one.

-- Case-insensitive text, so `Budi@Example.com` and `budi@example.com` are the
-- same account. Doing this with `LOWER(email)` and a functional index instead
-- would work until the first query forgot the `LOWER`, and that query would
-- create a duplicate account rather than fail.
CREATE EXTENSION IF NOT EXISTS citext;

CREATE TABLE users (
    id            UUID PRIMARY KEY,
    email         CITEXT      NOT NULL UNIQUE,
    -- A PHC string: `$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`. The
    -- parameters travel with the hash, so raising them later does not
    -- invalidate existing passwords — an old hash still verifies against the
    -- settings it was made with, and can be re-hashed on the next successful
    -- login.
    password_hash TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Cheap sanity, not validation. Real address validation is a delivery
    -- problem, not a regex; the constraint exists to stop obvious junk
    -- reaching the unique index.
    CONSTRAINT users_email_shape CHECK (email ~ '^[^@[:space:]]+@[^@[:space:]]+\.[^@[:space:]]+$')
);

CREATE TRIGGER users_set_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

COMMENT ON COLUMN users.password_hash IS
    'argon2id PHC string. Never logged, never returned in a response.';
