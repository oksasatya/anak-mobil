//! Browsing the catalog, and reporting what is missing from it.

use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::adapter::postgres::catalog_repo::{
    self, Brand, CatalogEntry, Generation, SuggestionInput, Variant,
};

/// How many suggestions one person may file in a day.
///
/// The queue is read by a human, so flooding it denies service to curation
/// rather than to the server. Ten is far above what somebody reporting their
/// own missing cars would ever need and far below what makes the queue
/// unusable.
const SUGGESTIONS_PER_DAY: i64 = 10;

/// Why a catalog operation did not succeed.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("too many suggestions today")]
    TooManySuggestions,
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
}

/// Every brand.
///
/// # Errors
///
/// [`CatalogError::Database`] when the query fails.
pub async fn brands(pool: &PgPool) -> Result<Vec<Brand>, CatalogError> {
    let mut conn = pool.acquire().await?;
    Ok(catalog_repo::brands(&mut conn).await?)
}

/// Models under a brand.
///
/// An unknown brand yields an empty list rather than a 404. The client is
/// walking a tree it was just handed, so an empty level is a real answer —
/// and returning one avoids a round trip to distinguish "no models" from
/// "no brand", which the client would treat identically anyway.
///
/// # Errors
///
/// [`CatalogError::Database`] when the query fails.
pub async fn models(pool: &PgPool, brand_id: Uuid) -> Result<Vec<CatalogEntry>, CatalogError> {
    let mut conn = pool.acquire().await?;
    Ok(catalog_repo::models(&mut conn, brand_id).await?)
}

/// Generations of a model.
///
/// # Errors
///
/// [`CatalogError::Database`] when the query fails.
pub async fn generations(pool: &PgPool, model_id: Uuid) -> Result<Vec<Generation>, CatalogError> {
    let mut conn = pool.acquire().await?;
    Ok(catalog_repo::generations(&mut conn, model_id).await?)
}

/// Variants of a generation.
///
/// # Errors
///
/// [`CatalogError::Database`] when the query fails.
pub async fn variants(pool: &PgPool, generation_id: Uuid) -> Result<Vec<Variant>, CatalogError> {
    let mut conn = pool.acquire().await?;
    Ok(catalog_repo::variants(&mut conn, generation_id).await?)
}

/// Report a car the catalog does not have.
///
/// # Errors
///
/// [`CatalogError::TooManySuggestions`] when today's allowance is spent.
pub async fn suggest(
    pool: &PgPool,
    suggested_by: Uuid,
    input: &SuggestionInput,
) -> Result<Uuid, CatalogError> {
    let mut conn = pool.acquire().await?;

    let since = OffsetDateTime::now_utc() - Duration::days(1);
    let filed = catalog_repo::suggestions_since(&mut conn, suggested_by, since).await?;
    if filed >= SUGGESTIONS_PER_DAY {
        return Err(CatalogError::TooManySuggestions);
    }

    let id = Uuid::now_v7();
    catalog_repo::insert_suggestion(&mut conn, id, suggested_by, input).await?;

    // Worth a line: a run of these for one model is the signal that curation
    // should look at it next. No email, no free text — the subject is enough.
    tracing::info!(
        brand = %input.brand,
        model = %input.model,
        "catalog gap reported"
    );

    Ok(id)
}
