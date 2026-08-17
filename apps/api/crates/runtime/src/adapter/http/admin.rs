//! The admin surface.
//!
//! Every handler here takes [`Admin`] rather than [`Authenticated`], which is
//! what makes the authorisation check unforgettable: a route wired to a
//! handler that forgot it does not compile into this module in the first
//! place.
//!
//! # Why an admin's view of a car is the ordinary view
//!
//! [`VehicleResponse`] has no field for a plate, a VIN, or a purchase price —
//! not a skipped field, not an `Option` that happens to be `None`, no field.
//! So redaction here is structural rather than a filter somebody remembered to
//! apply, and the reason `find_private` is never called from this module is
//! that there is nowhere to put what it returns.
//!
//! The summary is left absent for the same reason. `ListSummaryResponse`
//! carries `total_cost`, which is service spend, which `CONTEXT.md` names as
//! private vehicle data — "never leaves the server for anyone who should not
//! see it, including an admin". `GET /vehicles` fills it in because that
//! endpoint answers to the owner. This one does not.

use axum::Json;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::adapter::http::auth::Admin;
use crate::adapter::http::vehicles::VehicleResponse;
use crate::adapter::postgres::user_repo::PlatformRole;
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::response::{ApiResponse, NoContent};
use crate::usecase::garage::{self, GarageError};
use crate::usecase::roles::{self, Actor, RoleChange, RoleError};

/// `GET /admin/users/{id}/vehicles`
///
/// An admin listing one person's cars. No plate, no VIN, no purchase price,
/// and no spend.
///
/// An unknown `id` answers with an empty list rather than a `404`: `users` is
/// not queried, and an admin distinguishing "no cars" from "no such person"
/// through this endpoint would make it a user-id oracle for no gain. The
/// non-admin case never reaches here at all — the extractor refuses first.
///
/// # Errors
///
/// A storage failure.
pub async fn vehicles(
    State(state): State<AppState>,
    // Unused by name and load-bearing by position: constructing it IS the
    // authorisation check, and it runs before this body does.
    _caller: Admin,
    Path(id): Path<Uuid>,
) -> Result<ApiResponse<Vec<VehicleResponse>>, ApiError> {
    let vehicles = garage::list(&state.pool, id).await.map_err(to_api_error)?;

    Ok(ApiResponse::ok(
        vehicles.into_iter().map(VehicleResponse::from).collect(),
    ))
}

/// The single failure-to-response mapping for this module.
///
/// Exhaustive with no `_` arm, so a new [`GarageError`] variant makes this
/// match non-exhaustive and the build fails until somebody decides what it
/// means here. Three of the four arms are unreachable from `garage::list` —
/// it neither resolves an id nor touches the catalog — and they are mapped
/// anyway rather than collapsed into a catch-all, because that is what keeps
/// the compiler reporting the drift.
fn to_api_error(err: GarageError) -> ApiError {
    match err {
        GarageError::NotFound | GarageError::ReorderMismatch => ApiError::not_found(),
        GarageError::UnknownVariant => ApiError::validation(serde_json::json!({
            "variant_id": "Varian tidak ada di katalog."
        })),
        GarageError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}

#[derive(Debug, Deserialize)]
pub struct RoleRequest {
    pub role: PlatformRole,
    /// Why. Stored on the audit row, never returned and never logged.
    pub reason: String,
}

/// What changed.
///
/// No email and no reason: this answers what changed, not who anybody is.
#[derive(Debug, Serialize)]
pub struct RoleChangeResponse {
    pub target_user_id: Uuid,
    pub from_role: PlatformRole,
    pub to_role: PlatformRole,
    pub created_at: String,
}

impl From<RoleChange> for RoleChangeResponse {
    fn from(change: RoleChange) -> Self {
        Self {
            target_user_id: change.target_user_id,
            from_role: change.from_role,
            to_role: change.to_role,
            created_at: change.created_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}

/// `PATCH /admin/users/{id}/role`
///
/// | Situation | Response |
/// |---|---|
/// | Role changed | `200` with the change |
/// | Already in that role | `204`, nothing written |
/// | Target does not exist | `404` |
/// | Caller is not an admin | `403`, before any lookup |
/// | Caller was demoted between the extractor and the lock | `403`, same code and message |
/// | Caller is not signed in | `401` |
///
/// **`204` rather than an error is what makes a retry safe.** A dropped
/// connection after a successful call leaves the client unsure; retrying hits
/// the no-op branch and succeeds.
///
/// **`403` before any lookup is what stops this being a user-id oracle**, and
/// it is automatic: the extractor runs before this body, so a non-admin never
/// reaches a query. An admin receiving `404` for a missing id is not a leak —
/// they are authorised to see the user list.
///
/// # Errors
///
/// See the table above.
pub async fn set_role(
    State(state): State<AppState>,
    caller: Admin,
    Path(id): Path<Uuid>,
    Json(body): Json<RoleRequest>,
) -> Result<Response, ApiError> {
    let change = roles::set_role(
        &state.pool,
        Actor::Admin(caller.user_id),
        id,
        body.role,
        &body.reason,
    )
    .await
    .map_err(to_role_error)?;

    Ok(match change {
        Some(change) => ApiResponse::ok(RoleChangeResponse::from(change)).into_response(),
        None => NoContent.into_response(),
    })
}

/// The single failure-to-response mapping for role changes.
///
/// Exhaustive on purpose: a new [`RoleError`] variant makes this match
/// non-exhaustive and the build fails until somebody decides what it means
/// over HTTP.
fn to_role_error(err: RoleError) -> ApiError {
    match err {
        RoleError::NotFound => ApiError::not_found(),
        // The same code and the same message the extractor would have given.
        // Two different answers for "you are not an admin" would tell a caller
        // which check they tripped.
        RoleError::NotAdmin => ApiError::forbidden(),
        // Unreachable from this handler — only `Actor::Bootstrap` produces it,
        // and this handler always calls with `Actor::Admin` — but the match
        // must stay exhaustive, so it is mapped rather than left as `_` or
        // `unreachable!()` (a panic, which is denied on production paths).
        RoleError::AdminExists => ApiError::conflict(),
        RoleError::InvalidReason(message) => {
            ApiError::validation(serde_json::json!({ "reason": message }))
        }
        RoleError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}
