-- Builds: the thing an owner chooses to publish.
--
-- A vehicle is the private ownership record; a build is what its owner shares,
-- with its own visibility and its own lifecycle. An earlier draft justified
-- the split as a privacy boundary — that was wrong, and correcting it matters:
-- privacy already comes from vehicle_private being a separate table, and a
-- public build query still joins vehicles to read the variant. The split earns
-- itself on publication lifecycle, and on nothing else.

-- One enum, two columns. builds.visibility answers "who sees this build at
-- all"; vehicles.cost_visibility answers "who sees what it cost". They are
-- separate settings because wanting to show the car without showing the money
-- is an ordinary thing to want — and if one setting forced them together, the
-- one people would give up is the sharing.
CREATE TYPE visibility AS ENUM ('private', 'community', 'public');

CREATE TABLE builds (
    id          UUID PRIMARY KEY,

    -- UNIQUE is a product decision: one current build per car. It forecloses
    -- separate street and track builds on one vehicle. Written down here so
    -- that reversing it later is a decision rather than a surprise — dropping
    -- the constraint is one migration, but every query that assumed one build
    -- per car is not.
    vehicle_id  UUID        NOT NULL UNIQUE REFERENCES vehicles (id) ON DELETE CASCADE,

    notes       TEXT,
    visibility  visibility  NOT NULL DEFAULT 'private',

    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The table exists; the upload path does not, and that is deliberate. Upload
-- is its own problem — presigned URLs, size limits, EXIF stripping (a photo of
-- a car carries the coordinates of the house it is parked at), moderation.
-- Folding it in here would rush the privacy half of it.
CREATE TABLE build_photos (
    id         UUID PRIMARY KEY,
    build_id   UUID        NOT NULL REFERENCES builds (id) ON DELETE CASCADE,

    -- The object storage key, not a URL. A URL bakes in a bucket, a region,
    -- and a CDN hostname, and all three change without the photo changing.
    object_key TEXT        NOT NULL,
    position   INTEGER     NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT build_photos_object_key_present CHECK (length(btrim(object_key)) > 0)
);

CREATE TABLE modifications (
    id          UUID PRIMARY KEY,
    build_id    UUID        NOT NULL REFERENCES builds (id) ON DELETE CASCADE,

    -- NOT NULL, and there is no free-text alternative. A NULL part_id has no
    -- queue row, no suggester, and no per-part completeness state, so the
    -- "enters curation" half of AC3 would quietly not happen. An unknown part
    -- becomes a pending parts row in the same transaction instead.
    --
    -- ON DELETE RESTRICT: a part in use cannot be removed from under its
    -- users. Merging is how a duplicate is resolved, and merging never touches
    -- this column — that is what keeps evidence alive through a bad merge.
    part_id     UUID        NOT NULL REFERENCES parts (id) ON DELETE RESTRICT,

    -- No `category` column. The category lives on the part; storing it twice
    -- only creates a way for the two to disagree, and the one on the
    -- modification is the copy that would go stale.

    install_date DATE,
    mileage_km   INTEGER,
    cost         NUMERIC(14, 2),
    garage_name  TEXT,
    notes        TEXT,

    -- Removal sets this rather than deleting the row. A removed modification
    -- is still evidence that the part fitted the car, which is exactly what
    -- ever_installed_build_count counts.
    removed_at  TIMESTAMPTZ,

    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT modifications_mileage_not_negative
        CHECK (mileage_km IS NULL OR mileage_km >= 0),
    CONSTRAINT modifications_cost_not_negative
        CHECK (cost IS NULL OR cost >= 0),
    CONSTRAINT modifications_install_not_in_the_future
        CHECK (install_date IS NULL OR install_date <= CURRENT_DATE + 1)
);

-- Every foreign key gets an index. These two carry slice 3's batched reads:
-- WHERE build_id = ANY($1) is an index scan with them and a sequential scan
-- without, which is AC5 failing on the index rather than on the query count.
-- No index on builds.vehicle_id: the UNIQUE constraint already built one, and
-- a second btree on the same column is maintained on every write and chosen by
-- nothing. The rule is that Postgres never indexes a foreign key — a foreign
-- key that is ALSO unique is the exception, and this is it.
CREATE INDEX build_photos_build_idx    ON build_photos (build_id, position, id);

-- The sort is in the key on purpose. `modifications_for` orders by
-- (build_id, install_date DESC NULLS LAST, id); leaving install_date out makes
-- the planner add an Incremental Sort on top of the scan, which is cheap per
-- build and free to avoid at creation. `service_records_timeline_idx` is the
-- same shape solved the same way.
CREATE INDEX modifications_build_idx   ON modifications (build_id, install_date DESC NULLS LAST, id);

-- "Which builds use this part" — the query behind both evidence counts.
CREATE INDEX modifications_part_idx    ON modifications (part_id);

-- The community-facing list: everything not private, newest first.
--
-- `visibility` is in the PREDICATE and deliberately not in the KEY. A btree
-- sorts by its first column, so (visibility, id DESC) yields "all community by
-- id, then all public by id" — two runs, not one descending stream, and
-- merging them costs a full sort no LIMIT can stop early. Measured: the
-- planner declines that index entirely and takes a backward pkey scan.
--
-- With `visibility` demoted to the predicate, index order IS query order.
--
-- Stated honestly rather than overclaimed: the list also ORs in the caller's
-- own private builds, and `owner_id` lives on `vehicles`, so no index on
-- `builds` alone can serve that whole predicate. Do not write a complexity
-- comment claiming otherwise. When the feed is slow enough to matter, the fix
-- is a UNION ALL of "visible" and "mine", not another index here.
CREATE INDEX builds_visible_idx ON builds (id DESC)
    WHERE visibility <> 'private';

CREATE TRIGGER builds_set_updated_at
    BEFORE UPDATE ON builds
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER modifications_set_updated_at
    BEFORE UPDATE ON modifications
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Who sees what the car cost — service costs and modification costs alike,
-- since both are costs of the same car.
--
-- This also closes the half of AM-360 AC3 that currently holds only by
-- construction: today no non-owner read path exists, so nothing leaks, but
-- nothing is configured either.
ALTER TABLE vehicles
    ADD COLUMN cost_visibility visibility NOT NULL DEFAULT 'private';

COMMENT ON COLUMN vehicles.cost_visibility IS
    'Who may see this car''s costs — service and modification alike. Separate from builds.visibility so the car can be shown without the money.';
COMMENT ON COLUMN modifications.removed_at IS
    'Set on removal rather than deleting the row: a removed modification is still evidence the part fitted.';
