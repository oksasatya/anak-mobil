//! The caller's own profile, and the username namespace.
//!
//! `profile.rs` rather than `identity.rs`: `auth.rs` is already the identity
//! adapter, and these endpoints are specifically about the profile a person
//! shows the world rather than about proving who they are.

use axum::Json;
use axum::extract::{ConnectInfo, Path, State};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::adapter::http::auth::Authenticated;
use crate::adapter::postgres::user_repo::Profile;
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::response::ApiResponse;
use crate::usecase::profile::{self, ProfileError};

/// What the app reads once at launch to decide where to send somebody.
///
/// `username` and `display_name` are nullable and that is the whole point:
/// their absence is how the client knows onboarding is unfinished. There is no
/// completion flag — `has_vehicles` is derived from the garage, so it cannot
/// disagree with the garage.
#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: Uuid,
    pub email: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub has_vehicles: bool,
}

impl From<Profile> for MeResponse {
    fn from(profile: Profile) -> Self {
        Self {
            id: profile.id,
            email: profile.email,
            username: profile.username,
            display_name: profile.display_name,
            has_vehicles: profile.has_vehicles,
        }
    }
}

/// The longest display name a profile header renders without truncating.
const MAX_DISPLAY_NAME: usize = 60;

#[derive(Debug, Deserialize)]
pub struct ProfileUpdateRequest {
    /// Absent means "leave it alone". serde defaults an `Option` field to
    /// `None` when the key is missing, which is what makes an empty PATCH a
    /// no-op rather than a rejection.
    pub display_name: Option<String>,
}

impl ProfileUpdateRequest {
    /// The trimmed name to write, or `None` when the request asks for no change.
    fn check(&self) -> Result<Option<&str>, ApiError> {
        let Some(raw) = self.display_name.as_deref() else {
            return Ok(None);
        };
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(field_error("display_name", "Nama tidak boleh kosong."));
        }
        if trimmed.chars().count() > MAX_DISPLAY_NAME {
            return Err(field_error(
                "display_name",
                "Nama terlalu panjang. Maksimal 60 karakter.",
            ));
        }
        // `trim()` only strips `char::is_whitespace` from the edges, so a name
        // made entirely of zero-width spaces, an embedded RTL override, or a
        // control character survives it and passes every check above.
        // `has_vehicles` and `display_name` are the two facts the onboarding
        // gate reads — a non-null, non-empty `display_name` satisfies it, so
        // this is a byline that renders as nothing (or reads backwards) while
        // the account looks fully onboarded.
        //
        // `char::is_control` alone is not enough: it covers Unicode category
        // Cc, but the zero-width space, the byte-order mark, and every bidi
        // override/isolate character (the RTL-spoof exploit) are category Cf
        // — verified empirically, `'\u{202E}'.is_control()` is `false`. Both
        // checks are load-bearing.
        if trimmed
            .chars()
            .any(|c| c.is_control() || is_invisible_or_bidi_control(c))
            || !trimmed.chars().any(char::is_alphanumeric)
        {
            return Err(field_error(
                "display_name",
                "Nama harus berisi huruf atau angka.",
            ));
        }

        Ok(Some(trimmed))
    }
}

fn field_error(field: &'static str, message: &'static str) -> ApiError {
    ApiError::validation(serde_json::json!({ field: message }))
}

/// Unicode "Format" (Cf) characters that hide text or reorder it: zero-width
/// space/joiners, the byte-order mark, and every bidirectional
/// override/embedding/isolate control — the code points behind the
/// right-to-left display-name spoof. No dependency needed for a fixed,
/// well-known range; per ledger 81, `unicode-normalization` is deliberately
/// not pulled in for this crate.
const fn is_invisible_or_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200F}' // ZWSP, ZWNJ, ZWJ, LRM, RLM
            | '\u{202A}'..='\u{202E}' // LRE, RLE, PDF, LRO, RLO
            | '\u{2066}'..='\u{2069}' // LRI, RLI, FSI, PDI
            | '\u{FEFF}' // BOM / zero-width no-break space
    )
}

/// `GET /me`
///
/// # Errors
///
/// A storage failure, or an account that no longer exists.
pub async fn me(
    State(state): State<AppState>,
    caller: Authenticated,
) -> Result<ApiResponse<MeResponse>, ApiError> {
    let profile = profile::me(&state.pool, caller.user_id)
        .await
        .map_err(to_api_error)?;
    Ok(ApiResponse::ok(profile.into()))
}

/// `PATCH /me`
///
/// Answers with the same shape as [`me`], so a client writes one parser.
///
/// # Errors
///
/// Validation failure, a storage failure, or an account that no longer exists.
pub async fn update_me(
    State(state): State<AppState>,
    caller: Authenticated,
    Json(body): Json<ProfileUpdateRequest>,
) -> Result<ApiResponse<MeResponse>, ApiError> {
    let profile = match body.check()? {
        Some(display_name) => {
            profile::update_display_name(&state.pool, caller.user_id, display_name)
                .await
                .map_err(to_api_error)?
        }
        None => profile::me(&state.pool, caller.user_id)
            .await
            .map_err(to_api_error)?,
    };

    Ok(ApiResponse::ok(profile.into()))
}

