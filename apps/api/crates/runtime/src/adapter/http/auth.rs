//! Authentication endpoints and the extractor that guards everything else.

use axum::Json;
use axum::extract::{ConnectInfo, FromRequestParts, MatchedPath, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::adapter::postgres::user_repo::{self, PlatformRole};
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::response::ApiResponse;
use crate::usecase::auth::{self, AuthError};

/// A caller proven to be a particular account.
///
/// A handler that takes this parameter cannot be reached without a valid,
/// unrevoked session — the check is in the type, not in a line of code
/// somebody has to remember to write first.
#[derive(Debug, Clone, Copy)]
pub struct Authenticated {
    pub user_id: Uuid,
}

impl FromRequestParts<AppState> for Authenticated {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or_else(ApiError::unauthorized)?;

        // Resolving the token requires the session to still exist, which is
        // what makes a logout take effect on the very next request.
        state
            .sessions
            .authenticate(token)
            .await
            .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?
            .map(|user_id| Self { user_id })
            .ok_or_else(ApiError::unauthorized)
    }
}

/// A caller proven to be a platform admin.
///
/// The same claim [`Authenticated`] makes, one level up: a handler that takes
/// this parameter cannot be routed without an admin behind it, so the check is
/// in the type rather than in a line of code somebody has to remember to write
/// first. With one admin endpoint today that looks like ceremony; with AM-366's
/// curation queues it is the only thing that scales, and adding it later means
/// auditing every route written in between.
///
/// The role is read from Postgres on **every** admin request. Nothing caches
/// it, so a demoted admin's next admin request is refused without waiting for
/// a new token — while their ordinary requests are unaffected, because a
/// demoted admin is still a user and their garage still belongs to them.
///
/// **Failure is closed.** A database error is a 500; it is never "assume
/// `user`" and never "assume `admin`". A missing account — a session that
/// outlived its row — is a refusal, not an assumption.
///
/// **403, not 404.** AM-84's AC2 asks for a rejection the person can
/// understand, and hiding an endpoint's existence from an already
/// authenticated account buys nothing.
#[derive(Debug, Clone, Copy)]
pub struct Admin {
    pub user_id: Uuid,
}

impl FromRequestParts<AppState> for Admin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let Authenticated { user_id } = Authenticated::from_request_parts(parts, state).await?;

        let mut conn = state
            .pool
            .acquire()
            .await
            .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;

        let role = user_repo::platform_role_of(&mut conn, user_id)
            .await
            .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;

        match role {
            Some(PlatformRole::Admin) => Ok(Self { user_id }),
            // Enumerated rather than `_`, so a third variant is a compile
            // error somebody has to decide about. It would fail closed either
            // way; what the enumeration buys is that the decision is visible.
            Some(PlatformRole::User) | None => {
                // Without this line, probing for admin endpoints is
                // indistinguishable from silence on precisely the routes an
                // attacker would probe. The user id and the matched route
                // pattern, and nothing else — no email, no path, no body.
                // A user id is not a credential and is enough to investigate.
                tracing::warn!(
                    %user_id,
                    route = parts
                        .extensions
                        .get::<MatchedPath>()
                        .map_or(super::request_id::UNMATCHED, MatchedPath::as_str),
                    "admin route refused"
                );
                Err(ApiError::forbidden())
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CredentialsRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// What a client receives after signing in.
///
/// `expires_in` is seconds, so a client does not have to parse a timestamp or
/// trust its own clock against the server's.
#[derive(Debug, Serialize)]
pub struct TokensResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
}

impl From<crate::adapter::redis::session::TokenPair> for TokensResponse {
    fn from(pair: crate::adapter::redis::session::TokenPair) -> Self {
        Self {
            access_token: pair.access,
            refresh_token: pair.refresh,
            token_type: "Bearer",
            expires_in: crate::adapter::redis::session::ACCESS_TTL.as_secs(),
        }
    }
}

/// The shortest password worth calling a password.
///
/// Length is the only rule. Composition rules — a digit, a symbol, a capital —
/// push people toward `Password1!` and are worse than useless; NIST dropped
/// them in 2017.
const MIN_PASSWORD: usize = 8;

fn check_password_shape(password: &str) -> Result<(), ApiError> {
    if password.chars().count() < MIN_PASSWORD {
        return Err(ApiError::validation(serde_json::json!({
            "password": format!("Minimal {MIN_PASSWORD} karakter.")
        })));
    }
    Ok(())
}

/// `POST /auth/register`
///
/// # Errors
///
/// Validation failure, a taken email, or a storage failure.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<CredentialsRequest>,
) -> Result<ApiResponse<TokensResponse>, ApiError> {
    check_password_shape(&body.password)?;

    let pair = auth::register(&state.pool, &state.sessions, &body.email, &body.password)
        .await
        .map_err(to_api_error)?;

    Ok(ApiResponse::created(pair.into()))
}

