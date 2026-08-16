-- The catalog's escape hatch.
--
-- The catalog will never be complete. Indonesia has grey imports, facelifts
-- nobody catalogued, and models that lasted two years — so a person whose car
-- is missing needs somewhere to say so, or the catalog can only ever describe
-- cars that were already in it.
--
-- Curation is manual and lands with the backoffice. This table is the queue.

CREATE TYPE suggestion_status AS ENUM ('pending', 'accepted', 'rejected', 'duplicate');

CREATE TABLE catalog_suggestions (
    id            UUID PRIMARY KEY,

    -- ON DELETE SET NULL, not CASCADE. What was suggested — "Datsun Go+ Panca
    -- 2018" — is catalog work, not personal data, and losing it because the
    -- person who reported it later closed their account would throw away the
    -- only record that the gap exists.
    suggested_by  UUID        REFERENCES users (id) ON DELETE SET NULL,

    brand         TEXT        NOT NULL,
    model         TEXT        NOT NULL,
    generation    TEXT,
    variant       TEXT,
    year          SMALLINT,
    note          TEXT,

    status        suggestion_status NOT NULL DEFAULT 'pending',
    reviewed_at   TIMESTAMPTZ,

    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT catalog_suggestions_brand_present CHECK (length(btrim(brand)) > 0),
    CONSTRAINT catalog_suggestions_model_present CHECK (length(btrim(model)) > 0),
    CONSTRAINT catalog_suggestions_year_plausible
        CHECK (year IS NULL OR year BETWEEN 1900 AND 2100),
    -- A decision needs a date, and a date without a decision is a half-written
    -- review. Neither, or both.
    CONSTRAINT catalog_suggestions_review_complete
        CHECK ((status = 'pending') = (reviewed_at IS NULL))
);

-- The curation queue: oldest pending first.
CREATE INDEX catalog_suggestions_queue_idx
    ON catalog_suggestions (status, created_at)
    WHERE status = 'pending';

-- Deliberately NOT unique. The same missing model suggested forty times is
-- the strongest demand signal the catalog will ever get, and collapsing those
-- into one row throws that signal away. Grouping happens when the queue is
-- read, not when it is written.
CREATE INDEX catalog_suggestions_subject_idx ON catalog_suggestions (brand, model);

CREATE TRIGGER catalog_suggestions_set_updated_at
    BEFORE UPDATE ON catalog_suggestions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
