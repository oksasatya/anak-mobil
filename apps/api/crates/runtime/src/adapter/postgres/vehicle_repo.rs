//! Reading and writing a person's cars.
//!
//! # Private data is a separate call, not a separate column
//!
//! Nothing in this module returns a plate, a VIN, or a purchase price unless
//! the caller asked for it by name. [`find_private`] is the only way to reach
//! them, and the types the other functions return have nowhere to put them.
//!
//! That is deliberate and it is the whole reason the schema is split. A
//! filtered `SELECT` protects the columns until the first query that forgets;
//! a separate type protects them until somebody deliberately changes a
//! function signature, which is a thing a reviewer sees.
//!
//! # Every query is scoped by owner
//!
//! There is no `find_by_id(id)`. Ownership is a parameter, not an assumption,
//! so "I forgot the WHERE clause" is a compile error rather than a leak.

use sqlx::PgConnection;
use sqlx::types::BigDecimal;
use time::Date;
use uuid::Uuid;

/// A car as a screen shows it. No private fields exist on this type.
#[derive(Debug, Clone)]
pub struct Vehicle {
    pub id: Uuid,
    pub variant_id: Option<Uuid>,
    pub nickname: Option<String>,
    pub described_as: Option<String>,
    pub year: Option<i16>,
    pub colour: Option<String>,
    pub mileage_km: Option<i32>,
    pub position: i32,
    /// Assembled from the catalog when the car is matched to a variant.
    pub catalog_name: Option<String>,
}

/// The three fields that never leave the server for anyone but the owner.
#[derive(Debug, Clone)]
pub struct VehiclePrivate {
    pub plate: Option<String>,
    pub vin: Option<String>,
    pub purchase_price: Option<BigDecimal>,
    pub purchase_date: Option<Date>,
}

/// What a caller may set on a car.
#[derive(Debug, Clone, Default)]
pub struct VehicleInput {
    pub variant_id: Option<Uuid>,
    pub nickname: Option<String>,
    pub described_as: Option<String>,
    pub year: Option<i16>,
    pub colour: Option<String>,
    pub mileage_km: Option<i32>,
}

/// Every car this person owns, in the order they arranged.
///
/// One query. The catalog name is joined rather than fetched per row — the
/// list is exactly where an N+1 would hide, because it looks like a loop over
/// results rather than a loop over queries.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn list_owned(
    conn: &mut PgConnection,
    owner_id: Uuid,
) -> Result<Vec<Vehicle>, sqlx::Error> {
    sqlx::query_as!(
        Vehicle,
        r#"
        SELECT
            v.id,
            v.variant_id,
            v.nickname,
            v.described_as,
            v.year,
            v.colour,
            v.mileage_km,
            v.position,
            CASE WHEN va.id IS NULL THEN NULL
                 ELSE b.name || ' ' || m.name || ' ' || va.name
            END AS catalog_name
        FROM vehicles v
        LEFT JOIN vehicle_variants va    ON va.id = v.variant_id
        LEFT JOIN vehicle_generations g  ON g.id = va.generation_id
        LEFT JOIN vehicle_models m       ON m.id = g.model_id
        LEFT JOIN brands b               ON b.id = m.brand_id
        WHERE v.owner_id = $1
        ORDER BY v.position, v.id
        "#,
        owner_id
    )
    .fetch_all(conn)
    .await
}

/// One car, if this person owns it.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn find_owned(
    conn: &mut PgConnection,
    owner_id: Uuid,
    id: Uuid,
) -> Result<Option<Vehicle>, sqlx::Error> {
    sqlx::query_as!(
        Vehicle,
        r#"
        SELECT
            v.id,
            v.variant_id,
            v.nickname,
            v.described_as,
            v.year,
            v.colour,
            v.mileage_km,
            v.position,
            CASE WHEN va.id IS NULL THEN NULL
                 ELSE b.name || ' ' || m.name || ' ' || va.name
            END AS catalog_name
        FROM vehicles v
        LEFT JOIN vehicle_variants va    ON va.id = v.variant_id
        LEFT JOIN vehicle_generations g  ON g.id = va.generation_id
        LEFT JOIN vehicle_models m       ON m.id = g.model_id
        LEFT JOIN brands b               ON b.id = m.brand_id
        WHERE v.owner_id = $1 AND v.id = $2
        "#,
        owner_id,
        id
    )
    .fetch_optional(conn)
    .await
}

