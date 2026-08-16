-- The merge log. Append-only, and the reason it exists rather than a pointer.
--
-- The first design set `merged_into` on the losing part and resolved it at
-- read time, claiming undo was free because no row was ever rewritten. It does
-- not hold. Merge A → B, then merge B → C:
--
--   * Leave both pointers and the result is a chain A → B → C. A count for C
--     that resolves one hop misses everything under A.
--   * Flatten A → C when B → C is applied, and undoing B → C cannot restore
--     A → B — that edge no longer exists anywhere. "Undo restores the previous
--     state exactly" is simply false.
--
-- Concurrency breaks it a second way: two curators merging A → B and A → C
-- lock different rows, so the second silently overwrites the first and neither
-- operation reports that anything was lost.
--
-- So the log is the truth and `parts.canonical_part_id` is a one-hop cache
-- recomputed from it. This is not event sourcing; it is the operation history
-- that an undoable curation action requires anyway.

CREATE TABLE part_merges (
    id             UUID PRIMARY KEY,

    -- RESTRICT on both: a part that appears in the log cannot be deleted, and
    -- neither side should be, because both are still referenced by
    -- modifications that were never rewritten.
    source_part_id UUID        NOT NULL REFERENCES parts (id) ON DELETE RESTRICT,
    target_part_id UUID        NOT NULL REFERENCES parts (id) ON DELETE RESTRICT,

    -- SET NULL, not CASCADE. Who did it is provenance; losing the row because
    -- a curator later closed their account would throw away the history undo
    -- reads to know what the previous state was.
    merged_by      UUID        REFERENCES users (id) ON DELETE SET NULL,
    merged_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    undone_by      UUID        REFERENCES users (id) ON DELETE SET NULL,
    undone_at      TIMESTAMPTZ,

    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT part_merges_not_self CHECK (source_part_id <> target_part_id),
    -- undone_at alone is the state machine ("no status enum" per the plan's
    -- minimality check); undone_by is provenance only, same as merged_by.
    -- The check is one-directional, not an equivalence, on purpose: an
    -- equivalence would make ON DELETE SET NULL on undone_by impossible to
    -- honor. Deleting the user who undid a merge sets undone_by to NULL
    -- without touching undone_at — and that is the exact case this table
    -- exists to survive, not to reject. What the check still forbids is the
    -- genuinely half-written state: undone_by recorded with no undone_at.
    CONSTRAINT part_merges_undo_complete
        CHECK (undone_by IS NULL OR undone_at IS NOT NULL)
);

-- One live merge per source. Two standing merges out of one part is the state
-- that makes "resolve one hop" ambiguous, and it is what two curators racing
-- on A → B and A → C would otherwise produce.
CREATE UNIQUE INDEX part_merges_one_live_per_source
    ON part_merges (source_part_id)
    WHERE undone_at IS NULL;

CREATE INDEX part_merges_target_idx ON part_merges (target_part_id);
CREATE INDEX part_merges_merged_by_idx ON part_merges (merged_by)
    WHERE merged_by IS NOT NULL;
CREATE INDEX part_merges_undone_by_idx ON part_merges (undone_by)
    WHERE undone_by IS NOT NULL;

CREATE TRIGGER part_merges_set_updated_at
    BEFORE UPDATE ON part_merges
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

COMMENT ON TABLE part_merges IS
    'Append-only. Only undone_by and undone_at are ever updated; a merge is never deleted, because undo reads this to know what the previous state was.';
