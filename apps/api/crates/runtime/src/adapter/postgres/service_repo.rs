//! Reading and writing service history.
//!
//! Every function takes the owner and joins through `vehicles`, so there is
//! no path that reaches a service record without saying whose car it belongs
//! to — the same discipline as the vehicle repository, for the same reason.

use sqlx::PgConnection;
use sqlx::types::BigDecimal;
use time::Date;
use uuid::Uuid;

use anakmobil_domain::garage::policy;

use crate::shared::pagination::DateCursor;

/// The taxonomy from the feature breakdown, mirrored from the Postgres enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "service_category", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ServiceCategory {
    OliMesin,
    OliTransmisi,
    Rem,
    KakiKaki,
    TuneUp,
    Ac,
    Kelistrikan,
    Body,
    Lainnya,
}

impl From<ServiceCategory> for policy::ServiceCategory {
    /// The persistence enum is not the domain enum, deliberately: this
    /// one carries the sqlx and serde derives that the domain crate has
    /// no dependencies for. The cost is this match, and the compiler
    /// refuses to let it go stale — adding a variant on either side makes
    /// it non-exhaustive.
    fn from(category: ServiceCategory) -> Self {
        match category {
            ServiceCategory::OliMesin => Self::OliMesin,
            ServiceCategory::OliTransmisi => Self::OliTransmisi,
            ServiceCategory::Rem => Self::Rem,
            ServiceCategory::KakiKaki => Self::KakiKaki,
            ServiceCategory::TuneUp => Self::TuneUp,
            ServiceCategory::Ac => Self::Ac,
            ServiceCategory::Kelistrikan => Self::Kelistrikan,
            ServiceCategory::Body => Self::Body,
            ServiceCategory::Lainnya => Self::Lainnya,
        }
    }
}

impl From<policy::ServiceCategory> for ServiceCategory {
    /// The way back, for rendering a reminder the domain produced.
    fn from(category: policy::ServiceCategory) -> Self {
        match category {
            policy::ServiceCategory::OliMesin => Self::OliMesin,
            policy::ServiceCategory::OliTransmisi => Self::OliTransmisi,
            policy::ServiceCategory::Rem => Self::Rem,
            policy::ServiceCategory::KakiKaki => Self::KakiKaki,
            policy::ServiceCategory::TuneUp => Self::TuneUp,
            policy::ServiceCategory::Ac => Self::Ac,
            policy::ServiceCategory::Kelistrikan => Self::Kelistrikan,
            policy::ServiceCategory::Body => Self::Body,
            policy::ServiceCategory::Lainnya => Self::Lainnya,
        }
    }
}

/// One entry in a car's history.
#[derive(Debug, Clone)]
pub struct ServiceRecord {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub service_date: Date,
    pub mileage_km: Option<i32>,
    pub category: ServiceCategory,
    pub parts_replaced: Vec<String>,
    pub garage_name: Option<String>,
    pub cost: Option<BigDecimal>,
    pub notes: Option<String>,
}

/// What a caller may set.
#[derive(Debug, Clone)]
pub struct ServiceInput {
    pub service_date: Date,
    pub mileage_km: Option<i32>,
    pub category: ServiceCategory,
    pub parts_replaced: Vec<String>,
    pub garage_name: Option<String>,
    pub cost: Option<BigDecimal>,
    pub notes: Option<String>,
}

