-- The job queue. One table, one `kind`, one JSONB payload.
--
-- Postgres rather than Redis, and that is settled in README.md: the failures that
-- matter -- a worker dying mid-job, a duplicate push notification, a poisoned message
-- -- need leases, retries, and dead-lettering whatever the broker is. Redis removes
-- none of that work and adds a service to operate.
--
-- One table rather than one per kind, because the acceptance criteria are about
-- mechanics and not about typing. A table per kind earns its place when a kind needs a
-- column of its own; none does.

CREATE TYPE job_state AS ENUM ('queued', 'leased', 'done', 'dead');

CREATE TABLE jobs (
    id            UUID PRIMARY KEY,

    kind          TEXT        NOT NULL,
    payload       JSONB       NOT NULL DEFAULT '{}'::jsonb,

    -- A stable logical name for the SIDE EFFECT this job produces, when it has one
    -- outside the database.
    --
    -- "Processing twice has the same effect as once" is satisfied for a media file by
    -- overwriting a deterministic object key. That argument does not transfer to a push
    -- notification: if the worker dies after the provider accepted the push but before
    -- the job was marked done, the retry sends a second one and no object key helps.
    --
    -- So the queue is AT-LEAST-ONCE and says so. Two halves, and only the first is
    -- built here:
    --
    --   * ENQUEUE side, enforced below by jobs_one_live_per_effect: while a job with
    --     this key is queued or leased, enqueuing it again does nothing. That closes
    --     the double-`POST /media/{id}/complete` case without any consumer code.
    --   * EFFECT side, the consumer's own job: record the key in the SAME transaction
    --     as the effect, and use the provider's idempotency key where one exists. The
    --     notification ticket is the first consumer that needs it, and it brings its
    --     own table -- building one here would be building another ticket's schema.
    effect_key    TEXT,

    state         job_state   NOT NULL DEFAULT 'queued',

    -- When this job is next eligible. Set forward by the backoff on every transient
    -- failure.
    run_at        TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Incremented by the CLAIM, not by the settle. A job whose worker is killed mid-run
    -- has still burned an attempt, which is the only thing that makes the cap bite for a
    -- payload that crashes the process rather than returning an error.
    attempts      INTEGER     NOT NULL DEFAULT 0,
    max_attempts  INTEGER     NOT NULL DEFAULT 8,

    -- The lease. NOT NULL exactly while state = 'leased' -- see the constraint below,
    -- which is what makes COALESCE(leased_until, run_at) mean "next claimable at".
    leased_until  TIMESTAMPTZ,
    -- Which worker process holds it. A UUID minted at process start and logged once, so
    -- an operator can join this to a log line without this table needing a hostname.
    leased_by     UUID,

    -- What the last failure said, truncated by the application. Never a payload.
    last_error    TEXT,

    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT jobs_kind_present CHECK (length(btrim(kind)) > 0),
    -- Mirrors jobs_kind_present. Without it an empty string is a valid effect_key that
    -- participates in the uniqueness below, so one bad `format!` turns the queue into a
    -- black hole: an unrelated job's INSERT ... ON CONFLICT (effect_key) matches the
    -- empty-string row and returns "0 rows" -- silently discarded, no error, no counter.
    -- 200 rather than unbounded: an incompressible key over ~2704 bytes fails the btree
    -- index outright ("index row size exceeds btree version 4 maximum"), and a flat
    -- character limit here is what stops that failure from being data-dependent.
    CONSTRAINT jobs_effect_key_shape
        CHECK (effect_key IS NULL OR length(btrim(effect_key)) BETWEEN 1 AND 200),
    CONSTRAINT jobs_attempts_sane CHECK (attempts >= 0 AND max_attempts >= 1),
    -- The invariant the claim index depends on. Without it a released job could keep a
    -- stale leased_until, and COALESCE(leased_until, run_at) would return a moment in
    -- the past that has nothing to do with when the job is due.
    CONSTRAINT jobs_lease_matches_state
        CHECK ((state = 'leased') = (leased_until IS NOT NULL))
);

-- "When is this row next claimable", indexed.
--
-- PARTIAL on the two live states, so the cost of claiming depends on how many jobs are
-- PENDING and not on how many have ever run. Done and dead rows are not in this index
-- at all, which is why a growing dead-letter set cannot slow the hot path down.
CREATE INDEX jobs_claimable_idx
    ON jobs (COALESCE(leased_until, run_at))
    WHERE state IN ('queued', 'leased');

-- At most one LIVE job per effect key. Same shape as part_merges_one_live_per_source.
--
-- Scoped to the live states on purpose: once a job is done or dead, the same logical
-- effect may legitimately be requested again -- a re-upload of the same media, a
-- notification for a second event. Terminal "has this effect ever happened" is the
-- consumer's question, answered in the consumer's own transaction.
CREATE UNIQUE INDEX jobs_one_live_per_effect
    ON jobs (effect_key)
    WHERE effect_key IS NOT NULL AND state IN ('queued', 'leased');

-- AC5's second number, as an index-only scan rather than a sequential scan of every job
-- the platform has ever run.
CREATE INDEX jobs_dead_idx
    ON jobs (created_at)
    WHERE state = 'dead';

CREATE TRIGGER jobs_set_updated_at
    BEFORE UPDATE ON jobs
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

COMMENT ON TABLE jobs IS
    'At-least-once job queue. Claimed with FOR UPDATE SKIP LOCKED; a lease that expires returns the job; attempts are capped and the cap is terminal (dead). A consumer with a side effect outside this database is responsible for its own dedupe -- see effect_key.';

COMMENT ON COLUMN jobs.effect_key IS
    'Stable logical name for the side effect. Unique among queued and leased rows, so enqueuing the same live effect twice is a no-op. Terminal dedupe is the consumer''s, recorded in the same transaction as the effect.';

COMMENT ON COLUMN jobs.attempts IS
    'Incremented when the job is CLAIMED, not when it fails, so a worker killed mid-run still burns an attempt.';
