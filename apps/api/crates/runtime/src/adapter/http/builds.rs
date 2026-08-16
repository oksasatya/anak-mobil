//! Build and modification endpoints.
//!
//! # Why one request type handles both `part_id` and an inline `part`
//!
//! Adding a modification and amending one both need to say which part it is,
//! and a caller who does not know the part's id yet needs to be able to type
//! it — the same choice `POST /parts` offers, reused here rather than copied.
//! [`ModificationRequest::check`] resolves that choice once; the two
//! endpoints differ only in whether a build is created on demand.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use time::Date;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::adapter::http::auth::Authenticated;
use crate::adapter::postgres::build_repo::{
    Build, BuildInput, BuildPhoto, Modification, ModificationInput, Visibility,
};
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::pagination::{Page, clamp_limit};
use crate::shared::response::{ApiResponse, NoContent};
use crate::shared::validation::{self, DecimalSpec};
use crate::usecase::builds::{self, BuildDetail, BuildError, PartChoice};

/// `modifications.cost` is `NUMERIC(14, 2)`, the same shape as
/// `service_records.cost`.
const MAX_COST: f64 = 999_999_999_999.99;

#[derive(Debug, Serialize)]
pub struct ModificationResponse {
    pub id: Uuid,
    pub build_id: Uuid,
    pub part_id: Uuid,
    #[serde(with = "crate::shared::iso_date::option")]
    pub install_date: Option<Date>,
    pub mileage_km: Option<i32>,
    /// A decimal string, exact — the same reason every money field on this
    /// server is one.
    pub cost: Option<String>,
    pub garage_name: Option<String>,
    pub notes: Option<String>,
    /// Present once the part has been taken off. The row itself never
    /// disappears — `usecase::builds::remove_modification` only sets this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed_at: Option<String>,
}

impl From<Modification> for ModificationResponse {
    fn from(m: Modification) -> Self {
        Self {
            id: m.id,
            build_id: m.build_id,
            part_id: m.part_id,
            install_date: m.install_date,
            mileage_km: m.mileage_km,
            cost: m.cost.map(|amount| amount.with_scale(2).to_string()),
            garage_name: m.garage_name,
            notes: m.notes,
            removed_at: m
                .removed_at
                .map(|at| at.format(&Rfc3339).unwrap_or_default()),
        }
    }
}

impl ModificationResponse {
    /// The same mapping as [`From<Modification>`], except `cost` goes through
    /// [`visible_cost`] instead of always being shown.
    ///
    /// Every other place this type is built already knows the caller owns
    /// the car — `for_vehicle` and the modification-write endpoints all fail
    /// with [`BuildError::NotFound`] before reaching a response otherwise.
    /// `GET /builds` is the one place that is not true: the build on the
    /// page may belong to somebody else, and only its `visibility` said the
    /// caller may see the build at all — never its cost.
    fn filtered(
        m: Modification,
        cost_visibility: Visibility,
        owner_id: Uuid,
        viewer_id: Uuid,
    ) -> Self {
        let cost = visible_cost(m.cost.clone(), cost_visibility, owner_id, viewer_id);
        Self {
            cost,
            ..Self::from(m)
        }
    }
}

/// The cost, if this viewer may see it.
///
/// Filtered here, server-side, from the `cost_visibility` the row already
/// carries — no extra query, and the client is never trusted to hide a
/// number. A cost that reaches the wire cannot be recalled.
///
/// What each of the three settings means for a modification's cost:
/// - `Private` — only the vehicle's owner. Everyone else gets `null`.
/// - `Community` — any signed-in caller, which today is every caller: every
///   endpoint on this server requires authentication, the same fact
///   [`crate::adapter::postgres::build_repo::find_build_visible`] notes for
///   the build itself.
/// - `Public` — the same as `Community`, for the same reason, until an
///   unauthenticated surface exists to actually tell them apart.
///
/// The owner's own view is unconditional: `owner_id == viewer_id`
/// short-circuits the setting, so a `Private` build never hides its own
/// numbers from the person who paid them.
fn visible_cost(
    cost: Option<BigDecimal>,
    setting: Visibility,
    owner_id: Uuid,
    viewer_id: Uuid,
) -> Option<String> {
    let allowed = owner_id == viewer_id || setting != Visibility::Private;
    allowed
        .then_some(cost)
        .flatten()
        .map(|amount| amount.with_scale(2).to_string())
}

#[derive(Debug, Serialize)]
pub struct BuildResponse {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub notes: Option<String>,
    pub visibility: Visibility,
    pub modifications: Vec<ModificationResponse>,
}

