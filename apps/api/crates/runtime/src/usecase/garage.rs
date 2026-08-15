//! A person's garage.
//!
//! Every function takes the owner. There is no path that reaches a car
//! without saying whose it is, which is what makes "only your own vehicles"
//! a property of the signatures rather than of the care taken at each call
//! site.

use sqlx::PgPool;
use uuid::Uuid;

use crate::adapter::postgres::vehicle_repo::{self, Vehicle, VehicleInput, VehiclePrivate};

/// Why a garage operation did not succeed.
#[derive(Debug, thiserror::Error)]
pub enum GarageError {
    /// The car does not exist, or belongs to somebody else.
    ///
    /// One variant for both, deliberately. Answering "that is not yours"
    /// differently from "that does not exist" tells a caller which ids are
    /// real, which is a way to enumerate other people's cars.
    #[error("no such vehicle")]
    NotFound,
    #[error("the reorder listed a vehicle that is not yours")]
    ReorderMismatch,
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
}

/// Every car this person owns.
///
/// # Errors
///
/// [`GarageError::Database`] when the query fails.
pub async fn list(pool: &PgPool, owner_id: Uuid) -> Result<Vec<Vehicle>, GarageError> {
    let mut conn = pool.acquire().await?;
    Ok(vehicle_repo::list_owned(&mut conn, owner_id).await?)
}

/// One car, with its private fields.
///
/// The private half is fetched separately and only here — the list never
/// touches it.
///
/// # Errors
///
/// [`GarageError::NotFound`] when the car is not this person's.
pub async fn detail(
    pool: &PgPool,
    owner_id: Uuid,
    id: Uuid,
) -> Result<(Vehicle, Option<VehiclePrivate>), GarageError> {
    let mut conn = pool.acquire().await?;

    let vehicle = vehicle_repo::find_owned(&mut conn, owner_id, id)
        .await?
        .ok_or(GarageError::NotFound)?;
    let private = vehicle_repo::find_private(&mut conn, owner_id, id).await?;

    Ok((vehicle, private))
}

/// Add a car, optionally with its private fields.
///
/// Both writes are one transaction: a car whose plate was recorded but whose
/// row was rolled back would be private data belonging to nothing.
///
/// # Errors
///
/// [`GarageError::Database`] when the write fails.
pub async fn add(
    pool: &PgPool,
    owner_id: Uuid,
    input: &VehicleInput,
    private: Option<&VehiclePrivate>,
) -> Result<Uuid, GarageError> {
    let id = Uuid::now_v7();

    let mut tx = pool.begin().await?;
    vehicle_repo::insert(&mut tx, id, owner_id, input).await?;
    if let Some(private) = private {
        vehicle_repo::upsert_private(&mut tx, owner_id, id, private).await?;
    }
    tx.commit().await?;

    Ok(id)
}

/// Replace a car's public fields.
///
/// # Errors
///
/// [`GarageError::NotFound`] when the car is not this person's.
pub async fn update(
    pool: &PgPool,
    owner_id: Uuid,
    id: Uuid,
    input: &VehicleInput,
) -> Result<(), GarageError> {
    let mut conn = pool.acquire().await?;
    if vehicle_repo::update(&mut conn, owner_id, id, input).await? {
        Ok(())
    } else {
        Err(GarageError::NotFound)
    }
}

/// Set or replace a car's private fields.
///
/// # Errors
///
/// [`GarageError::NotFound`] when the car is not this person's.
pub async fn set_private(
    pool: &PgPool,
    owner_id: Uuid,
    id: Uuid,
    private: &VehiclePrivate,
) -> Result<(), GarageError> {
    let mut conn = pool.acquire().await?;
    if vehicle_repo::upsert_private(&mut conn, owner_id, id, private).await? {
        Ok(())
    } else {
        Err(GarageError::NotFound)
    }
}

/// Remove a car, and with it the plate, the VIN, and the price.
///
/// A hard delete. When somebody asks for their car to be gone, leaving a
/// row with a flag on it means their plate is still stored — which is not
/// what they asked for, and not what a deletion request under the personal
/// data law means either.
///
/// # Errors
///
/// [`GarageError::NotFound`] when the car is not this person's.
pub async fn remove(pool: &PgPool, owner_id: Uuid, id: Uuid) -> Result<(), GarageError> {
    let mut conn = pool.acquire().await?;
    if vehicle_repo::delete(&mut conn, owner_id, id).await? {
        Ok(())
    } else {
        Err(GarageError::NotFound)
    }
}

/// Rearrange the garage.
///
/// The count of rows actually moved is compared against the list that was
/// sent. A mismatch means an id in the request was not this person's, and the
/// whole reorder is rolled back rather than partly applied — a garage in a
/// half-sorted state is harder to explain than a rejected request.
///
/// # Errors
///
/// [`GarageError::ReorderMismatch`] when the list contains an id that is not
/// this person's.
pub async fn reorder(pool: &PgPool, owner_id: Uuid, ordered: &[Uuid]) -> Result<(), GarageError> {
    let mut tx = pool.begin().await?;
    let moved = vehicle_repo::reorder(&mut tx, owner_id, ordered).await?;

    if moved as usize == ordered.len() {
        tx.commit().await?;
        Ok(())
    } else {
        tx.rollback().await?;
        Err(GarageError::ReorderMismatch)
    }
}
