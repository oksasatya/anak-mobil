//! AnakMobil domain — entities, value objects, errors, and pure policy.
//!
//! # The one rule
//!
//! Nothing in this crate knows that HTTP, SQL, Redis, or object storage
//! exist. That is enforced by `Cargo.toml`, not by discipline: the
//! frameworks are simply absent from the dependency list, so importing
//! one does not compile.
//!
//! # What lives here, and what does not
//!
//! Here: entities, value objects, domain errors, and **policy** — pure
//! functions that take data and return a decision, with no async and no
//! I/O. `derive_reminders(vehicle, history, today) -> Vec<Reminder>` is
//! the shape.
//!
//! Not here: use cases. A use case needs a repository, repositories live
//! in the runtime crate, and `domain -> runtime -> domain` is a cycle
//! Cargo rejects. Orchestration lives in `runtime/src/usecase/`, owns the
//! transaction, and calls into the policy functions here.
//!
//! # Module boundaries
//!
//! Cargo enforces the framework boundary but not the boundary *between*
//! these modules — `ai` could reach into `garage` today. Keep each
//! module's surface small and mark internals `pub(in crate::<module>)`
//! so the seam stays visible if a module ever needs to be extracted.

pub mod ai;
pub mod build;
pub mod garage;
pub mod identity;
pub mod knowledge;
pub mod waitlist;
