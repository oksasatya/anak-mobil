-- Reverse order: children before parents, types after the columns using them.
DROP TABLE IF EXISTS vehicle_variants;
DROP TABLE IF EXISTS vehicle_generations;
DROP TABLE IF EXISTS vehicle_models;
DROP TABLE IF EXISTS brands;

DROP TYPE IF EXISTS fuel_kind;
DROP TYPE IF EXISTS drivetrain_kind;
DROP TYPE IF EXISTS transmission_kind;
