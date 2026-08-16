-- Back to the shipped Indonesian labels.
--
-- `ADD VALUE` is the statement with historical transaction restrictions.
-- `RENAME VALUE` is not, and this file only renames.

ALTER TYPE service_category RENAME VALUE 'engine_oil'       TO 'oli_mesin';
ALTER TYPE service_category RENAME VALUE 'transmission_oil' TO 'oli_transmisi';
ALTER TYPE service_category RENAME VALUE 'brakes'           TO 'rem';
ALTER TYPE service_category RENAME VALUE 'suspension'       TO 'kaki_kaki';
ALTER TYPE service_category RENAME VALUE 'air_conditioning' TO 'ac';
ALTER TYPE service_category RENAME VALUE 'electrical'       TO 'kelistrikan';
ALTER TYPE service_category RENAME VALUE 'bodywork'         TO 'body';
ALTER TYPE service_category RENAME VALUE 'other'            TO 'lainnya';
