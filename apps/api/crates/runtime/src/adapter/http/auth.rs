//! Authentication endpoints and the extractor that guards everything else.

use anakmobil_domain::identity::username::{self, UsernameError};
use axum::Json;
use axum::extract::{ConnectInfo, FromRequestParts, MatchedPath, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::adapter::postgres::user_repo::{self, PlatformRole};
use crate::adapter::redis::rate_limit::LoginAttempt;
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
    /// The session this token belongs to, so a handler that must end it does
    /// not have to rediscover it. Every handler that only wants `user_id`
    /// ignores this field and is unaffected.
    pub session_id: Uuid,
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
            .map(|resolved| Self {
                user_id: resolved.user_id,
                session_id: resolved.session_id,
            })
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
        let Authenticated { user_id, .. } = Authenticated::from_request_parts(parts, state).await?;

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

/// What `/auth/login` takes, and nothing else.
///
/// Registration has its own type. Sharing one would have meant adding
/// `username` here, which would make the shipped login contract demand a field
/// no client sends.
#[derive(Debug, Deserialize)]
pub struct CredentialsRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegistrationRequest {
    pub email: String,
    pub username: String,
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

/// The Bahasa Indonesia message for each way a username can be wrong.
///
/// The domain crate has no i18n and should not grow one — its `#[error]`
/// strings are English, for logs. Product text is written here, where the rest
/// of this file's field messages are.
pub(super) const fn username_message(err: UsernameError) -> &'static str {
    match err {
        UsernameError::TooShort => "Minimal 3 karakter.",
        UsernameError::TooLong => "Maksimal 30 karakter.",
        UsernameError::BadCharacter => "Hanya huruf kecil, angka, titik, dan garis bawah.",
        UsernameError::EdgePunctuation => {
            "Tidak boleh diawali atau diakhiri titik atau garis bawah."
        }
        UsernameError::ConsecutiveDots => "Titik tidak boleh berurutan.",
    }
}

/// Canonicalise a username, or fail with a message under the field.
pub(super) fn check_username(raw: &str) -> Result<String, ApiError> {
    username::canonicalise(raw).map_err(|err| {
        ApiError::validation(serde_json::json!({ "username": username_message(err) }))
    })
}

/// `POST /auth/register`
///
/// # Errors
///
/// Validation failure, a taken email or username, or a storage failure.
pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<RegistrationRequest>,
) -> Result<ApiResponse<TokensResponse>, ApiError> {
    // Counted first, before any validation runs. `usecase::auth::register`
    // hashes the password with argon2 before the uniqueness check ever
    // touches the database (`usecase/auth.rs::register`), so a throttled
    // probe must be refused here or it pays that cost anyway. Fails closed,
    // like the login and lookup limiters: an unreachable Redis means the
    // bound cannot be enforced, and an unthrottled register is worse than a
    // brief refusal.
    let allowed = state
        .limiter
        .allow_register(&peer.ip().to_string())
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;
    if !allowed {
        return Err(ApiError::too_many_requests());
    }

    check_password_shape(&body.password)?;
    let username = check_username(&body.username)?;

    // A reserved name answers exactly as a taken one does — same status, same
    // body (`tests/auth_flow.rs::a_reserved_username_answers_exactly_like_a_taken_one`).
    // Reporting it as a validation failure instead would answer 422 where a
    // taken name answers 409, and that difference is a way to enumerate the
    // reserved list.
    //
    // The response is identical; the TIMING is not, and that is a known,
    // accepted gap rather than a second guarantee this line makes. This
    // branch returns before `security::hash_password` runs, so a reserved
    // name costs well under a millisecond while a taken or available one
    // costs argon2 (~20ms, per that function's own doc) plus a database round
    // trip. What leaks through the gap is membership of this twelve-entry
    // public constant — and `GET /usernames/{name}/availability` (Task 7) is
    // the endpoint designed to answer exactly that question, at the same
    // low cost for both reserved and taken. Closing the timing gap here would
    // not hide anything the availability endpoint does not already publish
    // on purpose.
    if username::is_reserved(&username) {
        return Err(to_api_error(AuthError::UsernameTaken));
    }

    let pair = auth::register(
        &state.pool,
        &state.sessions,
        auth::Registration {
            email: &body.email,
            username: &username,
            password: &body.password,
        },
    )
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
    let attempt = state
        .limiter
        .allow_login(&peer.ip().to_string(), &body.email)
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;

    if let LoginAttempt::Refused {
        retry_after_seconds,
    } = attempt
    {
        return Err(ApiError::too_many_requests_in(retry_after_seconds));
    }

    let pair = auth::login(&state.pool, &state.sessions, &body.email, &body.password)
        .await
        .map_err(to_login_error)?;

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
/// Ends the session the access token belongs to, and no other. Signing out on
/// a phone should not sign out the tablet.
///
/// The session id comes from the extractor, which resolved it from the token
/// the caller actually presented — so a client still cannot end a session it
/// cannot prove it holds, and no refresh token is spent to find out which one
/// it is. Rotating to discover the session was the old approach, and a rotation
/// that came back `Reused` or `Invalid` left this handler answering success
/// having revoked nothing.
///
/// A logout against an already-dead session never reaches here: the extractor
/// refuses it as unauthenticated, exactly as it refuses any other request
/// carrying a revoked token. Nothing distinguishes the two cases for a caller.
///
/// # Errors
///
/// A storage failure.
pub async fn logout(
    State(state): State<AppState>,
    caller: Authenticated,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    auth::logout(&state.sessions, caller.user_id, caller.session_id)
        .await
        .map_err(to_api_error)?;

    Ok(ApiResponse::ok(serde_json::json!({ "signed_out": true })))
}

/// Map a use-case failure to a response.
///
/// Every credential failure becomes the same `401`, whatever went wrong
/// underneath — an unknown email, a wrong password, and (via `/auth/refresh`)
/// an expired or replayed token are all [`AuthError::InvalidCredentials`] and
/// must stay externally identical, or the endpoint tells an attacker which
/// addresses have accounts. `/auth/login` reports a friendlier message
/// through [`to_login_error`] instead; the enumeration guarantee lives in the
/// variant staying one variant, not in which message it renders.
fn to_api_error(err: AuthError) -> ApiError {
    match err {
        AuthError::InvalidCredentials => ApiError::unauthorized(),
        AuthError::EmailTaken => ApiError::conflict_on("email", "Email ini sudah terdaftar."),
        AuthError::UsernameTaken => {
            ApiError::conflict_on("username", "Username ini sudah dipakai.")
        }
        AuthError::Session(inner) => ApiError::internal(anyhow::anyhow!(inner)),
        AuthError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
        AuthError::Password(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}

/// Map a login failure to a response.
///
/// Identical to [`to_api_error`] except for [`AuthError::InvalidCredentials`]:
/// `/auth/login` answers "email atau password salah", never "you need to sign
/// in first" — the message that variant gets everywhere else, including
/// `/auth/refresh`, where it is exactly right. Telling someone typing their
/// password wrong that they need to sign in is nonsense; telling someone
/// whose refresh token expired the same thing is correct. Both messages cover
/// an unknown email and a wrong password identically, so neither weakens the
/// enumeration defence.
fn to_login_error(err: AuthError) -> ApiError {
    match err {
        AuthError::InvalidCredentials => ApiError::invalid_credentials(),
        other => to_api_error(other),
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
    fn each_username_error_carries_its_own_message() {
        // Five Bahasa Indonesia strings with zero coverage before this test:
        // collapsing all five arms to one string, or transposing `TooShort`
        // and `TooLong`, left every other test green while the product told
        // somebody who typed two characters "Maksimal 30 karakter." — pinning
        // uniqueness alone would not catch a transposition, since the five
        // messages would still be five distinct strings, just on the wrong
        // variants. Each assertion below pins the exact text per variant.
        assert_eq!(
            username_message(UsernameError::TooShort),
            "Minimal 3 karakter."
        );
        assert_eq!(
            username_message(UsernameError::TooLong),
            "Maksimal 30 karakter."
        );
        assert_eq!(
            username_message(UsernameError::BadCharacter),
            "Hanya huruf kecil, angka, titik, dan garis bawah."
        );
        assert_eq!(
            username_message(UsernameError::EdgePunctuation),
            "Tidak boleh diawali atau diakhiri titik atau garis bawah."
        );
        assert_eq!(
            username_message(UsernameError::ConsecutiveDots),
            "Titik tidak boleh berurutan."
        );
    }

    #[test]
    fn every_credential_failure_looks_the_same_from_outside() {
        // The enumeration defence, on the generic mapper `/auth/refresh` (and
        // `register`/`logout`, which never actually produce this variant)
        // still use. If this ever diverges by branch, an attacker can tell a
        // registered address from an unregistered one.
        assert_eq!(
            to_api_error(AuthError::InvalidCredentials).code(),
            crate::shared::i18n::ErrorCode::Unauthorized
        );
    }

    #[test]
    fn login_reports_the_login_specific_credential_message() {
        // `/auth/login` alone gets the friendlier code — see `to_login_error`.
        assert_eq!(
            to_login_error(AuthError::InvalidCredentials).code(),
            crate::shared::i18n::ErrorCode::InvalidCredentials
        );
    }

    #[test]
    fn an_internal_failure_is_masked() {
        let err = to_api_error(AuthError::Database(sqlx::Error::PoolTimedOut));
        assert_eq!(err.code(), crate::shared::i18n::ErrorCode::Internal);
    }
}
