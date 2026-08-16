-- A starter vehicle catalog. Real cars, real years, real bolt patterns.
--
-- This is NOT the fake data the repository rules forbid. That rule is about
-- invented community counts, fabricated testimonials, and screenshots of
-- activity that never happened — the platform launches empty and says so.
-- A catalog of cars that genuinely exist is reference data, and the escape
-- hatch for a car that is missing already exists (`POST /catalog/suggestions`).
--
-- It is also deliberately SMALL. The PRD scopes the real catalog at roughly
-- the top 200 models as a separate content job; this is enough to develop
-- against, and honest about being a fraction.
--
-- What is NOT here, on purpose: engine codes, power and torque figures, and
-- OEM offsets. Those vary by market year and trim, and a plausible wrong
-- number in a fitment table is worse than a blank one — the same rule the
-- parts API enforces on user input applies to us. PCD, centre bore and
-- displacement are widely published and stable, so those are filled in.
--
-- Idempotent: re-running changes nothing. The natural key is the slug scoped
-- to its parent — only `brands.slug` is globally unique, and models,
-- generations, and variants are unique per parent. The prefixes below keep
-- them readable and unambiguous anyway, which is what lets the joins here
-- look a row up by slug alone.

BEGIN;

INSERT INTO brands (id, name, slug) VALUES
    (gen_random_uuid(), 'Toyota',     'toyota'),
    (gen_random_uuid(), 'Honda',      'honda'),
    (gen_random_uuid(), 'Daihatsu',   'daihatsu'),
    (gen_random_uuid(), 'Suzuki',     'suzuki'),
    (gen_random_uuid(), 'Mitsubishi', 'mitsubishi')
ON CONFLICT (slug) DO NOTHING;

INSERT INTO vehicle_models (id, brand_id, name, slug)
SELECT gen_random_uuid(), b.id, m.name, m.slug
FROM (VALUES
    ('toyota',     'Avanza',   'toyota-avanza'),
    ('toyota',     'Yaris',    'toyota-yaris'),
    ('honda',      'Brio',     'honda-brio'),
    ('honda',      'Jazz',     'honda-jazz'),
    ('honda',      'Civic',    'honda-civic'),
    ('daihatsu',   'Xenia',    'daihatsu-xenia'),
    ('suzuki',     'Ertiga',   'suzuki-ertiga'),
    ('mitsubishi', 'Xpander',  'mitsubishi-xpander')
) AS m(brand_slug, name, slug)
JOIN brands b ON b.slug = m.brand_slug
-- Slugs are scoped to their parent, not globally unique — only `brands.slug`
-- is. The prefixes below keep them readable and unambiguous anyway.
ON CONFLICT (brand_id, slug) DO NOTHING;

-- Generation names use what owners and workshops actually say. "Yaris Bakpao"
-- and the Civic chassis codes are the names people search with; a catalog
-- that only carries the manufacturer's marketing names cannot be found by
-- the people it is for.
INSERT INTO vehicle_generations (id, model_id, name, code, slug, year_start, year_end)
SELECT gen_random_uuid(), vm.id, g.name, g.code, g.slug, g.year_start, g.year_end
FROM (VALUES
    ('toyota-avanza',    'Avanza Gen 1',          NULL,   'toyota-avanza-1',      2003, 2011),
    ('toyota-avanza',    'Avanza Gen 2',          NULL,   'toyota-avanza-2',      2011, 2021),
    ('toyota-avanza',    'Avanza Gen 3',          NULL,   'toyota-avanza-3',      2021, NULL),
    ('toyota-yaris',     'Yaris Bakpao',          'XP90', 'toyota-yaris-bakpao',  2006, 2013),
    ('toyota-yaris',     'Yaris XP150',           'XP150','toyota-yaris-xp150',   2013, 2022),
    ('honda-brio',       'Brio Satya',            NULL,   'honda-brio-satya',     2012, 2018),
    ('honda-brio',       'Brio RS',               NULL,   'honda-brio-rs',        2018, NULL),
    ('honda-jazz',       'Jazz GE8',              'GE8',  'honda-jazz-ge8',       2008, 2014),
    ('honda-jazz',       'Jazz GK5',              'GK5',  'honda-jazz-gk5',       2014, 2021),
    ('honda-civic',      'Civic FD',              'FD',   'honda-civic-fd',       2006, 2012),
    ('honda-civic',      'Civic FB',              'FB',   'honda-civic-fb',       2012, 2016),
    ('honda-civic',      'Civic FC',              'FC',   'honda-civic-fc',       2016, 2021),
    ('daihatsu-xenia',   'Xenia Gen 1',           NULL,   'daihatsu-xenia-1',     2003, 2011),
    ('daihatsu-xenia',   'Xenia Gen 2',           NULL,   'daihatsu-xenia-2',     2011, 2021),
    ('suzuki-ertiga',    'Ertiga Gen 1',          NULL,   'suzuki-ertiga-1',      2012, 2018),
    ('suzuki-ertiga',    'Ertiga Gen 2',          NULL,   'suzuki-ertiga-2',      2018, NULL),
    ('mitsubishi-xpander','Xpander',              NULL,   'mitsubishi-xpander-1', 2017, NULL)
) AS g(model_slug, name, code, slug, year_start, year_end)
JOIN vehicle_models vm ON vm.slug = g.model_slug
ON CONFLICT (model_id, slug) DO NOTHING;

