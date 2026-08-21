//! Reading and changing the caller's own profile.
//!
//! Separate from [`crate::usecase::auth`], which owns credentials and sessions.
//! Nothing here authenticates; every function takes a user id the HTTP layer
//! already proved.

use sqlx::PgPool;
use uuid::Uuid;

use crate::adapter::postgres::user_repo::{self, Profile};

/// Why a profile operation did not succeed.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    /// The session outlived the row it points at. Refused rather than assumed.
    #[error("no such account")]
    NotFound,
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
}

/// This account's identity and derived onboarding state.
///
/// # Errors
///
/// [`ProfileError::NotFound`] when the account is gone, otherwise a storage
/// error.
pub async fn me(pool: &PgPool, user_id: Uuid) -> Result<Profile, ProfileError> {
    let mut conn = pool.acquire().await?;
    user_repo::profile_of(&mut conn, user_id)
        .await?
        .ok_or(ProfileError::NotFound)
}

/// Set the display name and answer with the profile that results.
///
/// One transaction: the write and the read-back cannot straddle a concurrent
/// change, so the response is the state that was actually committed rather than
/// a hopeful echo of the request.
///
/// # Errors
///
/// [`ProfileError::NotFound`] when the account is gone, otherwise a storage
/// error.
pub async fn update_display_name(
    pool: &PgPool,
    user_id: Uuid,
    display_name: &str,
) -> Result<Profile, ProfileError> {
    let mut tx = pool.begin().await?;
    user_repo::set_display_name(&mut tx, user_id, display_name).await?;
    let profile = user_repo::profile_of(&mut tx, user_id)
        .await?
        .ok_or(ProfileError::NotFound)?;
    tx.commit().await?;
    Ok(profile)
}
