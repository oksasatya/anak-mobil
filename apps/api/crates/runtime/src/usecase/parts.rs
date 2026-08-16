//! Searching the parts catalog, and adding to it.
//!
//! A part is reference data, so none of these take an owner. What they do
//! take is the caller's id, recorded as `suggested_by` — curation provenance,
//! not ownership.

use sqlx::PgPool;
use uuid::Uuid;

use crate::adapter::postgres::part_repo::{self, Part, PartCategory, PartInput};

/// How many parts one person may add in a day.
///
/// The queue is read by a human, so flooding it denies service to curation
/// rather than to the server. The same reasoning and the same number as
/// `catalog::SUGGESTIONS_PER_DAY`; kept separate because the two queues are
/// read by different people and their limits will diverge.
pub(crate) const PARTS_PER_DAY: i64 = 20;

/// Why a parts operation did not succeed.
#[derive(Debug, thiserror::Error)]
pub enum PartError {
    #[error("no such part")]
    NotFound,
    #[error("too many parts added today")]
    TooManyParts,
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
}

/// Parts matching a category and a scrap of text.
///
/// # Errors
///
/// [`PartError::Database`] when the query fails.
pub async fn search(
    pool: &PgPool,
    category: Option<PartCategory>,
    query: Option<&str>,
    limit: u16,
) -> Result<Vec<Part>, PartError> {
    let mut conn = pool.acquire().await?;
    Ok(part_repo::search(&mut conn, category, query, limit).await?)
}

/// One part.
///
/// # Errors
///
/// [`PartError::NotFound`] when no part has this id.
pub async fn detail(pool: &PgPool, id: Uuid) -> Result<Part, PartError> {
    let mut conn = pool.acquire().await?;
    part_repo::find(&mut conn, id)
        .await?
        .ok_or(PartError::NotFound)
}

/// Add a part, or return the id of the identical one somebody already added.
///
/// AC3: usable immediately, and in the curation queue. The row is written
/// `pending`; nothing here approves anything.
///
/// # Errors
///
/// [`PartError::TooManyParts`] when today's allowance is spent.
/// Whether this person has used up today's parts allowance.
///
/// Shared, and it has to be: the queue has more than one entrance. Typing a
/// part inline while recording a modification reaches the same insert without
/// passing through [`suggest`], so a limit that lives only here is a limit on
/// one door of two. A review found exactly that hole.
///
/// A rolling twenty-four hours rather than a calendar day, so the allowance
/// does not refill in a burst at midnight.
///
/// # Errors
///
/// [`sqlx::Error`] when the count fails.
pub(crate) async fn allowance_spent(
    conn: &mut sqlx::PgConnection,
    suggested_by: Uuid,
) -> Result<bool, sqlx::Error> {
    let since = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    Ok(part_repo::suggested_since(conn, suggested_by, since).await? >= PARTS_PER_DAY)
}

pub async fn suggest(
    pool: &PgPool,
    suggested_by: Uuid,
    input: &PartInput,
) -> Result<Uuid, PartError> {
    let mut conn = pool.acquire().await?;

    if allowance_spent(&mut conn, suggested_by).await? {
        return Err(PartError::TooManyParts);
    }

    let id =
        part_repo::find_or_create_pending(&mut conn, Uuid::now_v7(), suggested_by, input).await?;

    tracing::info!(
        category = ?input.category,
        brand = %input.brand,
        "part added to the curation queue"
    );

    Ok(id)
}
