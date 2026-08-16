//! Service history endpoints.
//!
//! # Cost, and what AC3 means today
//!
//! Cost is stored and returned to the owner. AM-162 requires it filtered by
//! the vehicle's visibility setting for anyone else — and today there is no
//! anyone else: every read joins through `vehicles` on `owner_id`, so no
//! path exists that hands a service record to a stranger.
//!
//! That is a real guarantee but a narrow one, and it is worth naming as such.
//! The filtering AM-162 describes arrives with the story that opens sharing
//! — Explore, a public page, a share link — together with the visibility
//! column and the test that proves a non-owner sees no numbers. Building the
//! setting now would mean guessing the shape of a reader that does not exist.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use time::Date;
use uuid::Uuid;

use crate::adapter::http::auth::Authenticated;
use crate::adapter::postgres::service_repo::{ServiceCategory, ServiceInput};
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::pagination::{DateCursor, Page, clamp_limit};
use crate::shared::response::{ApiResponse, NoContent};
use crate::shared::validation::{self, DecimalSpec};
use crate::usecase::service_history::{self, HistoryError};

/// `service_records.cost` and `modifications.cost` are both `NUMERIC(14, 2)`
/// — the same shape, the same guard.
const MAX_COST: f64 = 999_999_999_999.99;

#[derive(Debug, Serialize)]
pub struct ServiceResponse {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    #[serde(with = "crate::shared::iso_date")]
    pub service_date: Date,
    pub mileage_km: Option<i32>,
    pub category: ServiceCategory,
    pub parts_replaced: Vec<String>,
    pub garage_name: Option<String>,
    /// A decimal string, exact. A JSON number is a double in most clients,
    /// and this is money.
    pub cost: Option<String>,
    pub notes: Option<String>,
}

