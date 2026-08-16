//! A car's service history.

use sqlx::PgPool;
use uuid::Uuid;

use crate::adapter::postgres::service_repo::{self, ServiceInput, ServiceRecord};
use crate::shared::pagination::{DateCursor, Page};

/// Why a history operation did not succeed.
#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    /// The record or the car does not exist, or belongs to somebody else.
    ///
    /// One variant for all three. Distinguishing them would let a caller
    /// discover which ids are real.
    #[error("no such service record")]
    NotFound,
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
}

/// One page of a car's history, newest first.
///
/// # Errors
///
/// [`HistoryError::Database`] when the query fails.
pub async fn page(
    pool: &PgPool,
    owner_id: Uuid,
    vehicle_id: Uuid,
    after: Option<&DateCursor>,
    limit: u16,
) -> Result<Page<ServiceRecord>, HistoryError> {
    let mut conn = pool.acquire().await?;
    let mut rows = service_repo::page(&mut conn, owner_id, vehicle_id, after, limit).await?;

    // The repository asked for one row more than the page. Its presence is
    // how we know another page exists; it is dropped rather than returned,
    // which is also why a full final page still ends the loop correctly —
    // a client that stopped on a short page would ask one time too few.
    let has_more = rows.len() > usize::from(limit);
    rows.truncate(usize::from(limit));

    let next_cursor = has_more.then(|| rows.last()).flatten().map(|last| {
        DateCursor {
            date: last.service_date,
            id: last.id,
        }
        .encode()
    });

    Ok(Page {
        items: rows,
        next_cursor,
    })
}

/// One record.
///
/// # Errors
///
/// [`HistoryError::NotFound`] when it is not this person's.
pub async fn detail(
    pool: &PgPool,
    owner_id: Uuid,
    id: Uuid,
) -> Result<ServiceRecord, HistoryError> {
    let mut conn = pool.acquire().await?;
    service_repo::find_owned(&mut conn, owner_id, id)
        .await?
        .ok_or(HistoryError::NotFound)
}

/// Record a service.
///
/// # Errors
///
/// [`HistoryError::NotFound`] when the car is not this person's.
pub async fn record(
    pool: &PgPool,
    owner_id: Uuid,
    vehicle_id: Uuid,
    input: &ServiceInput,
) -> Result<Uuid, HistoryError> {
    let id = Uuid::now_v7();
    let mut conn = pool.acquire().await?;

    if service_repo::insert(&mut conn, id, owner_id, vehicle_id, input).await? {
        Ok(id)
    } else {
        Err(HistoryError::NotFound)
    }
}

/// Replace a record.
///
/// # Errors
///
/// [`HistoryError::NotFound`] when it is not this person's.
pub async fn amend(
    pool: &PgPool,
    owner_id: Uuid,
    id: Uuid,
    input: &ServiceInput,
) -> Result<(), HistoryError> {
    let mut conn = pool.acquire().await?;
    if service_repo::update(&mut conn, owner_id, id, input).await? {
        Ok(())
    } else {
        Err(HistoryError::NotFound)
    }
}

/// Remove a record.
///
/// # Errors
///
/// [`HistoryError::NotFound`] when it is not this person's.
pub async fn remove(pool: &PgPool, owner_id: Uuid, id: Uuid) -> Result<(), HistoryError> {
    let mut conn = pool.acquire().await?;
    if service_repo::delete(&mut conn, owner_id, id).await? {
        Ok(())
    } else {
        Err(HistoryError::NotFound)
    }
}
