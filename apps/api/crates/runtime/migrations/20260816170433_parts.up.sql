-- Parts, and the numbers that make fitment answerable.
--
-- A row here is one exact configuration, not a product name. `Enkei RPF1
-- 18x8.5 ET40` and `Enkei RPF1 18x9.5 ET45` share a brand and a product name
-- and are different wheels. Identifying a part by its name lets a curator
-- collapse them, and every modification that genuinely used the first is then
-- read as having used the second — a confident wrong number, which the
-- platform's own rules call worse than no answer at all.

-- PRD §10. A closed set, so a native enum: ALTER TYPE … ADD VALUE does not
-- break a running older version, and free text would give twelve spellings of
-- "velg". `brakes` (not `brake`) to match the taxonomy service_category was
-- just renamed onto.
CREATE TYPE modification_category AS ENUM (
    'wheels',
    'tyres',
    'suspension',
    'brakes',
    'engine',
    'intake',
    'exhaust',
    'ecu',
    'transmission',
    'exterior',
    'interior',
    'lighting',
    'audio',
    'electronics',
    'other'
);

-- No `rejected`, and no `duplicate`. A rejected part would orphan every
-- modification already using it, and AC3's whole point is that the part is
-- usable the moment it is typed. A curator approves it, or merges it into the
-- part it duplicates — the same rule the vehicle catalog already follows.
CREATE TYPE part_status AS ENUM ('pending', 'approved');

CREATE TYPE suspension_type AS ENUM ('coilover', 'lowering_spring', 'air');

