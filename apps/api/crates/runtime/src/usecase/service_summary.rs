//! Cost rollups and service reminders, computed here rather than in the app.
//!
//! AC4 asks that the arithmetic be finished before the data is sent. The
//! reason is not tidiness: four clients — mobile, backoffice, landing,
//! and whatever comes next — each reimplementing "when is this oil change
//! due" is four chances to disagree about a date the owner is trusting.

use std::collections::HashMap;

use anakmobil_domain::garage::policy::{self, LastService, Reminder, Urgency};
use sqlx::PgPool;
use time::Date;
use uuid::Uuid;

use crate::adapter::postgres::summary_repo::{self, CategoryCost, Rollup};
use crate::usecase::service_history::HistoryError;

/// A year, in days. Leap years shift the window by one day, which is not
/// a distinction anybody reading "spent this year" is making.
const A_YEAR: i64 = 365;

/// What one car's row in a list needs.
#[derive(Debug, Clone, Default)]
pub struct ListSummary {
    pub rollup: Option<Rollup>,
    pub overdue_count: usize,
    pub due_soon_count: usize,
}

/// Everything the summary screen shows for one car.
#[derive(Debug, Clone)]
pub struct VehicleSummary {
    pub rollup: Option<Rollup>,
    pub by_category: Vec<CategoryCost>,
    pub reminders: Vec<Reminder>,
}

/// A summary for every car this person owns, keyed by car.
///
/// **Two queries, whatever the number of cars.** One aggregate scan for
/// the totals, one `DISTINCT ON` for the latest service of each kind, and
/// the reminders derived in memory from what came back. A car absent from
/// both simply has no history, and gets the default.
///
/// Complexity: `O(r + c log c)` where `r` is the owner's service records
/// and `c` the categories per car — at most nine, so the sort inside
/// `derive_reminders` is not the term that matters.
///
/// # Errors
///
/// [`HistoryError::Database`] when a query fails.
pub async fn for_list(
    pool: &PgPool,
    owner_id: Uuid,
    today: Date,
) -> Result<HashMap<Uuid, ListSummary>, HistoryError> {
    let since = a_year_before(today);
    let mut conn = pool.acquire().await?;

    let rollups = summary_repo::rollup_owned(&mut conn, owner_id, since).await?;
    let history = summary_repo::last_per_category(&mut conn, owner_id, None).await?;

    let mut grouped: HashMap<Uuid, Vec<LastService>> = HashMap::new();
    for row in history {
        grouped
            .entry(row.vehicle_id)
            .or_default()
            .push(LastService {
                category: row.category.into(),
                date: row.service_date,
                mileage_km: row.mileage_km,
            });
    }

    let mut summaries: HashMap<Uuid, ListSummary> = HashMap::new();
    for rollup in rollups {
        let id = rollup.vehicle_id;
        let odometer = rollup.odometer_km;
        let reminders =
            policy::derive_reminders(grouped.get(&id).map_or(&[], Vec::as_slice), odometer, today);

        summaries.insert(
            id,
            ListSummary {
                rollup: Some(rollup),
                overdue_count: count(&reminders, Urgency::Overdue),
                due_soon_count: count(&reminders, Urgency::Soon),
            },
        );
    }

    Ok(summaries)
}

/// The full summary for one car.
///
/// Ownership is not checked here — the caller has already resolved the
/// car through [`crate::usecase::garage::detail`], and every query below
/// joins through `vehicles` on `owner_id` regardless, so a car that is
/// not this person's yields nothing rather than somebody else's numbers.
///
/// # Errors
///
/// [`HistoryError::Database`] when a query fails.
pub async fn for_vehicle(
    pool: &PgPool,
    owner_id: Uuid,
    vehicle_id: Uuid,
    today: Date,
) -> Result<VehicleSummary, HistoryError> {
    let since = a_year_before(today);
    let mut conn = pool.acquire().await?;

    let rollup = summary_repo::rollup_owned(&mut conn, owner_id, since)
        .await?
        .into_iter()
        .find(|r| r.vehicle_id == vehicle_id);
    let history = summary_repo::last_per_category(&mut conn, owner_id, Some(vehicle_id)).await?;
    let by_category = summary_repo::cost_by_category(&mut conn, owner_id, vehicle_id).await?;

    let latest: Vec<LastService> = history
        .into_iter()
        .map(|row| LastService {
            category: row.category.into(),
            date: row.service_date,
            mileage_km: row.mileage_km,
        })
        .collect();

    let reminders =
        policy::derive_reminders(&latest, rollup.as_ref().and_then(|r| r.odometer_km), today);

    Ok(VehicleSummary {
        rollup,
        by_category,
        reminders,
    })
}

fn count(reminders: &[Reminder], urgency: Urgency) -> usize {
    reminders.iter().filter(|r| r.urgency == urgency).count()
}

/// Clamped rather than unwrapped: a date a year before `today` exists for
/// every date this application will ever see, and the far edge of the
/// calendar is not worth a panic on a summary screen.
fn a_year_before(today: Date) -> Date {
    today.saturating_sub(time::Duration::days(A_YEAR))
}
