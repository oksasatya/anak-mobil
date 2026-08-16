-- Service history.
--
-- What was done to a car, when, at what mileage, and what it cost. The
-- record people actually keep, and the one a used-car buyer wants to see.

-- The taxonomy from the feature breakdown. A closed set, so a native enum
-- rather than free text: `ALTER TYPE … ADD VALUE` does not break a running
-- older version, and free text would give twelve spellings of "ganti oli".
CREATE TYPE service_category AS ENUM (
    'oli_mesin',
    'oli_transmisi',
    'rem',
    'kaki_kaki',
    'tune_up',
    'ac',
    'kelistrikan',
    'body',
    'lainnya'
);

CREATE TABLE service_records (
    id             UUID PRIMARY KEY,
    vehicle_id     UUID        NOT NULL REFERENCES vehicles (id) ON DELETE CASCADE,

    service_date   DATE        NOT NULL,
    mileage_km     INTEGER,
    category       service_category NOT NULL,

    -- Free text until the parts catalog exists (AM-361) and the garage
    -- directory exists (Phase 2). Waiting for those tables would mean people
    -- cannot record a service for months; text now is lossless and can be
    -- matched to real rows later.
    parts_replaced TEXT[]      NOT NULL DEFAULT '{}',
    garage_name    TEXT,

    -- NUMERIC, never a float. Also the column AM-162 will filter by vehicle
    -- visibility once anything can read another person's history — today
    -- nothing can, because every query is scoped to the owner.
    cost           NUMERIC(14, 2),
    notes          TEXT,

    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT service_records_mileage_not_negative
        CHECK (mileage_km IS NULL OR mileage_km >= 0),
    CONSTRAINT service_records_cost_not_negative
        CHECK (cost IS NULL OR cost >= 0),
    -- A service dated in the future is a typo, and it would sort above every
    -- real record forever.
    CONSTRAINT service_records_not_in_the_future
        CHECK (service_date <= CURRENT_DATE + 1)
);

-- The timeline, and the index the cursor walks.
--
-- `(vehicle_id, service_date DESC, id DESC)` matches the sort exactly, so
-- paging deep into a long history stays an index scan rather than a sort of
-- everything before it.
--
-- `id` is in the key because it breaks ties: several services can share one
-- date, and a cursor over date alone would either repeat them or skip them.
CREATE INDEX service_records_timeline_idx
    ON service_records (vehicle_id, service_date DESC, id DESC);

CREATE TRIGGER service_records_set_updated_at
    BEFORE UPDATE ON service_records
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

COMMENT ON COLUMN service_records.cost IS
    'Filtered by vehicle visibility for non-owners once sharing exists (AM-162). Today every read is owner-scoped.';
