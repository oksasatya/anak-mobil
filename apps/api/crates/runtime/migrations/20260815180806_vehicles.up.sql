-- A person's cars.
--
-- Split across two tables, and the split is the point.
--
-- `vehicles` holds what a screen shows. `vehicle_private` holds the number
-- plate, the VIN, and what the car cost. The repository rule is that those
-- three never leave the server for anyone who should not see them — including
-- an admin — and the cheapest way to guarantee that is for them not to be in
-- the row a query returns by default.
--
-- Keeping them in one table and filtering in the SELECT works exactly until
-- the first query that forgets, and a leaked plate cannot be recalled. Here a
-- `SELECT * FROM vehicles` cannot leak a plate, because there is no plate in
-- `vehicles`. The join is one extra line, on the owner's own screen only.

CREATE TABLE vehicles (
    id          UUID PRIMARY KEY,
    owner_id    UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,

    -- Nullable on purpose. A person whose model is not in the catalog must
    -- still be able to add their car — otherwise the "suggest a model" flow
    -- has nothing to attach to, and the catalog can only ever describe cars
    -- somebody already managed to enter.
    variant_id  UUID        REFERENCES vehicle_variants (id) ON DELETE SET NULL,

    -- What the owner calls it. Falls back to the catalog name when absent.
    nickname    TEXT,
    -- Free text until a variant is matched: "Toyota Avanza 2019" typed by
    -- hand is better than refusing to store the car at all.
    described_as TEXT,

    year        SMALLINT,
    colour      TEXT,
    mileage_km  INTEGER,

    -- Garage order, owner-defined. A small integer rewritten on reorder
    -- rather than a fractional index: a person has a handful of cars, and
    -- rewriting five rows is cheaper to understand than a rebalancing scheme
    -- nobody will read again.
    position    INTEGER     NOT NULL DEFAULT 0,

    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT vehicles_year_plausible
        CHECK (year IS NULL OR year BETWEEN 1900 AND 2100),
    CONSTRAINT vehicles_mileage_not_negative
        CHECK (mileage_km IS NULL OR mileage_km >= 0),
    -- A car with neither a catalog match nor a description is a row nobody
    -- can identify, including its owner.
    CONSTRAINT vehicles_identifiable
        CHECK (variant_id IS NOT NULL OR described_as IS NOT NULL)
);

-- Every list is "my cars, in my order", so the index carries the sort.
CREATE INDEX vehicles_owner_position_idx ON vehicles (owner_id, position, id);

-- "Which cars in the community are this variant" — the query behind every
-- community count and every piece of fitment evidence.
CREATE INDEX vehicles_variant_idx ON vehicles (variant_id) WHERE variant_id IS NOT NULL;

CREATE TABLE vehicle_private (
    vehicle_id     UUID PRIMARY KEY REFERENCES vehicles (id) ON DELETE CASCADE,

    plate          TEXT,
    vin            TEXT,
    purchase_price NUMERIC(14, 2),
    purchase_date  DATE,

    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT vehicle_private_price_not_negative
        CHECK (purchase_price IS NULL OR purchase_price >= 0)
);

COMMENT ON TABLE vehicle_private IS
    'Plate, VIN, and purchase price. Never returned to anyone but the owner, including admins. Separate from vehicles so a SELECT * cannot leak it.';

CREATE TRIGGER vehicles_set_updated_at
    BEFORE UPDATE ON vehicles
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER vehicle_private_set_updated_at
    BEFORE UPDATE ON vehicle_private
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