impl BuildResponse {
    fn from_parts(build: Build, modifications: Vec<Modification>) -> Self {
        Self {
            id: build.id,
            vehicle_id: build.vehicle_id,
            notes: build.notes,
            visibility: build.visibility,
            modifications: modifications.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BuildPhotoResponse {
    pub id: Uuid,
    pub object_key: String,
    pub position: i32,
}

impl From<BuildPhoto> for BuildPhotoResponse {
    fn from(p: BuildPhoto) -> Self {
        Self {
            id: p.id,
            object_key: p.object_key,
            position: p.position,
        }
    }
}

/// One row of `GET /builds` — a build the caller may see, its modifications
/// with `cost` filtered per [`visible_cost`], and its photos.
#[derive(Debug, Serialize)]
pub struct BuildListItem {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub notes: Option<String>,
    pub visibility: Visibility,
    pub nickname: Option<String>,
    pub described_as: Option<String>,
    pub modifications: Vec<ModificationResponse>,
    pub photos: Vec<BuildPhotoResponse>,
}

impl BuildListItem {
    fn from_detail(detail: BuildDetail, viewer_id: Uuid) -> Self {
        let BuildDetail {
            build,
            modifications,
            photos,
        } = detail;
        let cost_visibility = build.cost_visibility;
        let owner_id = build.owner_id;

        Self {
            id: build.id,
            vehicle_id: build.vehicle_id,
            notes: build.notes,
            visibility: build.visibility,
            nickname: build.nickname,
            described_as: build.described_as,
            modifications: modifications
                .into_iter()
                .map(|m| ModificationResponse::filtered(m, cost_visibility, owner_id, viewer_id))
                .collect(),
            photos: photos.into_iter().map(Into::into).collect(),
        }
    }
}

fn default_visibility() -> Visibility {
    Visibility::Private
}

#[derive(Debug, Deserialize)]
pub struct BuildRequest {
    pub notes: Option<String>,
    /// An absent field must mean private, never a silent widening of who
    /// sees a build — the same rule `vehicles.rs` applies to
    /// `cost_visibility`.
    #[serde(default = "default_visibility")]
    pub visibility: Visibility,
}

impl BuildRequest {
    fn to_input(&self) -> BuildInput {
        BuildInput {
            notes: self
                .notes
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
            visibility: self.visibility,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ModificationRequest {
    /// A part already in the catalog.
    pub part_id: Option<Uuid>,
    /// Or one being typed for the first time. Reuses the parts request so the
    /// range validation exists once — a second copy of those bounds is a
    /// second thing to keep in step with the `CHECK` constraints.
    pub part: Option<super::parts::PartRequest>,

    #[serde(default, with = "crate::shared::iso_date::option")]
    pub install_date: Option<Date>,
    pub mileage_km: Option<i32>,
    pub cost: Option<String>,
    pub garage_name: Option<String>,
    pub notes: Option<String>,
}

impl ModificationRequest {
    /// Which part, and the rest of the fields.
    ///
    /// Both or neither is a validation failure rather than a silent
    /// preference. A caller who sends both has a bug, and picking one for them
    /// hides it until the wrong part is attached to somebody's build.
    ///
    /// `ModificationInput::part_id` in the returned value is a placeholder
    /// (`Uuid::nil()`) — the use case always overwrites it once the part
    /// choice is resolved inside its transaction (`Existing` and `New` both
    /// go through the same replacement in `usecase::builds`), so the value
    /// written here is never read.
    fn check(&self) -> Result<(PartChoice, ModificationInput), ApiError> {
        let choice = match (self.part_id, &self.part) {
            (Some(id), None) => PartChoice::Existing(id),
            (None, Some(request)) => PartChoice::New(Box::new(request.check()?)),
            (Some(_), Some(_)) => {
                return Err(ApiError::validation(serde_json::json!({
                    "part": "Isi part_id atau part, jangan keduanya."
                })));
            }
            (None, None) => {
                return Err(ApiError::validation(serde_json::json!({
                    "part": "Wajib diisi: part_id atau part."
                })));
            }
        };

        let mut problems = serde_json::Map::new();

        if self.mileage_km.is_some_and(|km| km < 0) {
            problems.insert("mileage_km".into(), "Tidak boleh negatif.".into());
        }
        // Mirrors `modifications_install_not_in_the_future`: without this,
        // a future date reaches Postgres as a CHECK violation and surfaces
        // as a 500 where a 422 is correct — the same defect class as an
        // unguarded cost. `super::services::today()` rather than a second
        // copy: Indonesia runs up to seven hours ahead of the UTC server, so
        // "today" there can be tomorrow here.
        if self
            .install_date
            .is_some_and(|date| date > super::services::today())
        {
            problems.insert(
                "install_date".into(),
                "Tanggal pasang belum terjadi.".into(),
            );
        }

        let cost = validation::decimal(
            DecimalSpec {
                field: "cost",
                raw: self.cost.as_deref(),
                min: 0.0,
                max: MAX_COST,
                scale: 2,
            },
            &mut problems,
        );

        if !problems.is_empty() {
            return Err(ApiError::validation(serde_json::Value::Object(problems)));
        }

        Ok((
            choice,
            ModificationInput {
                part_id: Uuid::nil(),
                install_date: self.install_date,
                mileage_km: self.mileage_km,
                cost,
                garage_name: self
                    .garage_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned),
                notes: self
                    .notes
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned),
            },
        ))
    }
}

#[derive(Debug, Deserialize)]
pub struct BuildsQuery {
    /// A plain build id, not an encoded [`crate::shared::pagination::DateCursor`]
    /// — build ids are UUID v7 and therefore already sortable, per
    /// [`crate::adapter::postgres::build_repo::page_visible`].
    pub after: Option<Uuid>,
    pub limit: Option<u16>,
}

/// `GET /builds`
///
/// A page of builds this caller may see — their own, plus every
/// `community`/`public` build — newest first. A build the caller may not see
/// is absent from the page, never present with its fields blanked.
///
/// # Errors
///
/// A storage failure.
pub async fn list(
    State(state): State<AppState>,
    caller: Authenticated,
    Query(query): Query<BuildsQuery>,
) -> Result<ApiResponse<Page<BuildListItem>>, ApiError> {
    let page = builds::page(
        &state.pool,
        caller.user_id,
        query.after,
        clamp_limit(query.limit),
    )
    .await
    .map_err(to_api_error)?;

    Ok(ApiResponse::ok(Page {
        items: page
            .items
            .into_iter()
            .map(|detail| BuildListItem::from_detail(detail, caller.user_id))
            .collect(),
        next_cursor: page.next_cursor,
    }))
}

/// `PUT /vehicles/{id}/build`
///
/// # Errors
///
/// Not found, or a storage failure.
pub async fn save(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(vehicle_id): Path<Uuid>,
    Json(body): Json<BuildRequest>,
) -> Result<NoContent, ApiError> {
    builds::save(&state.pool, caller.user_id, vehicle_id, &body.to_input())
        .await
        .map_err(to_api_error)?;
    Ok(NoContent)
}

/// `GET /vehicles/{id}/build`
///
/// # Errors
///
/// Not found, or a storage failure.
pub async fn detail(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(vehicle_id): Path<Uuid>,
) -> Result<ApiResponse<BuildResponse>, ApiError> {
    let (build, modifications) = builds::for_vehicle(&state.pool, caller.user_id, vehicle_id)
        .await
        .map_err(to_api_error)?;
    Ok(ApiResponse::ok(BuildResponse::from_parts(
        build,
        modifications,
    )))
}

/// `POST /vehicles/{id}/build/modifications`
///
/// # Errors
///
/// Validation failure, the car is not the caller's, or a storage failure.
pub async fn add_modification(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(vehicle_id): Path<Uuid>,
    Json(body): Json<ModificationRequest>,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    let (choice, input) = body.check()?;
    let id = builds::add_modification(&state.pool, caller.user_id, vehicle_id, choice, &input)
        .await
        .map_err(to_api_error)?;

    Ok(ApiResponse::created(serde_json::json!({ "id": id })))
}

/// `PUT /modifications/{id}`
///
/// # Errors
///
/// Validation failure, not found, or a storage failure.
pub async fn update_modification(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(id): Path<Uuid>,
    Json(body): Json<ModificationRequest>,
) -> Result<NoContent, ApiError> {
    let (choice, input) = body.check()?;
    builds::amend_modification(&state.pool, caller.user_id, id, choice, &input)
        .await
        .map_err(to_api_error)?;
    Ok(NoContent)
}

/// `DELETE /modifications/{id}`
///
/// Idempotent: a second call still answers `204` rather than `404`, because a
/// client retrying a request that already succeeded must not be told the
/// thing does not exist.
///
/// # Errors
///
/// Not found, or a storage failure.
pub async fn remove_modification(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(id): Path<Uuid>,
) -> Result<NoContent, ApiError> {
    builds::remove_modification(&state.pool, caller.user_id, id)
        .await
        .map_err(to_api_error)?;
    Ok(NoContent)
}

fn to_api_error(err: BuildError) -> ApiError {
    match err {
        BuildError::NotFound => ApiError::not_found(),
        BuildError::TooManyParts => ApiError::too_many_requests(),
        // A foreign key violation here means the caller named a `part_id` that
        // does not exist — a well-formed but stale UUID, which is an ordinary
        // thing for a client holding an old list. Postgres 23503 reaching the
        // caller as a 500 is the same defect class this module guards every
        // numeric field against, applied to the one foreign key.
        BuildError::Database(sqlx::Error::Database(inner))
            if inner.code().as_deref() == Some("23503") =>
        {
            ApiError::not_found()
        }
        BuildError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::postgres::part_repo::PartCategory;
    use time::Month;

    fn a_part_request() -> super::super::parts::PartRequest {
        super::super::parts::PartRequest {
            category: PartCategory::Wheels,
            brand: "Enkei".to_owned(),
            product_name: "RPF1".to_owned(),
            wheel_diameter_in: Some("18".to_owned()),
            wheel_width_in: Some("8.5".to_owned()),
            offset_et_mm: Some(40),
            pcd_bolt_count: Some(5),
            pcd_diameter_mm: Some("114.3".to_owned()),
            center_bore_mm: Some("73.1".to_owned()),
            tyre_width_mm: None,
            tyre_aspect_ratio: None,
            tyre_rim_diameter_in: None,
            suspension_kind: None,
            spring_rate_kgmm: None,
        }
    }

    fn a_request() -> ModificationRequest {
        ModificationRequest {
            part_id: Some(Uuid::now_v7()),
            part: None,
            install_date: None,
            mileage_km: Some(1_000),
            cost: Some("1500000".to_owned()),
            garage_name: Some("Bengkel ABC".to_owned()),
            notes: None,
        }
    }

    #[test]
    fn an_existing_part_id_is_accepted() {
        let (choice, _) = a_request().check().expect("valid");
        assert!(matches!(choice, PartChoice::Existing(_)));
    }

    #[test]
    fn sending_both_part_id_and_part_is_refused() {
        let mut request = a_request();
        request.part = Some(a_part_request());
        assert!(request.check().is_err());
    }

    #[test]
    fn sending_neither_part_id_nor_part_is_refused() {
        let mut request = a_request();
        request.part_id = None;
        assert!(request.check().is_err());
    }

    #[test]
    fn an_inline_part_is_accepted() {
        let mut request = a_request();
        request.part_id = None;
        request.part = Some(a_part_request());
        let (choice, _) = request.check().expect("valid");
        assert!(matches!(choice, PartChoice::New(_)));
    }

    #[test]
    fn an_invalid_inline_part_is_refused() {
        // The inline branch reuses `PartRequest::check`, so its own range
        // guards apply here too rather than a second, weaker copy.
        let mut request = a_request();
        request.part_id = None;
        let mut part = a_part_request();
        part.pcd_bolt_count = Some(0);
        request.part = Some(part);
        assert!(request.check().is_err());
    }

    #[test]
    fn a_negative_mileage_is_refused() {
        let mut request = a_request();
        request.mileage_km = Some(-1);
        assert!(request.check().is_err());
    }

    #[test]
    fn a_cost_keeps_its_exact_value() {
        let mut request = a_request();
        request.cost = Some("1500000.75".to_owned());
        let (_, input) = request.check().expect("valid");
        assert_eq!(input.cost.expect("a cost").to_string(), "1500000.75");
    }

    #[test]
    fn a_cost_the_column_cannot_hold_is_refused_rather_than_rounded() {
        // The defect this closes: a bare parse let "1200000.755" through,
        // Postgres rounded it silently on insert, and the idempotent re-read
        // this endpoint relies on elsewhere would then match nothing.
        let mut request = a_request();
        request.cost = Some("1200000.755".to_owned());
        assert!(request.check().is_err());
    }

    #[test]
    fn a_cost_past_the_columns_range_is_refused_here_rather_than_by_the_database() {
        let mut request = a_request();
        request.cost = Some("9999999999999999".to_owned());
        assert!(request.check().is_err());
    }

    #[test]
    fn a_negative_cost_is_refused() {
        let mut request = a_request();
        request.cost = Some("-500".to_owned());
        assert!(request.check().is_err());
    }

    #[test]
    fn an_install_date_far_in_the_future_is_refused() {
        let mut request = a_request();
        request.install_date =
            Some(Date::from_calendar_date(2099, Month::January, 1).expect("a real date"));
        assert!(request.check().is_err());
    }

    #[test]
    fn todays_install_date_is_allowed() {
        let mut request = a_request();
        request.install_date = Some(time::OffsetDateTime::now_utc().date());
        assert!(request.check().is_ok());
    }

    #[test]
    fn an_install_date_recorded_tonight_in_jakarta_is_allowed() {
        // Indonesia runs up to seven hours ahead, so "today" there can be
        // tomorrow in UTC — the same allowance `services.rs` gives
        // `service_date`.
        let mut request = a_request();
        request.install_date = Some(
            time::OffsetDateTime::now_utc()
                .date()
                .next_day()
                .expect("tomorrow exists"),
        );
        assert!(request.check().is_ok());
    }
}
