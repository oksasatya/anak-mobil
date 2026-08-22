//! Application services — the layer that orchestrates.
//!
//! A use case owns the transaction boundary. The shape is always the
//! same: open a transaction, authorise, read, call a **pure policy
//! function** from the domain crate to decide, write the result, commit.
//!
//! ```text
//! pub async fn record_service(pool: &PgPool, user: UserId, input: Input)
//!     -> Result<ServiceRecordId, AppError>
//! {
//!     let mut tx = pool.begin().await?;
//!     let vehicle = GarageRepo::find_owned(&mut tx, input.vehicle_id, user).await?
//!         .ok_or(AppError::NotFound)?;
//!     let id = GarageRepo::insert_service(&mut tx, &record).await?;
//!     let history = GarageRepo::history_for_update(&mut tx, vehicle.id).await?;
//!     let reminders = garage::derive_reminders(&vehicle, &history, today());
//!     GarageRepo::replace_reminders(&mut tx, vehicle.id, &reminders).await?;
//!     tx.commit().await?;
//!     Ok(id)
//! }
//! ```
//!
//! # Why this layer exists at all
//!
//! Without it, the code above ends up inside an HTTP handler. It is named
//! "handler" but it is an application service, and once that happens it
//! is hard to find, hard to test, and impossible to reuse from the worker
//! role or a CLI command.
//!
//! # Why it is not in the domain crate
//!
//! It needs a repository, repositories live in `adapter`, and
//! `domain -> runtime -> domain` is a cycle Cargo rejects. Only the pure
//! policy functions live in the domain crate.

// Populated per bounded context as the API stories land — AM-360 onward.

pub mod auth;
pub mod builds;
pub mod catalog;
pub mod garage;
pub mod jobs;
pub mod part_merge;
pub mod parts;
pub mod profile;
pub mod roles;
pub mod service_history;
pub mod service_summary;