/// The private fields, for the owner only.
///
/// Takes `owner_id` and joins on it, so this cannot be called with an id
/// alone. Returning `None` covers both "no private data recorded" and "not
/// this person's car", which is the right answer to both.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn find_private(
    conn: &mut PgConnection,
    owner_id: Uuid,
    vehicle_id: Uuid,
) -> Result<Option<VehiclePrivate>, sqlx::Error> {
    sqlx::query_as!(
        VehiclePrivate,
        r#"
        SELECT p.plate, p.vin, p.purchase_price, p.purchase_date
        FROM vehicle_private p
        JOIN vehicles v ON v.id = p.vehicle_id
        WHERE v.owner_id = $1 AND p.vehicle_id = $2
        "#,
        owner_id,
        vehicle_id
    )
    .fetch_optional(conn)
    .await
}

/// Add a car at the end of the garage.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn insert(
    conn: &mut PgConnection,
    id: Uuid,
    owner_id: Uuid,
    input: &VehicleInput,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO vehicles
            (id, owner_id, variant_id, nickname, described_as, year, colour, mileage_km, position)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8,
             COALESCE((SELECT MAX(position) + 1 FROM vehicles WHERE owner_id = $2), 0))
        "#,
        id,
        owner_id,
        input.variant_id,
        input.nickname,
        input.described_as,
        input.year,
        input.colour,
        input.mileage_km,
    )
    .execute(conn)
    .await
    .map(drop)
}

/// Replace a car's public fields. Returns false when it is not this person's.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn update(
    conn: &mut PgConnection,
    owner_id: Uuid,
    id: Uuid,
    input: &VehicleInput,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        UPDATE vehicles SET
            variant_id = $3, nickname = $4, described_as = $5,
            year = $6, colour = $7, mileage_km = $8
        WHERE owner_id = $1 AND id = $2
        "#,
        owner_id,
        id,
        input.variant_id,
        input.nickname,
        input.described_as,
        input.year,
        input.colour,
        input.mileage_km,
    )
    .execute(conn)
    .await?;

    Ok(done.rows_affected() > 0)
}

/// Set or replace the private fields.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn upsert_private(
    conn: &mut PgConnection,
    owner_id: Uuid,
    vehicle_id: Uuid,
    private: &VehiclePrivate,
) -> Result<bool, sqlx::Error> {
    // The `SELECT ... WHERE owner_id` in the source is what keeps this from
    // writing private data onto somebody else's car.
    let done = sqlx::query!(
        r#"
        INSERT INTO vehicle_private (vehicle_id, plate, vin, purchase_price, purchase_date)
        SELECT v.id, $3, $4, $5, $6
        FROM vehicles v
        WHERE v.id = $2 AND v.owner_id = $1
        ON CONFLICT (vehicle_id) DO UPDATE SET
            plate = EXCLUDED.plate,
            vin = EXCLUDED.vin,
            purchase_price = EXCLUDED.purchase_price,
            purchase_date = EXCLUDED.purchase_date
        "#,
        owner_id,
        vehicle_id,
        private.plate,
        private.vin,
        private.purchase_price,
        private.purchase_date,
    )
    .execute(conn)
    .await?;

    Ok(done.rows_affected() > 0)
}

/// Remove a car. Its private data and history go with it, by cascade.
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
        r#"DELETE FROM vehicles WHERE owner_id = $1 AND id = $2"#,
        owner_id,
        id
    )
    .execute(conn)
    .await?;

    Ok(done.rows_affected() > 0)
}

/// Rewrite the garage order.
///
/// One statement over the whole list rather than one per car — the loop that
/// looks harmless at five vehicles is the shape that becomes a problem
/// everywhere else, so it does not get written here either.
///
/// Returns how many rows moved, which the caller compares against what it
/// sent to detect an id that was not theirs.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn reorder(
    conn: &mut PgConnection,
    owner_id: Uuid,
    ordered: &[Uuid],
) -> Result<u64, sqlx::Error> {
    let positions: Vec<i32> = (0..ordered.len())
        .map(|index| i32::try_from(index).unwrap_or(i32::MAX))
        .collect();

    let done = sqlx::query!(
        r#"
        UPDATE vehicles AS v
        SET position = ordering.position
        FROM UNNEST($2::uuid[], $3::int[]) AS ordering(id, position)
        WHERE v.id = ordering.id AND v.owner_id = $1
        "#,
        owner_id,
        ordered,
        &positions,
    )
    .execute(conn)
    .await?;

    Ok(done.rows_affected())
}
