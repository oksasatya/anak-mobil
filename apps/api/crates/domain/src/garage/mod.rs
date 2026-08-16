//! Vehicles, the vehicle catalog, and service history.
//!
//! Entities: `Vehicle`, `ServiceRecord`. Value objects: `VehicleId`,
//! `Plate`, `Vin`, `Odometer`, `Money`.
//!
//! Policy lives in [`policy`]: `derive_reminders` turns the latest
//! service per category into what the car is due for. Cost rollups are
//! deliberately not here — summing money is what a database does, and
//! doing it in SQL avoids loading years of history into memory to add up.
//!
//! # Privacy
//!
//! `Plate`, `Vin`, and purchase price are private by default and are
//! filtered server-side at every read — including from admins. The
//! filtering itself is a runtime concern, but the types here should make
//! it hard to hand those values to a serialiser by accident.

pub mod policy;
