//! Reading and writing builds and their modifications.
//!
//! Every write joins through `builds → vehicles` on `owner_id`, so there is no
//! path that reaches a modification without saying whose car it belongs to —
//! the same discipline as the service repository, for the same reason.
//!
//! The ownership check lives inside each statement rather than in a preceding
//! SELECT. A read-then-write leaves a window in which the car could change
//! hands, and this way there is no window.

use sqlx::PgConnection;
use sqlx::types::BigDecimal;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

/// Who may see a thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "visibility", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    Community,
    Public,
}

#[derive(Debug, Clone)]
pub struct Build {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub notes: Option<String>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone)]
pub struct BuildInput {
    pub notes: Option<String>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone)]
pub struct Modification {
    pub id: Uuid,
    pub build_id: Uuid,
    pub part_id: Uuid,
    pub install_date: Option<Date>,
    pub mileage_km: Option<i32>,
    pub cost: Option<BigDecimal>,
    pub garage_name: Option<String>,
    pub notes: Option<String>,
    /// `Some` when the part has been taken off. The row stays: it is still
    /// evidence the part fitted.
    pub removed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct ModificationInput {
    pub part_id: Uuid,
    pub install_date: Option<Date>,
    pub mileage_km: Option<i32>,
    pub cost: Option<BigDecimal>,
    pub garage_name: Option<String>,
    pub notes: Option<String>,
}

/// Create or replace this car's build. `None` when the car is not this
/// person's.
///
/// `ON CONFLICT (vehicle_id)` rather than a read-then-branch: one car has one
/// build, so "create it if it is not there" is the whole operation and two
/// statements would race with themselves.
///
/// This is the explicit write — `PUT /vehicles/{id}/build` — and it means
/// what it says: the caller's `notes` and `visibility` replace whatever was
/// there. Anything that only needs a build to *exist*, without asserting an
/// opinion about its content, calls [`ensure_build`] instead.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn upsert_build(
    conn: &mut PgConnection,
    id: Uuid,
    owner_id: Uuid,
    vehicle_id: Uuid,
    input: &BuildInput,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        INSERT INTO builds (id, vehicle_id, notes, visibility)
        SELECT $1, v.id, $4, $5
        FROM vehicles v
        WHERE v.id = $3 AND v.owner_id = $2
        ON CONFLICT (vehicle_id) DO UPDATE
            SET notes = EXCLUDED.notes, visibility = EXCLUDED.visibility
        RETURNING id
        "#,
        id,
        owner_id,
        vehicle_id,
        input.notes,
        input.visibility as Visibility,
    )
    .fetch_optional(conn)
    .await
}

/// Get or create this car's build, without touching one that already exists.
///
/// `add_modification` (Task 2.3) needs a build to attach a modification to,
/// and a person adding their first modification has not thought about
/// "creating a build" first — the row is created on demand, the moment it is
/// needed. Calling [`upsert_build`] for that would be wrong: its conflict
/// clause writes the caller's `notes` and `visibility` on *every* call, which
/// is correct for the explicit `PUT` and would silently wipe the owner's
/// notes and reset a public build back to private every time they added a
/// part.
///
/// The conflict clause here is a no-op — it writes `vehicle_id` back to
/// itself, touching nothing else — kept only so `RETURNING id` still fires
/// when the build already exists. `ON CONFLICT DO NOTHING` looks like the
/// obvious no-op, but a `DO NOTHING` conflict returns no row at all, and the
/// caller needs the existing build's id either way.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn ensure_build(
    conn: &mut PgConnection,
    id: Uuid,
    owner_id: Uuid,
    vehicle_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        INSERT INTO builds (id, vehicle_id)
        SELECT $1, v.id
        FROM vehicles v
        WHERE v.id = $3 AND v.owner_id = $2
        ON CONFLICT (vehicle_id) DO UPDATE
            SET vehicle_id = EXCLUDED.vehicle_id
        RETURNING id
        "#,
        id,
        owner_id,
        vehicle_id,
    )
    .fetch_optional(conn)
    .await
}

/// This person's build for this car.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn find_build_for_vehicle(
    conn: &mut PgConnection,
    owner_id: Uuid,
    vehicle_id: Uuid,
) -> Result<Option<Build>, sqlx::Error> {
    sqlx::query_as!(
        Build,
        r#"
        SELECT b.id, b.vehicle_id, b.notes, b.visibility AS "visibility: Visibility"
        FROM builds b
        JOIN vehicles v ON v.id = b.vehicle_id
        WHERE v.owner_id = $1 AND b.vehicle_id = $2
        "#,
        owner_id,
        vehicle_id,
    )
    .fetch_optional(conn)
    .await
}

