//! Registering, signing in, refreshing, and signing out.
//!
//! This layer owns the transaction and the decisions. The Postgres repository
//! runs SQL, the Redis store keeps sessions, and neither of them branches on a
//! business rule.

use sqlx::PgPool;
use uuid::Uuid;

use crate::adapter::postgres::user_repo::{self, Credentials};
use crate::adapter::redis::session::{Rotation, SessionError, SessionStore, TokenPair};
use crate::shared::security::{self, PasswordError};

/// Why an authentication attempt did not succeed.
///
/// `InvalidCredentials` covers an unknown email, a wrong password, and a
/// stored hash that will not parse. They are one variant on purpose: the
/// caller must not be able to tell them apart, because doing so turns login
/// into a way to discover which email addresses have accounts.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("that email is already registered")]
    EmailTaken,
    #[error("the session could not be reached")]
    Session(#[from] SessionError),
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
    #[error("password hashing failed")]
    Password(#[from] PasswordError),
}

/// Create an account and sign it in.
///
/// # Errors
///
/// [`AuthError::EmailTaken`] when the address is registered, otherwise a
/// storage error.
pub async fn register(
    pool: &PgPool,
    sessions: &SessionStore,
    email: &str,
    password: &str,
) -> Result<TokenPair, AuthError> {
    let hash = security::hash_password(password)?;
    let id = Uuid::now_v7();

    let mut tx = pool.begin().await?;
    let result = user_repo::insert(&mut tx, id, email, &hash).await;

    match result {
        Ok(()) => {
            tx.commit().await?;
            Ok(sessions.create(id).await?)
        }
        // 23505 is a unique violation, which here can only be the email index.
        Err(sqlx::Error::Database(err)) if err.code().as_deref() == Some("23505") => {
            Err(AuthError::EmailTaken)
        }
        Err(err) => Err(err.into()),
    }
}

/// Verify a password and start a session.
///
/// Takes the same time whether the email is registered or not. Returning
/// early on a missing account would let an attacker enumerate addresses by
/// timing alone, without ever guessing a password — so a missing account
/// still pays for one argon2 verification against a decoy hash.
///
/// # Errors
///
/// [`AuthError::InvalidCredentials`] for any credential failure.
pub async fn login(
    pool: &PgPool,
    sessions: &SessionStore,
    email: &str,
    password: &str,
) -> Result<TokenPair, AuthError> {
    let mut conn = pool.acquire().await?;
    let found = user_repo::find_credentials(&mut conn, email).await?;
    drop(conn);

    let (id, hash) = match &found {
        Some(Credentials { id, password_hash }) => (Some(*id), password_hash.as_str()),
        None => (None, security::decoy_hash()),
    };

    // Runs on both paths. `verify_password` returning an error means the
    // stored hash is corrupt, which is an operational problem — but the
    // client is told the same thing either way.
    let matches = security::verify_password(password, hash).unwrap_or(false);

    match (id, matches) {
        (Some(id), true) => Ok(sessions.create(id).await?),
        _ => Err(AuthError::InvalidCredentials),
    }
}

/// Exchange a refresh token for a new pair.
///
/// A token presented after it was already exchanged is treated as theft: the
/// legitimate holder would have moved on to the replacement, so a replay means
/// a copy exists somewhere it should not. Every session for the account is
/// revoked — but the account is **not** fenced, because the owner has to be
/// able to sign in again. Fencing here would turn a stolen token into a
/// permanent lockout of the victim.
///
/// # Errors
///
/// [`AuthError::InvalidCredentials`] for an unusable token, including a
/// detected replay.
pub async fn refresh(sessions: &SessionStore, refresh_token: &str) -> Result<TokenPair, AuthError> {
    match sessions.rotate(refresh_token).await? {
        Rotation::Rotated(pair) => Ok(*pair),
        Rotation::Reused { user_id } => {
            let revoked = sessions.revoke_all(user_id, false).await?;
            // No email, no token, no digest — the user id is enough to
            // investigate and is not a credential.
            tracing::warn!(
                %user_id,
                sessions_revoked = revoked,
                "refresh token replayed after rotation, revoking every session"
            );
            Err(AuthError::InvalidCredentials)
        }
        Rotation::Invalid => Err(AuthError::InvalidCredentials),
    }
}

/// End one session.
///
/// # Errors
///
/// [`AuthError::Session`] when the store is unreachable.
pub async fn logout(
    sessions: &SessionStore,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<(), AuthError> {
    sessions.revoke(user_id, session_id).await?;
    Ok(())
}

/// End every session an account has, and stop it starting new ones.
///
/// This is what account deletion calls. The fence is the difference between
/// this and the response to a detected replay: a deleted account must never
/// authenticate again, and a robbed one must.
///
/// # Errors
///
/// [`AuthError::Session`] when the store is unreachable.
pub async fn revoke_every_session(
    sessions: &SessionStore,
    user_id: Uuid,
) -> Result<usize, AuthError> {
    Ok(sessions.revoke_all(user_id, true).await?)
}