/// `GET /usernames/{username}/availability`
///
/// The one endpoint here that answers without a session, because the register
/// screen needs it before there is one.
///
/// **Taken and reserved answer identically**, and that is the guard rail the
/// whole endpoint rests on. A username is a public namespace — `/@username` is
/// a real address — so "this one is taken" is not the leak that "this email has
/// an account" would be. What would be a leak is distinguishing a name somebody
/// holds from a name the platform keeps, so nothing does.
///
/// The response never accepts, returns, or correlates an email or any account
/// state. One name in, one boolean out.
///
/// # Errors
///
/// A malformed name, too many lookups from this address, or a storage failure.
pub async fn availability(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(raw): Path<String>,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    // Fails closed, like the login limiter: an unreachable Redis means the
    // bound cannot be enforced, and an unthrottled namespace scrape is worse
    // than a brief refusal.
    let allowed = state
        .limiter
        .allow_lookup(&peer.ip().to_string())
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;
    if !allowed {
        return Err(ApiError::too_many_requests());
    }

    let username = crate::adapter::http::auth::check_username(&raw)?;

    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;
    let taken = crate::adapter::postgres::user_repo::username_exists(&mut conn, &username)
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;

    // `taken || reserved`, collapsed into one boolean before it leaves the
    // process. Two fields, or two codes, would be the distinction this endpoint
    // exists to avoid making.
    let available = !taken && !anakmobil_domain::identity::username::is_reserved(&username);

    Ok(ApiResponse::ok(
        serde_json::json!({ "available": available }),
    ))
}

fn to_api_error(err: ProfileError) -> ApiError {
    match err {
        // A session that outlived its account. Refused rather than assumed —
        // the same failure-closed choice the Admin extractor makes.
        ProfileError::NotFound => ApiError::unauthorized(),
        ProfileError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(display_name: Option<&str>) -> ProfileUpdateRequest {
        ProfileUpdateRequest {
            display_name: display_name.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn an_absent_field_asks_for_no_change() {
        assert_eq!(request(None).check().ok().flatten(), None);
    }

    #[test]
    fn whitespace_is_trimmed_rather_than_stored() {
        assert_eq!(
            request(Some("  Budi Santoso  ")).check().ok().flatten(),
            Some("Budi Santoso")
        );
    }

    #[test]
    fn a_name_of_only_spaces_is_not_a_name() {
        assert!(request(Some("   ")).check().is_err());
        assert!(request(Some("")).check().is_err());
    }

    #[test]
    fn sixty_zero_width_spaces_are_not_a_name() {
        // Passes `trim()` (not whitespace by the Unicode property `trim`
        // strips) and the length bound (60 chars, exactly at the limit) —
        // renders as nothing in every byline while satisfying the onboarding
        // gate's "display_name is not null" check.
        assert!(request(Some(&"\u{200B}".repeat(60))).check().is_err());
    }

    #[test]
    fn a_lone_byte_order_mark_is_not_a_name() {
        assert!(request(Some("\u{FEFF}")).check().is_err());
    }

    #[test]
    fn embedded_newlines_are_refused() {
        assert!(request(Some("Budi\nSantoso")).check().is_err());
    }

    #[test]
    fn a_right_to_left_override_is_refused() {
        // The classic display-name spoof: reverses the rest of the rendered
        // string. `char::is_control` alone (Unicode category Cc) does not
        // cover this — U+202E is category Cf — so this depends on the
        // `is_alphanumeric` half failing to hold if `is_control` is ever
        // narrowed; both arms of the predicate are load-bearing.
        assert!(request(Some("Budi\u{202E}Santoso")).check().is_err());
    }

    #[test]
    fn an_embedded_nul_is_refused() {
        assert!(request(Some("Budi\u{0}Santoso")).check().is_err());
    }

    #[test]
    fn an_ordinary_name_with_real_letters_still_passes() {
        // The guard must not become so aggressive it refuses real names —
        // Indonesian names are plain Latin letters and spaces.
        assert!(request(Some("Budi Santoso")).check().is_ok());
    }

    #[test]
    fn the_length_bound_counts_characters() {
        // Sixty emoji is sixty characters and 240 bytes. Counting bytes would
        // refuse a name well inside the limit.
        assert!(request(Some(&"a".repeat(MAX_DISPLAY_NAME))).check().is_ok());
        assert!(
            request(Some(&"a".repeat(MAX_DISPLAY_NAME + 1)))
                .check()
                .is_err()
        );
    }
}
