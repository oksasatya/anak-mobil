//! A car's build.
//!
//! One build per car, so it is addressed through the vehicle for writes and by
//! its own id for reads.

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::adapter::postgres::build_repo::{
    self, Build, BuildInput, Modification, ModificationInput,
};
use crate::adapter::postgres::part_repo::{self, PartInput};
use crate::usecase::parts;

/// Why a build operation did not succeed.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// The build, the car, or the modification does not exist, or belongs to
    /// somebody else. One variant for all of them: answering "that is not
    /// yours" differently from "that does not exist" tells a caller which ids
    /// are real.
    #[error("no such build")]
    NotFound,
    /// Today's allowance for new parts is spent.
    ///
    /// The same ceiling `POST /parts` enforces, on the other entrance to the
    /// same queue.
    #[error("today's parts allowance is spent")]
    TooManyParts,
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
}

/// Create or replace this car's build.
///
/// This is the explicit `PUT` — it means what it says, and the caller's
/// `notes` and `visibility` replace whatever was there. Anything that only
/// needs a build to *exist*, without asserting an opinion about its content,
/// goes through [`add_modification`] instead, which calls
/// [`build_repo::ensure_build`] rather than this.
///
/// # Errors
///
/// [`BuildError::NotFound`] when the car is not this person's.
pub async fn save(
    pool: &PgPool,
    owner_id: Uuid,
    vehicle_id: Uuid,
    input: &BuildInput,
) -> Result<Uuid, BuildError> {
    let mut conn = pool.acquire().await?;
    build_repo::upsert_build(&mut conn, Uuid::now_v7(), owner_id, vehicle_id, input)
        .await?
        .ok_or(BuildError::NotFound)
}

/// This person's build for this car, with its modifications.
///
/// Two queries, not one per modification.
///
/// # Errors
///
/// [`BuildError::NotFound`] when the car has no build, or is not this
/// person's.
pub async fn for_vehicle(
    pool: &PgPool,
    owner_id: Uuid,
    vehicle_id: Uuid,
) -> Result<(Build, Vec<Modification>), BuildError> {
    let mut conn = pool.acquire().await?;
    let build = build_repo::find_build_for_vehicle(&mut conn, owner_id, vehicle_id)
        .await?
        .ok_or(BuildError::NotFound)?;
    let modifications = build_repo::modifications_for(&mut conn, &[build.id]).await?;

    Ok((build, modifications))
}

/// What part a modification is about: one already in the catalog, or one the
/// person is typing for the first time.
#[derive(Debug, Clone)]
pub enum PartChoice {
    Existing(Uuid),
    New(Box<PartInput>),
}

/// Resolve a part choice to a concrete id, inside the caller's transaction.
///
/// The one piece `add_modification` and `amend_modification` share: neither
/// cares whether the part already existed, only that it now has an id.
async fn resolve_part(
    tx: &mut PgConnection,
    owner_id: Uuid,
    part: PartChoice,
) -> Result<Uuid, BuildError> {
    Ok(match part {
        PartChoice::Existing(id) => id,
        PartChoice::New(spec) => {
            // The allowance is checked HERE and not only in `usecase::parts`,
            // because the curation queue has two entrances and this is the
            // other one. `POST /parts` stops at twenty a day; typing a part
            // inline while recording a modification reaches the same insert,
            // and did so with no ceiling at all — a review found it. The queue
            // is read by a person, and flooding it is a denial of service
            // against curation rather than against the server.
            //
            // Calling `usecase::parts::suggest` instead would be the obvious
            // reuse and is wrong: it takes a pool and opens its own
            // connection, which would put the part insert outside the
            // transaction this modification depends on.
            if parts::allowance_spent(tx, owner_id).await? {
                return Err(BuildError::TooManyParts);
            }
            part_repo::find_or_create_pending(tx, Uuid::now_v7(), owner_id, &spec).await?
        }
    })
}

/// Add a modification to this car's build.
///
/// AC3, completed: when the part is new, the `parts` row and the
/// `modifications` row are written in **one transaction**. A rolled-back
/// modification must not leave an orphaned pending part sitting in the
/// curation queue for a build that does not exist.
///
/// The build is created on demand — a person adding their first modification
/// has not thought about "creating a build", and making them do it first
/// would be a step that exists only because the schema has two tables. This
/// calls [`build_repo::ensure_build`], never [`build_repo::upsert_build`]:
/// `upsert_build`'s conflict clause writes the caller's `notes` and
/// `visibility` on every call, which is correct for the explicit `PUT` in
/// [`save`] and would silently wipe the owner's notes and reset a public
/// build back to private every time they added a part.
///
/// # Errors
///
/// [`BuildError::NotFound`] when the car is not this person's.
pub async fn add_modification(
    pool: &PgPool,
    owner_id: Uuid,
    vehicle_id: Uuid,
    part: PartChoice,
    input: &ModificationInput,
) -> Result<Uuid, BuildError> {
    let mut tx = pool.begin().await?;

    let build_id = build_repo::ensure_build(&mut tx, Uuid::now_v7(), owner_id, vehicle_id)
        .await?
        .ok_or(BuildError::NotFound)?;

    let part_id = resolve_part(&mut tx, owner_id, part).await?;

    let id = Uuid::now_v7();
    let input = ModificationInput {
        part_id,
        ..input.clone()
    };
    if !build_repo::insert_modification(&mut tx, id, owner_id, build_id, &input).await? {
        return Err(BuildError::NotFound);
    }

    tx.commit().await?;
    Ok(id)
}

/// Replace a modification, including its part.
///
/// `ModificationRequest` on the HTTP side accepts either `part_id` or an
/// inline `part` on the amend too, so this resolves a part choice the same
/// way [`add_modification`] does — the difference is there is no build to
/// create on demand, because a modification cannot exist without one.
///
/// # Errors
///
/// [`BuildError::NotFound`] when it is not this person's.
pub async fn amend_modification(
    pool: &PgPool,
    owner_id: Uuid,
    id: Uuid,
    part: PartChoice,
    input: &ModificationInput,
) -> Result<(), BuildError> {
    let mut tx = pool.begin().await?;

    let part_id = resolve_part(&mut tx, owner_id, part).await?;
    let input = ModificationInput {
        part_id,
        ..input.clone()
    };

    if build_repo::update_modification(&mut tx, owner_id, id, &input).await? {
        tx.commit().await?;
        Ok(())
    } else {
        Err(BuildError::NotFound)
    }
}

/// Mark a part as taken off.
///
/// The row stays. A removed modification is still evidence the part fitted
/// the car, which is what `ever_installed_build_count` counts in the next
/// slice. `removed_at` is set server-side by `mark_modification_removed`
/// (`now()`, in SQL) — there is no path from a request body to this column.
///
/// # Errors
///
/// [`BuildError::NotFound`] when it is not this person's.
pub async fn remove_modification(
    pool: &PgPool,
    owner_id: Uuid,
    id: Uuid,
) -> Result<(), BuildError> {
    let mut conn = pool.acquire().await?;
    if build_repo::mark_modification_removed(&mut conn, owner_id, id).await? {
        Ok(())
    } else {
        Err(BuildError::NotFound)
    }
}