-- PCD and centre bore only. Both are published, stable per platform, and are
-- exactly the numbers the fitment engine needs. Everything else a variant
-- could carry is left NULL rather than guessed — `missing_specs` exists to
-- report a gap honestly, and a catalog that lies is worse than one that is
-- visibly incomplete.
INSERT INTO vehicle_variants
    (id, generation_id, name, slug, displacement_cc, transmission, drivetrain, fuel,
     pcd_bolt_count, pcd_diameter_mm, center_bore_mm)
SELECT gen_random_uuid(), vg.id, v.name, v.slug, v.cc,
       v.transmission::transmission_kind, v.drivetrain::drivetrain_kind, v.fuel::fuel_kind,
       v.bolts, v.pcd, v.bore
FROM (VALUES
    ('toyota-avanza-2',     '1.3 G M/T',  'toyota-avanza-2-13g-mt',  1329, 'manual',    'rwd', 'petrol', 4, 100.0, 54.1),
    ('toyota-avanza-2',     '1.5 Veloz',  'toyota-avanza-2-15-veloz',1496, 'automatic', 'rwd', 'petrol', 4, 100.0, 54.1),
    ('toyota-avanza-3',     '1.5 Veloz',  'toyota-avanza-3-15-veloz',1496, 'cvt',       'fwd', 'petrol', 4, 100.0, 54.1),
    ('toyota-yaris-bakpao', '1.5 E',      'toyota-yaris-bakpao-15e', 1497, 'automatic', 'fwd', 'petrol', 4, 100.0, 54.1),
    ('toyota-yaris-xp150',  '1.5 TRD',    'toyota-yaris-xp150-15trd',1496, 'cvt',       'fwd', 'petrol', 4, 100.0, 54.1),
    ('honda-brio-satya',    '1.2 E',      'honda-brio-satya-12e',    1198, 'manual',    'fwd', 'petrol', 4, 100.0, 56.1),
    ('honda-brio-rs',       '1.2 RS CVT', 'honda-brio-rs-12-cvt',    1199, 'cvt',       'fwd', 'petrol', 4, 100.0, 56.1),
    ('honda-jazz-ge8',      '1.5 RS',     'honda-jazz-ge8-15rs',     1497, 'manual',    'fwd', 'petrol', 4, 100.0, 56.1),
    ('honda-jazz-gk5',      '1.5 RS',     'honda-jazz-gk5-15rs',     1497, 'cvt',       'fwd', 'petrol', 4, 100.0, 56.1),
    ('honda-civic-fd',      '1.8 FD1',    'honda-civic-fd-18',       1799, 'automatic', 'fwd', 'petrol', 5, 114.3, 64.1),
    ('honda-civic-fb',      '1.8 CVT',    'honda-civic-fb-18-cvt',   1798, 'cvt',       'fwd', 'petrol', 5, 114.3, 64.1),
    ('honda-civic-fc',      '1.5 Turbo',  'honda-civic-fc-15-turbo', 1498, 'cvt',       'fwd', 'petrol', 5, 114.3, 64.1),
    ('daihatsu-xenia-2',    '1.3 R M/T',  'daihatsu-xenia-2-13r-mt', 1329, 'manual',    'rwd', 'petrol', 4, 100.0, 54.1),
    ('suzuki-ertiga-2',     '1.5 GX',     'suzuki-ertiga-2-15gx',    1462, 'automatic', 'fwd', 'petrol', 4, 100.0, 54.1),
    ('mitsubishi-xpander-1','1.5 Ultimate','mitsubishi-xpander-15u', 1499, 'cvt',       'fwd', 'petrol', 4, 100.0, 67.1)
) AS v(generation_slug, name, slug, cc, transmission, drivetrain, fuel, bolts, pcd, bore)
JOIN vehicle_generations vg ON vg.slug = v.generation_slug
ON CONFLICT (generation_id, slug) DO NOTHING;

COMMIT;

\echo ''
\echo 'Catalog seeded. A starter set, not the real catalog:'
SELECT
    (SELECT count(*) FROM brands)              AS brands,
    (SELECT count(*) FROM vehicle_models)      AS models,
    (SELECT count(*) FROM vehicle_generations) AS generations,
    (SELECT count(*) FROM vehicle_variants)    AS variants;
