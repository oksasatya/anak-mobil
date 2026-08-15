//! Vehicles, the vehicle catalog, and service history.
//!
//! Entities: `Vehicle`, `ServiceRecord`. Value objects: `VehicleId`,
//! `Plate`, `Vin`, `Odometer`, `Money`.
//!
//! Policy candidates: `derive_reminders(vehicle, history, today)`, cost
//! rollups, and odometer-plausibility checks.
//!
//! # Privacy
//!
//! `Plate`, `Vin`, and purchase price are private by default and are
//! filtered server-side at every read — including from admins. The
//! filtering itself is a runtime concern, but the types here should make
//! it hard to hand those values to a serialiser by accident.
