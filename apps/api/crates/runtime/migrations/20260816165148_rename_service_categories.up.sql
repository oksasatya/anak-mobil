-- One taxonomy language.
--
-- `service_category` shipped with Indonesian values while PRD §10 writes the
-- modification categories in English. Following each source literally would
-- put `oli_mesin` and `wheels` side by side in one API.
--
-- This is a public contract change, taken now because there are no users. The
-- same change after launch costs a versioned API.
--
-- `RENAME VALUE` renames the label in the catalog and rewrites no table data.
-- Verified on Postgres 17.11: it runs inside a transaction and existing rows
-- read back with the new label immediately.
--
-- Eight statements, not nine: `tune_up` is already English, and renaming a
-- value to itself fails with "enum label already exists".

ALTER TYPE service_category RENAME VALUE 'oli_mesin'     TO 'engine_oil';
ALTER TYPE service_category RENAME VALUE 'oli_transmisi' TO 'transmission_oil';
ALTER TYPE service_category RENAME VALUE 'rem'           TO 'brakes';
ALTER TYPE service_category RENAME VALUE 'kaki_kaki'     TO 'suspension';
ALTER TYPE service_category RENAME VALUE 'ac'            TO 'air_conditioning';
ALTER TYPE service_category RENAME VALUE 'kelistrikan'   TO 'electrical';
ALTER TYPE service_category RENAME VALUE 'body'          TO 'bodywork';
ALTER TYPE service_category RENAME VALUE 'lainnya'       TO 'other';