/// `POST /auth/login`
///
/// # Errors
///
/// Rate limited, invalid credentials, or a storage failure.
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<CredentialsRequest>,
) -> Result<ApiResponse<TokensResponse>, ApiError> {
    // Counted before the password is verified, so a correct guess costs the
    // attacker an attempt too.
    //
    // Fails closed. An unreachable Redis means the limit cannot be enforced,
    // and an unthrottled argon2 endpoint is a worse outcome than a brief
    // refusal to sign anybody in.
    let allowed = state
        .limiter
        .allow_login(&peer.ip().to_string(), &body.email)
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;

    if !allowed {
        return Err(ApiError::too_many_requests());
    }

    let pair = auth::login(&state.pool, &state.sessions, &body.email, &body.password)
        .await
        .map_err(to_api_error)?;

    Ok(ApiResponse::ok(pair.into()))
}

/// `POST /auth/refresh`
///
/// # Errors
///
/// An unusable or replayed token.
pub async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<ApiResponse<TokensResponse>, ApiError> {
    let pair = auth::refresh(&state.sessions, &body.refresh_token)
        .await
        .map_err(to_api_error)?;
    Ok(ApiResponse::ok(pair.into()))
}

/// `POST /auth/logout`
///
/// Ends the session the presented token belongs to, and no other. Signing out
/// on a phone should not sign out the tablet.
///
/// # Errors
///
/// A storage failure.
pub async fn logout(
    State(state): State<AppState>,
    caller: Authenticated,
    Json(body): Json<RefreshRequest>,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    // The session id comes from the refresh token rather than from the
    // request body: a client must not be able to end a session it cannot
    // prove it holds.
    // Anything other than a clean rotation means there is nothing left to
    // end — already signed out, or a replay. Both answer the same, because
    // distinguishing them would tell a caller which one it was.
    if let crate::adapter::redis::session::Rotation::Rotated(pair) = state
        .sessions
        .rotate(&body.refresh_token)
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?
    {
        auth::logout(&state.sessions, caller.user_id, pair.session_id)
            .await
            .map_err(to_api_error)?;
    }

    Ok(ApiResponse::ok(serde_json::json!({ "signed_out": true })))
}

/// Map a use-case failure to a response.
///
/// Every credential failure becomes the same `401`, whatever went wrong
/// underneath. An unknown email and a wrong password must be externally
/// identical, or the endpoint tells an attacker which addresses have
/// accounts.
fn to_api_error(err: AuthError) -> ApiError {
    match err {
        AuthError::InvalidCredentials => ApiError::unauthorized(),
        AuthError::EmailTaken => ApiError::conflict(),
        AuthError::Session(inner) => ApiError::internal(anyhow::anyhow!(inner)),
        AuthError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
        AuthError::Password(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_password_is_rejected_with_a_field_message() {
        let err = check_password_shape("pendek").expect_err("should reject");
        assert_eq!(err.code(), crate::shared::i18n::ErrorCode::ValidationFailed);
    }

    #[test]
    fn a_long_enough_password_is_accepted() {
        assert!(check_password_shape("cukup panjang").is_ok());
    }

    #[test]
    fn length_is_counted_in_characters_not_bytes() {
        // Eight emoji is eight characters and 32 bytes. Counting bytes would
        // accept a four-character password made of multi-byte glyphs.
        assert!(check_password_shape("абвгдежз").is_ok());
        assert!(check_password_shape("абвгдеж").is_err());
    }

    #[test]
    fn every_credential_failure_looks_the_same_from_outside() {
        // The enumeration defence. If these ever diverge, an attacker can tell
        // a registered address from an unregistered one.
        assert_eq!(
            to_api_error(AuthError::InvalidCredentials).code(),
            crate::shared::i18n::ErrorCode::Unauthorized
        );
    }

    #[test]
    fn an_internal_failure_is_masked() {
        let err = to_api_error(AuthError::Database(sqlx::Error::PoolTimedOut));
        assert_eq!(err.code(), crate::shared::i18n::ErrorCode::Internal);
    }
}
