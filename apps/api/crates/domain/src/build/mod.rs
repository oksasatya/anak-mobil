//! Builds, parts, and structured fitment specs.
//!
//! Entities: `Build`, `Modification`, `Part`. Value objects: `PartId`,
//! `Pcd`, `Offset`, `CenterBore`, `TyreSize`.
//!
//! Policy candidates: fitment comparison, build-completeness scoring.
//!
//! # Why this is its own module and not part of `garage`
//!
//! This is the source of the fitment data, which is the most valuable
//! thing the product knows. Its lifecycle differs from service history:
//! builds are edited and shared, service records are appended and mostly
//! private.
//!
//! # Numbers, not strings
//!
//! PCD, offset, and center bore are stored numerically. A fitment engine
//! that has to parse `"5x114.3"` out of a string at comparison time is a
//! fitment engine that will be wrong.