/// A build this viewer is allowed to see.
///
/// The predicate is the whole point: a private build is visible to its owner
/// and to nobody else, and `community` is visible to any signed-in caller —
/// which is every caller, because every endpoint on this server is
/// authenticated. When an unauthenticated surface arrives, `community` and
/// `public` stop being the same answer, and this predicate is the one place
/// that has to change.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn find_build_visible(
    conn: &mut PgConnection,
    viewer_id: Uuid,
    id: Uuid,
) -> Result<Option<Build>, sqlx::Error> {
    sqlx::query_as!(
        Build,
        r#"
        SELECT b.id, b.vehicle_id, b.notes, b.visibility AS "visibility: Visibility"
        FROM builds b
        JOIN vehicles v ON v.id = b.vehicle_id
        WHERE b.id = $2
          AND (b.visibility <> 'private' OR v.owner_id = $1)
        "#,
        viewer_id,
        id,
    )
    .fetch_optional(conn)
    .await
}

/// Add a modification. `false` when the build is not this person's.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn insert_modification(
    conn: &mut PgConnection,
    id: Uuid,
    owner_id: Uuid,
    build_id: Uuid,
    input: &ModificationInput,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        INSERT INTO modifications
            (id, build_id, part_id, install_date, mileage_km, cost, garage_name, notes)
        SELECT $1, b.id, $4, $5, $6, $7, $8, $9
        FROM builds b
        JOIN vehicles v ON v.id = b.vehicle_id
        WHERE b.id = $3 AND v.owner_id = $2
        "#,
        id,
        owner_id,
        build_id,
        input.part_id,
        input.install_date,
        input.mileage_km,
        input.cost,
        input.garage_name,
        input.notes,
    )
    .execute(conn)
    .await?;

    Ok(done.rows_affected() > 0)
}

/// Replace a modification. `false` when it is not this person's.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn update_modification(
    conn: &mut PgConnection,
    owner_id: Uuid,
    id: Uuid,
    input: &ModificationInput,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        UPDATE modifications m SET
            part_id = $3, install_date = $4, mileage_km = $5,
            cost = $6, garage_name = $7, notes = $8
        FROM builds b
        JOIN vehicles v ON v.id = b.vehicle_id
        WHERE b.id = m.build_id AND v.owner_id = $1 AND m.id = $2
        "#,
        owner_id,
        id,
        input.part_id,
        input.install_date,
        input.mileage_km,
        input.cost,
        input.garage_name,
        input.notes,
    )
    .execute(conn)
    .await?;

    Ok(done.rows_affected() > 0)
}

/// Mark a part as taken off. `false` when the modification is not this
/// person's.
///
/// Idempotent: `removed_at` is only written when it is currently NULL, and the
/// `EXISTS` is what makes a second call report success rather than a 404. A
/// client retrying a request that already succeeded must not be told the thing
/// does not exist.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn mark_modification_removed(
    conn: &mut PgConnection,
    owner_id: Uuid,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        WITH owned AS (
            SELECT m.id
            FROM modifications m
            JOIN builds b   ON b.id = m.build_id
            JOIN vehicles v ON v.id = b.vehicle_id
            WHERE m.id = $2 AND v.owner_id = $1
        ), removed AS (
            UPDATE modifications
            SET removed_at = now()
            WHERE id IN (SELECT id FROM owned) AND removed_at IS NULL
            RETURNING id
        )
        SELECT EXISTS(SELECT 1 FROM owned) AS "owned!"
        "#,
        owner_id,
        id,
    )
    .fetch_one(conn)
    .await
}

// `removed` above is a CTE whose result is never selected. PostgreSQL still
// executes a data-modifying CTE whether or not the outer query reads it —
// that is documented behaviour, and it is what makes this one statement
// instead of two. It reads like dead code to anyone who does not know that
// rule, hence this note.

/// Every modification on these builds, in one query.
///
/// `= ANY($1)` rather than a query per build. This is the function AC5 turns
/// on: a loop of `await`s here is the N+1 the acceptance criterion forbids, and
/// it is invisible in a review because the loop reads as a loop over rendered
/// rows.
///
/// Removed modifications are returned too, flagged by `removed_at`. Filtering
/// them here would make the caller unable to distinguish "never fitted" from
/// "fitted and taken off", which is exactly the distinction the two evidence
/// counts are built on.
///
/// Complexity: `O(log n + m)` for `m` matching rows via
/// `modifications_build_idx`; `O(m)` memory. One round trip regardless of how
/// many builds are asked for.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn modifications_for(
    conn: &mut PgConnection,
    build_ids: &[Uuid],
) -> Result<Vec<Modification>, sqlx::Error> {
    sqlx::query_as!(
        Modification,
        r#"
        SELECT id, build_id, part_id, install_date, mileage_km,
               cost, garage_name, notes, removed_at
        FROM modifications
        WHERE build_id = ANY($1)
        ORDER BY build_id, install_date DESC NULLS LAST, id
        "#,
        build_ids,
    )
    .fetch_all(conn)
    .await
}
