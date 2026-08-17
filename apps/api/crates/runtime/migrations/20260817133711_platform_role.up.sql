-- Two platform roles, and an append-only record of every change between them.
--
-- "Platform role" is one of three unrelated senses of the word "role" in this
-- codebase and they are never stored in the same column — see CONTEXT.md.
-- `runtime::Role` is the PROCESS role (web | worker | migrate), a property of
-- the deployment rather than of any person. Community membership will bring a
-- third sense, confined to one community, which grants nothing platform-wide.

CREATE TYPE platform_role AS ENUM ('user', 'admin');

-- Defaults to `user`, and no sign-up path writes this column, so the admin
-- role cannot be reached through registration.
ALTER TABLE users
    ADD COLUMN platform_role platform_role NOT NULL DEFAULT 'user';

COMMENT ON COLUMN users.platform_role IS
    'What this account may do across the whole platform. Read fresh on every admin request; never cached in a session.';

-- Every promotion, demotion, and bootstrap.
--
-- Append-only, and enforced by the trigger below rather than by a comment.
-- `part_merges` claims append-only in a comment while an UPDATE was verified
-- to rewrite its history. A comment is not a constraint. Here the guarantee is
-- in the schema from the first migration.
CREATE TABLE role_changes (
    id             UUID PRIMARY KEY,

    -- NULL for a bootstrap: `anakmobil grant-admin` has no signed-in human
    -- behind it.
    --
    -- RESTRICT rather than SET NULL, and the difference is the whole reason
    -- this table can carry foreign keys at all. PostgreSQL performs a
    -- referential action as an ordinary UPDATE or DELETE on the CHILD row,
    -- which the append-only trigger below rejects — so SET NULL would make
    -- every parent DELETE fail and account deletion impossible. That is the
    -- exact defect class this project has already shipped once. RESTRICT never
    -- writes to the child, so the trigger never sees it.
    --
    -- The usual convention for an audit table is to drop the keys entirely.
    -- What makes RESTRICT strictly better here is ADR-0001: deleted accounts
    -- are retained rather than erased, so the row a key points at is
    -- unconditionally present. Identity is therefore a join, and this table
    -- stores no copy of anybody's email. A future hard-erasure path meets a
    -- refusal here and has to make a conscious decision instead of silently
    -- orphaning the trail.
    actor_id       UUID          REFERENCES users (id) ON DELETE RESTRICT,
    target_user_id UUID NOT NULL REFERENCES users (id) ON DELETE RESTRICT,

    from_role      platform_role NOT NULL,
    to_role        platform_role NOT NULL,

    -- Why, in the admin's or operator's own words. Never logged, never
    -- returned in a response — the response answers what changed, not who
    -- anybody is or what they were thinking.
    reason         TEXT          NOT NULL,

    created_at     TIMESTAMPTZ   NOT NULL DEFAULT now(),

    -- A row claiming a change that did not happen is a lie about the past.
    -- The use case has an explicit no-op branch so this never surfaces to a
    -- client as a 500; the constraint is the backstop for a path that forgets.
    CONSTRAINT role_changes_real_change CHECK (from_role <> to_role)
);

-- No `updated_at` and no `set_updated_at` trigger, deliberately, against the
-- convention every other table here follows. A row is never updated — the
-- trigger below refuses — so a column recording when it last was would be a
-- claim the schema contradicts.

-- The only query that will matter: this person's role history, newest first.
-- It is also the mandatory foreign-key index — PostgreSQL indexes primary keys
-- and unique constraints and never a foreign key, and RESTRICT turns every
-- parent delete into a real lookup here.
CREATE INDEX role_changes_target_idx
    ON role_changes (target_user_id, created_at DESC, id);

-- Partial: bootstrap rows have no actor and there is no query for them.
CREATE INDEX role_changes_actor_idx
    ON role_changes (actor_id)
    WHERE actor_id IS NOT NULL;

CREATE OR REPLACE FUNCTION role_changes_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'role_changes is append-only: % is not permitted', TG_OP;
END;
$$ LANGUAGE plpgsql;

-- BEFORE, not AFTER: the row must never be written at all, and an AFTER
-- trigger raising would still have done the work first. UPDATE and DELETE
-- together, because removing history and rewriting it are the same defect.
CREATE TRIGGER role_changes_append_only
    BEFORE UPDATE OR DELETE ON role_changes
    FOR EACH ROW EXECUTE FUNCTION role_changes_append_only();

COMMENT ON TABLE role_changes IS
    'Append-only, enforced by the role_changes_append_only trigger rather than by this comment. Identity is a join to users; no name or email is copied here.';