/// One page of a car's history, newest first.
///
/// Asks for `limit + 1` rows. The extra row is never returned — it is how the
/// caller learns whether another page exists without a second `COUNT(*)`,
/// and it is why a full final page still ends the loop correctly.
///
/// The `WHERE` walks the same `(service_date DESC, id DESC)` order as the
/// index, so paging deep into a long history stays an index scan rather than
/// a sort of everything before it.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn page(
    conn: &mut PgConnection,
    owner_id: Uuid,
    vehicle_id: Uuid,
    after: Option<&DateCursor>,
    limit: u16,
) -> Result<Vec<ServiceRecord>, sqlx::Error> {
    let fetch = i64::from(limit) + 1;
    let (after_date, after_id) = match after {
        Some(cursor) => (Some(cursor.date), Some(cursor.id)),
        None => (None, None),
    };

    sqlx::query_as!(
        ServiceRecord,
        r#"
        SELECT
            s.id,
            s.vehicle_id,
            s.service_date,
            s.mileage_km,
            s.category AS "category: ServiceCategory",
            s.parts_replaced,
            s.garage_name,
            s.cost,
            s.notes
        FROM service_records s
        JOIN vehicles v ON v.id = s.vehicle_id
        WHERE v.owner_id = $1
          AND s.vehicle_id = $2
          AND (
                $3::date IS NULL
                OR (s.service_date, s.id) < ($3::date, $4::uuid)
              )
        ORDER BY s.service_date DESC, s.id DESC
        LIMIT $5
        "#,
        owner_id,
        vehicle_id,
        after_date,
        after_id,
        fetch,
    )
    .fetch_all(conn)
    .await
}

/// One record, if this person owns the car it belongs to.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn find_owned(
    conn: &mut PgConnection,
    owner_id: Uuid,
    id: Uuid,
) -> Result<Option<ServiceRecord>, sqlx::Error> {
    sqlx::query_as!(
        ServiceRecord,
        r#"
        SELECT
            s.id,
            s.vehicle_id,
            s.service_date,
            s.mileage_km,
            s.category AS "category: ServiceCategory",
            s.parts_replaced,
            s.garage_name,
            s.cost,
            s.notes
        FROM service_records s
        JOIN vehicles v ON v.id = s.vehicle_id
        WHERE v.owner_id = $1 AND s.id = $2
        "#,
        owner_id,
        id
    )
    .fetch_optional(conn)
    .await
}

/// Record a service. Returns false when the car is not this person's.
///
/// The ownership check is inside the statement rather than a separate read:
/// a `SELECT` followed by an `INSERT` leaves a window in which the car could
/// change hands, and this way there is no window.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn insert(
    conn: &mut PgConnection,
    id: Uuid,
    owner_id: Uuid,
    vehicle_id: Uuid,
    input: &ServiceInput,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        INSERT INTO service_records
            (id, vehicle_id, service_date, mileage_km, category,
             parts_replaced, garage_name, cost, notes)
        SELECT $1, v.id, $4, $5, $6, $7, $8, $9, $10
        FROM vehicles v
        WHERE v.id = $3 AND v.owner_id = $2
        "#,
        id,
        owner_id,
        vehicle_id,
        input.service_date,
        input.mileage_km,
        input.category as ServiceCategory,
        &input.parts_replaced,
        input.garage_name,
        input.cost,
        input.notes,
    )
    .execute(conn)
    .await?;

    Ok(done.rows_affected() > 0)
}

/// Replace a record. Returns false when it is not this person's.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn update(
    conn: &mut PgConnection,
    owner_id: Uuid,
    id: Uuid,
    input: &ServiceInput,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        UPDATE service_records s SET
            service_date = $3, mileage_km = $4, category = $5,
            parts_replaced = $6, garage_name = $7, cost = $8, notes = $9
        FROM vehicles v
        WHERE v.id = s.vehicle_id AND v.owner_id = $1 AND s.id = $2
        "#,
        owner_id,
        id,
        input.service_date,
        input.mileage_km,
        input.category as ServiceCategory,
        &input.parts_replaced,
        input.garage_name,
        input.cost,
        input.notes,
    )
    .execute(conn)
    .await?;

    Ok(done.rows_affected() > 0)
}

/// Remove a record. Returns false when it is not this person's.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn delete(
    conn: &mut PgConnection,
    owner_id: Uuid,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        DELETE FROM service_records s
        USING vehicles v
        WHERE v.id = s.vehicle_id AND v.owner_id = $1 AND s.id = $2
        "#,
        owner_id,
        id
    )
    .execute(conn)
    .await?;

    Ok(done.rows_affected() > 0)
}
