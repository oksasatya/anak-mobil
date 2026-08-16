//! Rollups over service history.
//!
//! Everything here answers for **every one of the owner's cars in one
//! query**, even when the caller wants a single car. That is the whole
//! point: a garage list showing "Rp 4.2jt, 2 servis terlambat" per row is
//! the exact shape that becomes one query per car without anybody
//! noticing, because the loop is over rendered rows rather than over
//! `await`s. AM-360's technical note names it directly.
//!
//! Summing money is left to the database rather than to the domain. A
//! rollup computed in Rust would mean loading every service record a car
//! has ever had in order to add up a single number.

use sqlx::PgConnection;
use sqlx::types::BigDecimal;
use time::Date;
use uuid::Uuid;

use crate::adapter::postgres::service_repo::ServiceCategory;

/// One car's totals.
#[derive(Debug, Clone)]
pub struct Rollup {
    pub vehicle_id: Uuid,
    pub service_count: i64,
    pub total_cost: Option<BigDecimal>,
    pub cost_since: Option<BigDecimal>,
    pub last_service_date: Option<Date>,
    /// The highest reading ever recorded, which is the best estimate of
    /// where the car is now. It is conservative by construction: a car
    /// driven since its last logged service is further along than this.
    pub odometer_km: Option<i32>,
}

/// The latest service of each kind, for one car or for all of them.
#[derive(Debug, Clone)]
pub struct CategoryLast {
    pub vehicle_id: Uuid,
    pub category: ServiceCategory,
    pub service_date: Date,
    pub mileage_km: Option<i32>,
}

/// What has been spent per kind of work.
#[derive(Debug, Clone)]
pub struct CategoryCost {
    pub category: ServiceCategory,
    pub service_count: i64,
    pub total_cost: Option<BigDecimal>,
}

/// Totals per car, for every car this person owns.
///
/// `since` bounds the second total — pass today minus twelve months for
/// "spent in the last year". It is an argument rather than
/// `CURRENT_DATE - INTERVAL '12 months'` so a test can put the car
/// anywhere in time, the same reason the reminder policy takes `today`.
///
/// Cars with no history at all are absent from the result rather than
/// present with zeroes; the caller fills them in, because only the caller
/// knows the full set of cars.
///
/// Complexity: one aggregate scan of the owner's records, `O(n)` in the
/// records they have — not `O(cars)` queries.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn rollup_owned(
    conn: &mut PgConnection,
    owner_id: Uuid,
    since: Date,
) -> Result<Vec<Rollup>, sqlx::Error> {
    sqlx::query_as!(
        Rollup,
        r#"
        SELECT
            s.vehicle_id                                        AS "vehicle_id!",
            COUNT(*)                                            AS "service_count!",
            SUM(s.cost)                                         AS "total_cost",
            SUM(s.cost) FILTER (WHERE s.service_date >= $2)     AS "cost_since",
            MAX(s.service_date)                                 AS "last_service_date",
            MAX(s.mileage_km)                                   AS "odometer_km"
        FROM service_records s
        JOIN vehicles v ON v.id = s.vehicle_id
        WHERE v.owner_id = $1
        GROUP BY s.vehicle_id
        "#,
        owner_id,
        since,
    )
    .fetch_all(conn)
    .await
}

/// The latest service of each kind, for one car or for all of them.
///
/// `DISTINCT ON` takes the first row of each group in the `ORDER BY`, so
/// the database returns at most nine rows per car instead of the whole
/// history for the caller to fold. Passing `None` for `vehicle_id` covers
/// every car in the same single query.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn last_per_category(
    conn: &mut PgConnection,
    owner_id: Uuid,
    vehicle_id: Option<Uuid>,
) -> Result<Vec<CategoryLast>, sqlx::Error> {
    sqlx::query_as!(
        CategoryLast,
        r#"
        SELECT DISTINCT ON (s.vehicle_id, s.category)
            s.vehicle_id,
            s.category AS "category: ServiceCategory",
            s.service_date,
            s.mileage_km
        FROM service_records s
        JOIN vehicles v ON v.id = s.vehicle_id
        WHERE v.owner_id = $1
          AND ($2::uuid IS NULL OR s.vehicle_id = $2)
        ORDER BY s.vehicle_id, s.category, s.service_date DESC, s.id DESC
        "#,
        owner_id,
        vehicle_id,
    )
    .fetch_all(conn)
    .await
}

/// Spend per kind of work, for one car.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn cost_by_category(
    conn: &mut PgConnection,
    owner_id: Uuid,
    vehicle_id: Uuid,
) -> Result<Vec<CategoryCost>, sqlx::Error> {
    sqlx::query_as!(
        CategoryCost,
        r#"
        SELECT
            s.category   AS "category: ServiceCategory",
            COUNT(*)     AS "service_count!",
            SUM(s.cost)  AS "total_cost"
        FROM service_records s
        JOIN vehicles v ON v.id = s.vehicle_id
        WHERE v.owner_id = $1 AND s.vehicle_id = $2
        GROUP BY s.category
        ORDER BY SUM(s.cost) DESC NULLS LAST, s.category
        "#,
        owner_id,
        vehicle_id,
    )
    .fetch_all(conn)
    .await
}