impl From<crate::adapter::postgres::service_repo::ServiceRecord> for ServiceResponse {
    fn from(r: crate::adapter::postgres::service_repo::ServiceRecord) -> Self {
        Self {
            id: r.id,
            vehicle_id: r.vehicle_id,
            service_date: r.service_date,
            mileage_km: r.mileage_km,
            category: r.category,
            parts_replaced: r.parts_replaced,
            garage_name: r.garage_name,
            // Scale pinned, for the same reason the purchase price is:
            // a BigDecimal keeps whatever scale the round trip left it with.
            cost: r.cost.map(|amount| amount.with_scale(2).to_string()),
            notes: r.notes,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ServiceRequest {
    #[serde(with = "crate::shared::iso_date")]
    pub service_date: Date,
    pub mileage_km: Option<i32>,
    pub category: ServiceCategory,
    #[serde(default)]
    pub parts_replaced: Vec<String>,
    pub garage_name: Option<String>,
    pub cost: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub after: Option<String>,
    pub limit: Option<u16>,
}

impl ServiceRequest {
    fn check(&self) -> Result<ServiceInput, ApiError> {
        let mut problems = serde_json::Map::new();

        // A service dated in the future is a typo, and it would sit above
        // every real record forever.
        if self.service_date > today() {
            problems.insert(
                "service_date".into(),
                "Tanggal servis belum terjadi.".into(),
            );
        }
        if self.mileage_km.is_some_and(|km| km < 0) {
            problems.insert("mileage_km".into(), "Tidak boleh negatif.".into());
        }

        // The same guard `parts.rs` uses, not a bare parse-and-check-the-sign.
        // A bare parse accepted "1200000.755" and silently lost the last
        // digit to Postgres's rounding, and accepted a value past
        // NUMERIC(14, 2)'s range as a `22003 numeric field overflow` that
        // reached the client as a 500 instead of a 422 — the same defect
        // `modifications.cost` was built to avoid from the first commit.
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

        Ok(ServiceInput {
            service_date: self.service_date,
            mileage_km: self.mileage_km,
            category: self.category,
            parts_replaced: self
                .parts_replaced
                .iter()
                .map(|part| part.trim().to_owned())
                .filter(|part| !part.is_empty())
                .collect(),
            garage_name: self
                .garage_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
            cost,
            notes: self
                .notes
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
        })
    }
}

/// The server's today, in UTC.
///
/// Indonesia is up to seven hours ahead, so a service recorded late in the
/// evening there is "tomorrow" to a UTC server. The check allows that day
/// rather than rejecting a record for being in a time zone.
///
/// `pub(super)` rather than private: `builds.rs` needs the identical
/// allowance for `install_date`, and re-deriving it there would be a second
/// place to keep in step with this one.
pub(super) fn today() -> Date {
    time::OffsetDateTime::now_utc()
        .date()
        .next_day()
        .unwrap_or_else(|| time::OffsetDateTime::now_utc().date())
}

/// `GET /vehicles/{id}/services`
///
/// # Errors
///
/// An unreadable cursor, or a storage failure.
pub async fn list(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(vehicle_id): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Result<ApiResponse<Page<ServiceResponse>>, ApiError> {
    // Rejected rather than ignored. Silently starting from the beginning
    // would make a client's paging loop repeat forever without ever failing.
    let after = query
        .after
        .as_deref()
        .map(DateCursor::decode)
        .transpose()
        .map_err(|_| {
            ApiError::validation(serde_json::json!({ "after": "Cursor tidak dikenali." }))
        })?;

    let page = service_history::page(
        &state.pool,
        caller.user_id,
        vehicle_id,
        after.as_ref(),
        clamp_limit(query.limit),
    )
    .await
    .map_err(to_api_error)?;

    Ok(ApiResponse::ok(Page {
        items: page.items.into_iter().map(Into::into).collect(),
        next_cursor: page.next_cursor,
    }))
}

/// `POST /vehicles/{id}/services`
///
/// # Errors
///
/// Validation failure, the car is not the caller's, or a storage failure.
pub async fn create(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(vehicle_id): Path<Uuid>,
    Json(body): Json<ServiceRequest>,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    let input = body.check()?;
    let id = service_history::record(&state.pool, caller.user_id, vehicle_id, &input)
        .await
        .map_err(to_api_error)?;

    Ok(ApiResponse::created(serde_json::json!({ "id": id })))
}

/// `GET /services/{id}`
///
/// # Errors
///
/// Not found, or a storage failure.
pub async fn detail(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(id): Path<Uuid>,
) -> Result<ApiResponse<ServiceResponse>, ApiError> {
    let record = service_history::detail(&state.pool, caller.user_id, id)
        .await
        .map_err(to_api_error)?;
    Ok(ApiResponse::ok(record.into()))
}

/// `PUT /services/{id}`
///
/// # Errors
///
/// Validation failure, not found, or a storage failure.
pub async fn update(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(id): Path<Uuid>,
    Json(body): Json<ServiceRequest>,
) -> Result<NoContent, ApiError> {
    let input = body.check()?;
    service_history::amend(&state.pool, caller.user_id, id, &input)
        .await
        .map_err(to_api_error)?;
    Ok(NoContent)
}

/// `DELETE /services/{id}`
///
/// # Errors
///
/// Not found, or a storage failure.
pub async fn delete(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(id): Path<Uuid>,
) -> Result<NoContent, ApiError> {
    service_history::remove(&state.pool, caller.user_id, id)
        .await
        .map_err(to_api_error)?;
    Ok(NoContent)
}

pub(super) fn to_api_error(err: HistoryError) -> ApiError {
    match err {
        HistoryError::NotFound => ApiError::not_found(),
        HistoryError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn a_request() -> ServiceRequest {
        ServiceRequest {
            service_date: Date::from_calendar_date(2026, Month::January, 10).expect("a real date"),
            mileage_km: Some(140_200),
            category: ServiceCategory::EngineOil,
            parts_replaced: vec!["Filter oli".to_owned()],
            garage_name: Some("Bengkel XYZ".to_owned()),
            cost: Some("1200000".to_owned()),
            notes: None,
        }
    }

    #[test]
    fn a_normal_record_is_accepted() {
        assert!(a_request().check().is_ok());
    }

    #[test]
    fn a_service_dated_in_the_future_is_refused() {
        let mut request = a_request();
        request.service_date = Date::from_calendar_date(2099, Month::January, 1).expect("a date");
        assert!(request.check().is_err());
    }

    #[test]
    fn a_negative_cost_is_refused() {
        let mut request = a_request();
        request.cost = Some("-500".to_owned());
        assert!(request.check().is_err());
    }

    #[test]
    fn a_cost_that_is_not_a_number_is_refused() {
        let mut request = a_request();
        request.cost = Some("satu juta dua ratus".to_owned());
        assert!(request.check().is_err());
    }

    #[test]
    fn a_cost_keeps_its_exact_value() {
        let mut request = a_request();
        request.cost = Some("1200000.75".to_owned());
        let parsed = request.check().expect("valid").cost.expect("a cost");
        assert_eq!(parsed.to_string(), "1200000.75");
    }

    #[test]
    fn a_cost_the_column_cannot_hold_is_refused_rather_than_rounded() {
        // `service_records.cost` is NUMERIC(14, 2), the same shape as
        // `modifications.cost`. A bare parse-and-sign-check let this through,
        // Postgres rounded it silently, and the idempotent path this guard
        // exists for elsewhere would have failed on a duplicate request.
        let mut request = a_request();
        request.cost = Some("1200000.755".to_owned());
        assert!(request.check().is_err());
    }

    #[test]
    fn a_cost_past_the_columns_range_is_refused_here_rather_than_by_the_database() {
        // Past NUMERIC(14, 2)'s twelve integer digits: a `22003 numeric field
        // overflow` reaching the caller as a 500, on the endpoint whose own
        // criteria promise a 422.
        let mut request = a_request();
        request.cost = Some("9999999999999999".to_owned());
        assert!(request.check().is_err());
    }

    #[test]
    fn blank_parts_are_dropped_rather_than_stored() {
        let mut request = a_request();
        request.parts_replaced = vec!["  ".to_owned(), " Filter udara ".to_owned(), String::new()];

        let input = request.check().expect("valid");
        assert_eq!(input.parts_replaced, vec!["Filter udara".to_owned()]);
    }

    #[test]
    fn a_record_with_no_cost_is_fine() {
        // Plenty of services are logged without the receipt to hand.
        let mut request = a_request();
        request.cost = None;
        assert!(request.check().expect("valid").cost.is_none());
    }

    #[test]
    fn todays_service_is_allowed() {
        // The obvious case, and the one an off-by-one in the future check
        // would break for every user in Indonesia at once.
        let mut request = a_request();
        request.service_date = time::OffsetDateTime::now_utc().date();
        assert!(request.check().is_ok());
    }

    #[test]
    fn a_service_recorded_tonight_in_jakarta_is_allowed() {
        // Indonesia runs up to seven hours ahead, so "today" there can be
        // tomorrow in UTC. Rejecting it would mean the app stops accepting
        // records every evening.
        let mut request = a_request();
        request.service_date = time::OffsetDateTime::now_utc()
            .date()
            .next_day()
            .expect("tomorrow exists");
        assert!(request.check().is_ok());
    }

    #[test]
    fn a_category_round_trips_through_its_wire_form() {
        // The wire form is the contract a client switches on.
        let json = serde_json::to_string(&ServiceCategory::Suspension).expect("serialising");
        assert_eq!(json, "\"suspension\"");
    }
}
