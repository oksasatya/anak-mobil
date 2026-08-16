//! Cost rollups and reminders, on the wire.
//!
//! Every money field is a decimal string, for the reason the rest of the
//! garage API is: a JSON number is a double in most clients, and this is
//! money.

use axum::extract::{Path, State};
use serde::Serialize;
use sqlx::types::BigDecimal;
use time::Date;
use uuid::Uuid;

use crate::adapter::http::auth::Authenticated;
use crate::adapter::postgres::service_repo::ServiceCategory;
use crate::adapter::postgres::summary_repo::Rollup;
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::response::ApiResponse;
use crate::usecase::service_summary::{self, ListSummary, VehicleSummary};

/// What a car's row in the garage list carries.
///
/// Counts rather than the reminders themselves: a list row shows "2
/// terlambat", and sending nine reminder objects per car so the client
/// can count them is the payload equivalent of the N+1 this endpoint
/// exists to avoid.
#[derive(Debug, Serialize)]
pub struct ListSummaryResponse {
    pub service_count: i64,
    pub total_cost: Option<String>,
    #[serde(
        with = "crate::shared::iso_date::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_service_date: Option<Date>,
    pub overdue_count: usize,
    pub due_soon_count: usize,
}

impl From<ListSummary> for ListSummaryResponse {
    fn from(summary: ListSummary) -> Self {
        let rollup = summary.rollup;
        Self {
            service_count: rollup.as_ref().map_or(0, |r| r.service_count),
            total_cost: rollup.as_ref().and_then(|r| money(r.total_cost.as_ref())),
            last_service_date: rollup.and_then(|r| r.last_service_date),
            overdue_count: summary.overdue_count,
            due_soon_count: summary.due_soon_count,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CategoryCostResponse {
    pub category: ServiceCategory,
    pub service_count: i64,
    pub total_cost: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReminderResponse {
    pub category: ServiceCategory,
    #[serde(with = "crate::shared::iso_date")]
    pub due_on: Date,
    /// Negative when the date has passed.
    pub days_remaining: i64,
    pub due_at_km: Option<i32>,
    pub km_remaining: Option<i32>,
    /// `overdue`, `soon`, or `later`.
    pub urgency: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SummaryResponse {
    pub service_count: i64,
    pub total_cost: Option<String>,
    /// Spent in the last twelve months.
    pub cost_last_year: Option<String>,
    #[serde(
        with = "crate::shared::iso_date::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_service_date: Option<Date>,
    /// The highest odometer reading ever recorded, which is what the
    /// distance reminders are measured against. Sent so the number on
    /// screen and the number behind the reminder are visibly the same.
    pub odometer_km: Option<i32>,
    pub by_category: Vec<CategoryCostResponse>,
    pub reminders: Vec<ReminderResponse>,
}

impl From<VehicleSummary> for SummaryResponse {
    fn from(summary: VehicleSummary) -> Self {
        let rollup: Option<Rollup> = summary.rollup;
        Self {
            service_count: rollup.as_ref().map_or(0, |r| r.service_count),
            total_cost: rollup.as_ref().and_then(|r| money(r.total_cost.as_ref())),
            cost_last_year: rollup.as_ref().and_then(|r| money(r.cost_since.as_ref())),
            last_service_date: rollup.as_ref().and_then(|r| r.last_service_date),
            odometer_km: rollup.and_then(|r| r.odometer_km),
            by_category: summary
                .by_category
                .into_iter()
                .map(|c| CategoryCostResponse {
                    category: c.category,
                    service_count: c.service_count,
                    total_cost: money(c.total_cost.as_ref()),
                })
                .collect(),
            reminders: summary
                .reminders
                .into_iter()
                .map(|r| ReminderResponse {
                    category: r.category.into(),
                    due_on: r.due_on,
                    days_remaining: r.days_remaining,
                    due_at_km: r.due_at_km,
                    km_remaining: r.km_remaining,
                    urgency: match r.urgency {
                        anakmobil_domain::garage::policy::Urgency::Overdue => "overdue",
                        anakmobil_domain::garage::policy::Urgency::Soon => "soon",
                        anakmobil_domain::garage::policy::Urgency::Later => "later",
                    },
                })
                .collect(),
        }
    }
}

/// Scale pinned at two: a `BigDecimal` otherwise keeps whatever scale the
/// database round trip left it with, and `SUM` over mixed scales is where
/// that shows up as `4200000.5000` on a screen.
fn money(amount: Option<&BigDecimal>) -> Option<String> {
    amount.map(|value| value.clone().with_scale(2).to_string())
}

/// `GET /vehicles/{id}/summary`
///
/// # Errors
///
/// A storage failure. A car that is not the caller's yields an empty
/// summary rather than an error, because every query behind it joins
/// through `vehicles` on `owner_id` — there is nothing to leak, and
/// distinguishing "not yours" from "no history" would say whether the id
/// is real.
pub async fn vehicle(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(vehicle_id): Path<Uuid>,
) -> Result<ApiResponse<SummaryResponse>, ApiError> {
    let summary = service_summary::for_vehicle(&state.pool, caller.user_id, vehicle_id, today())
        .await
        .map_err(super::services::to_api_error)?;

    Ok(ApiResponse::ok(summary.into()))
}

/// The server's today, in UTC.
pub fn today() -> Date {
    time::OffsetDateTime::now_utc().date()
}
