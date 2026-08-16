//! Searching the parts catalog, and adding to it.
//!
//! A part is reference data, so none of these take an owner. What they do
//! take is the caller's id, recorded as `suggested_by` — curation provenance,
//! not ownership.

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use crate::adapter::postgres::build_repo::{self, EvidenceCount};
use crate::adapter::postgres::part_repo::{self, Part, PartCategory, PartInput};

/// Both evidence counts, zeroed. [`build_repo::evidence_counts`] returns a row
/// of zeros for every id it is asked about, never an absent one — this is the
/// defensive fallback for the id that should never be missing, not the normal
/// path.
fn zero_counts(part_id: Uuid) -> EvidenceCount {
    EvidenceCount {
        part_id,
        active_build_count: 0,
        ever_installed_build_count: 0,
    }
}

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

/// Parts matching a category and a scrap of text, each with its community
/// evidence counts.
///
/// One extra query for the whole page, never one per part — a
/// `count_for_part(id)` helper is the N+1 this function exists to avoid.
/// `variant_id` is `None`: this is the parts catalog, not a fitment check
/// against one car, so the counts are not narrowed to any one variant.
///
/// Complexity: `O(log n + k)` (or `O(n)` with a text filter — see
/// [`part_repo::search`]) for the catalog query, plus `O(k + m)` for the
/// counts over the `k` parts returned and their `m` matching modifications.
/// Two round trips regardless of `k`. The `HashMap` join below is one pass
/// over `counts`, not a `.iter().find()` inside the part loop, which would
/// turn this back into the N+1 the doc comment above warns against.
///
/// # Errors
///
/// [`PartError::Database`] when a query fails.
pub async fn search(
    pool: &PgPool,
    viewer_id: Uuid,
    category: Option<PartCategory>,
    query: Option<&str>,
    limit: u16,
) -> Result<Vec<(Part, EvidenceCount)>, PartError> {
    let mut conn = pool.acquire().await?;
    let parts = part_repo::search(&mut conn, category, query, limit).await?;

    let ids: Vec<Uuid> = parts.iter().map(|part| part.id).collect();
    let mut counts: HashMap<Uuid, EvidenceCount> =
        build_repo::evidence_counts(&mut conn, &ids, viewer_id, None)
            .await?
            .into_iter()
            .map(|count| (count.part_id, count))
            .collect();

    Ok(parts
        .into_iter()
        .map(|part| {
            let count = counts
                .remove(&part.id)
                .unwrap_or_else(|| zero_counts(part.id));
            (part, count)
        })
        .collect())
}

/// One part, with its community evidence counts.
///
/// Complexity: `O(log n)` for the part lookup by primary key, plus
/// `O(m)` for its `m` matching modifications — one part id, so `evidence_counts`
/// does the same work `part_repo::find` would for a `WHERE part_id = $1`
/// version, just through the batched function so there is only ever one
/// shape of this query in the codebase. Two round trips.
///
/// # Errors
///
/// [`PartError::NotFound`] when no part has this id.
pub async fn detail(
    pool: &PgPool,
    viewer_id: Uuid,
    id: Uuid,
) -> Result<(Part, EvidenceCount), PartError> {
    let mut conn = pool.acquire().await?;
    let part = part_repo::find(&mut conn, id)
        .await?
        .ok_or(PartError::NotFound)?;

    let count = build_repo::evidence_counts(&mut conn, &[part.id], viewer_id, None)
        .await?
        .into_iter()
        .find(|c| c.part_id == part.id)
        .unwrap_or_else(|| zero_counts(part.id));

    Ok((part, count))
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