CREATE TABLE parts (
    id             UUID PRIMARY KEY,

    category       modification_category NOT NULL,
    brand          TEXT NOT NULL,
    product_name   TEXT NOT NULL,

    status         part_status NOT NULL DEFAULT 'pending',

    -- A one-hop cache, not a pointer chain. `part_merges` is the truth; this
    -- column is what keeps a read a single join instead of a recursive CTE.
    -- Merge and unmerge recompute it for the affected component and nothing
    -- else. NULL means this part is its own canonical form.
    canonical_part_id UUID REFERENCES parts (id) ON DELETE SET NULL,

    -- ON DELETE SET NULL, not CASCADE — the same reasoning as
    -- catalog_suggestions. What was suggested is catalog work, not personal
    -- data, and losing it because somebody closed their account would throw
    -- away a part other people's builds are pointing at.
    suggested_by   UUID REFERENCES users (id) ON DELETE SET NULL,

    -- Wheels.
    wheel_diameter_in    NUMERIC(3, 1),
    wheel_width_in       NUMERIC(3, 1),
    offset_et_mm         SMALLINT,
    pcd_bolt_count       SMALLINT,
    pcd_diameter_mm      NUMERIC(5, 1),
    center_bore_mm       NUMERIC(5, 1),

    -- Tyres.
    tyre_width_mm        SMALLINT,
    tyre_aspect_ratio    SMALLINT,
    tyre_rim_diameter_in NUMERIC(3, 1),

    -- Suspension.
    suspension_kind      suspension_type,
    spring_rate_kgmm     NUMERIC(4, 1),

    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT parts_brand_present        CHECK (length(btrim(brand)) > 0),
    CONSTRAINT parts_product_name_present CHECK (length(btrim(product_name)) > 0),

    -- A part cannot be its own canonical form by pointing at itself; NULL
    -- means that. A self-reference would make the one-hop resolution loop.
    CONSTRAINT parts_canonical_not_self CHECK (canonical_part_id IS DISTINCT FROM id),

    -- A type alone is not validity. `pcd_bolt_count = 0`, `offset_et_mm =
    -- -999`, and an aspect ratio of 900 are all perfectly typed and all
    -- impossible, and a completeness check that only looks for NULL calls them
    -- complete. Each bound below admits the real market and nothing else.

    -- 10" is a kei-car and trailer wheel; 30" is the ceiling on a lifted truck
    -- or a VIP-style build. Below 10 or above 30 is a typo.
    CONSTRAINT parts_wheel_diameter_range
        CHECK (wheel_diameter_in IS NULL OR wheel_diameter_in BETWEEN 10 AND 30),
    -- 4J is a narrow old-kei rim; 20J covers the widest wide-body rear.
    CONSTRAINT parts_wheel_width_range
        CHECK (wheel_width_in IS NULL OR wheel_width_in BETWEEN 4 AND 20),
    -- Signed, because a deep-dish wheel has a negative ET and an unsigned
    -- column would reject exactly the wheels people fit. -100 covers the
    -- deepest dish sold; +100 covers a van or light truck.
    CONSTRAINT parts_offset_range
        CHECK (offset_et_mm IS NULL OR offset_et_mm BETWEEN -100 AND 100),
    -- 3 bolts on a kei car, 8 on a heavy pickup. Zero is the value a
    -- NULL-only completeness check would happily call complete.
    CONSTRAINT parts_pcd_bolt_count_range
        CHECK (pcd_bolt_count IS NULL OR pcd_bolt_count BETWEEN 3 AND 8),
    -- Real PCDs run 98 (Fiat) through 139.7 (Hilux) to 170 (heavy 4x4).
    -- 80–250 admits all of them with headroom and rejects a decimal-point slip.
    CONSTRAINT parts_pcd_diameter_range
        CHECK (pcd_diameter_mm IS NULL OR pcd_diameter_mm BETWEEN 80 AND 250),
    -- Bores run 54.1 (Toyota) through 67.1 (Mitsubishi) to about 110 on vans.
    -- 40–150 is the envelope; outside it is a mistyped hub or a wrong unit.
    CONSTRAINT parts_center_bore_range
        CHECK (center_bore_mm IS NULL OR center_bore_mm BETWEEN 40 AND 150),
    -- Metric tyre widths sold in Indonesia run 125 to 355. 105–405 leaves room
    -- for imports without admitting a width typed in inches.
    CONSTRAINT parts_tyre_width_range
        CHECK (tyre_width_mm IS NULL OR tyre_width_mm BETWEEN 105 AND 405),
    -- 25-series on a stretched setup, 85 on an off-road tyre. A value of 900
    -- is the classic "typed the whole size into one field".
    CONSTRAINT parts_tyre_aspect_ratio_range
        CHECK (tyre_aspect_ratio IS NULL OR tyre_aspect_ratio BETWEEN 20 AND 95),
    -- Matches the wheel range. NUMERIC(3,1) rather than an integer because
    -- 16.5" rims exist on light commercials.
    CONSTRAINT parts_tyre_rim_diameter_range
        CHECK (tyre_rim_diameter_in IS NULL OR tyre_rim_diameter_in BETWEEN 10 AND 30),
    -- Street coilovers sit at 4–8 kg/mm; a time-attack car runs 20–30.
    -- 1–40 covers both ends without admitting a rate typed in N/mm.
    CONSTRAINT parts_spring_rate_range
        CHECK (spring_rate_kgmm IS NULL OR spring_rate_kgmm BETWEEN 1 AND 40),

    -- Category + brand + product + the typed specs together identify a part.
    -- NULLS NOT DISTINCT (Postgres 15+) is what makes that work across
    -- nullable spec columns: without it, two spec-less rows for the same
    -- product would both be "distinct" and the queue would fill with copies.
    --
    -- Writers use ON CONFLICT DO NOTHING and re-select, so this constraint
    -- never surfaces to a caller as an error — it makes typing the same part
    -- twice idempotent instead.
    --
    -- Case and spacing are NOT normalised here. Text is trimmed at the HTTP
    -- boundary; "ENKEI" and "Enkei" produce two rows, and merge is what
    -- resolves them. Folding case in the index would make the constraint
    -- disagree with what a curator sees on screen.
    CONSTRAINT parts_identity UNIQUE NULLS NOT DISTINCT (
        category, brand, product_name,
        wheel_diameter_in, wheel_width_in, offset_et_mm,
        pcd_bolt_count, pcd_diameter_mm, center_bore_mm,
        tyre_width_mm, tyre_aspect_ratio, tyre_rim_diameter_in,
        suspension_kind, spring_rate_kgmm
    )
);

-- Every foreign key gets an index; PostgreSQL indexes primary keys and unique
-- constraints and never indexes a foreign key.
CREATE INDEX parts_canonical_idx ON parts (canonical_part_id)
    WHERE canonical_part_id IS NOT NULL;
CREATE INDEX parts_suggested_by_idx ON parts (suggested_by)
    WHERE suggested_by IS NOT NULL;

-- The curation queue: oldest pending first, the same shape as
-- catalog_suggestions_queue_idx.
CREATE INDEX parts_queue_idx ON parts (status, created_at)
    WHERE status = 'pending';

-- Search filters by category first.
CREATE INDEX parts_category_brand_idx ON parts (category, brand);

CREATE TRIGGER parts_set_updated_at
    BEFORE UPDATE ON parts
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

COMMENT ON COLUMN parts.canonical_part_id IS
    'One-hop cache of the merge log. part_merges is the truth; this is recomputed by merge and unmerge for the affected component only.';
COMMENT ON CONSTRAINT parts_identity ON parts IS
    'A part is one exact configuration, not a product name. Two wheels sharing brand and product but differing in width are different parts.';
