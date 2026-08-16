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
/// The conflict clause here is a no-op for the columns that matter — it
/// writes `vehicle_id` back to itself and leaves `notes` and `visibility`
/// alone — kept only so `RETURNING id` still fires
/// when the build already exists. `ON CONFLICT DO NOTHING` looks like the
/// obvious no-op, but a `DO NOTHING` conflict returns no row at all, and the
/// caller needs the existing build's id either way.
///
/// It is not a no-op for `updated_at`: `DO UPDATE` is a real update, so the
/// `BEFORE UPDATE` trigger fires and the build's timestamp moves even when
/// its own content did not change. Adding a modification therefore counts as
/// touching the build. That may be exactly what a "recently updated" feed
/// wants — it is stated here so nobody has to discover it from a surprising
/// timestamp.
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
/// # The caller owns two checks this function does not make
///
/// This is the only function here that never reaches `vehicles.owner_id`: it
/// takes whatever build ids it is given. That is deliberate — the caller has
/// already resolved and authorised them — but it means two obligations live
/// outside this file and are easy to forget at the call site.
///
/// **Authorisation.** Pass only build ids the caller may see.
///
/// **Cost.** Every row carries `cost`, unfiltered. `vehicles.cost_visibility`
/// exists precisely to gate that, and filtering it server-side is one of the
/// platform's non-negotiables. A community list that maps these rows straight
/// into a response leaks every owner's spend — and the row shape carries
/// `cost_visibility` alongside, so the data to decide with is already there.
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

/// One row of [`page_visible`] — a build plus the vehicle fields its caller
/// needs without a second round trip: who owns it, what variant it is (for
/// the evidence-count filter), and both visibility settings.
#[derive(Debug, Clone)]
pub struct BuildRow {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub notes: Option<String>,
    pub visibility: Visibility,
    pub owner_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub nickname: Option<String>,
    pub described_as: Option<String>,
    /// Who may see this car's cost. Carried here so the response mapping can
    /// null out a modification's `cost` without a second query — the filter
    /// itself lives at the boundary that renders `Modification` into a
    /// response, not here.
    pub cost_visibility: Visibility,
}

/// One row of [`photos_for`].
#[derive(Debug, Clone)]
pub struct BuildPhoto {
    pub id: Uuid,
    pub build_id: Uuid,
    pub object_key: String,
    pub position: i32,
}

/// One row of [`evidence_counts`] — both counts, for one part.
#[derive(Debug, Clone)]
pub struct EvidenceCount {
    pub part_id: Uuid,
    pub active_build_count: i64,
    pub ever_installed_build_count: i64,
}

/// One page of builds this viewer may see, newest first.
///
/// A plain id cursor, unlike the service timeline's compound one. A build id
/// is a UUID v7 and therefore time-sortable, and unlike a service date it is
/// not user-supplied — so there is no case where a row is inserted into the
/// middle of the order, which is the failure the compound cursor exists for.
///
/// Complexity: NOT `O(log n + limit)`. Measured at 50k builds, 98% private — order of magnitude, not a benchmark: an independent run on its own fixture got 1001 rows and 3024 buffers where this says 601 and 2419, and without the fixture recipe there is no way to say which is right. The SHAPE is what was verified and what matters:
/// the planner declines `builds_visible_idx` for this predicate and walks
/// `builds_pkey` backwards — 601 rows and 2419 buffers to return 21. Deep into
/// a cursor with a viewer who owns nothing, it degrades to a `Seq Scan` over
/// `vehicles` plus a blocking sort.
///
/// The cause is `OR v.owner_id = $1`: half the predicate lives on another
/// table, and a partial index holding only non-private builds is not a
/// superset of what the query needs — the viewer's own private builds are
/// missing from it. No index on `builds` alone can fix that.
///
/// The fix, when the feed is slow enough to matter, is a `UNION ALL` of
/// "visible" and "mine". Do not add another index here, and do not restore a
/// complexity claim the planner does not deliver.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn page_visible(
    conn: &mut PgConnection,
    viewer_id: Uuid,
    after: Option<Uuid>,
    limit: u16,
) -> Result<Vec<BuildRow>, sqlx::Error> {
    sqlx::query_as!(
        BuildRow,
        r#"
        SELECT
            b.id,
            b.vehicle_id,
            b.notes,
            b.visibility AS "visibility: Visibility",
            v.owner_id,
            v.variant_id,
            v.nickname,
            v.described_as,
            v.cost_visibility AS "cost_visibility: Visibility"
        FROM builds b
        JOIN vehicles v ON v.id = b.vehicle_id
        WHERE (b.visibility <> 'private' OR v.owner_id = $1)
          AND ($2::uuid IS NULL OR b.id < $2)
        ORDER BY b.id DESC
        LIMIT $3
        "#,
        viewer_id,
        after,
        // One row more than asked for. Its presence is how the caller learns
        // another page exists without a second COUNT(*) — and it MUST be
        // dropped before rendering. It is a real, visible build, so a handler
        // that maps these rows straight into a response returns 21 items for a
        // page of 20 and repeats its last build at the top of the next page.
        i64::from(limit) + 1,
    )
    .fetch_all(conn)
    .await
}

/// Every photo on these builds, in one query.
///
/// A third query rather than a third join. Joining builds × modifications ×
/// photos multiplies rows — one build with 10 modifications and 20 photos
/// returns 200 — and the multiplier grows per build. Fetching photos per build
/// instead is the N+1 AC5 forbids. Three batched queries are a constant number
/// of round trips and `O(B + M + P)` in memory.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
/// # The caller owns the authorisation check, exactly as `modifications_for` does
///
/// This takes whatever build ids it is given and never reaches
/// `vehicles.owner_id`. Verified: passing a private build's id returns that
/// build's `object_key`.
///
/// The neighbouring function carries this warning and this one did not, which
/// is worse than neither having it — a caller who reads one and not the other
/// will reasonably assume the difference means something.
///
/// Pass only ids the caller may see.
pub async fn photos_for(
    conn: &mut PgConnection,
    build_ids: &[Uuid],
) -> Result<Vec<BuildPhoto>, sqlx::Error> {
    sqlx::query_as!(
        BuildPhoto,
        r#"
        SELECT id, build_id, object_key, position
        FROM build_photos
        WHERE build_id = ANY($1)
        ORDER BY build_id, position, id
        "#,
        build_ids,
    )
    .fetch_all(conn)
    .await
}

/// Both community-evidence counts, for every part on a page, in one query.
///
/// **Two counts, each named for what it counts.** `active_build_count` is
/// builds where the part is currently fitted; `ever_installed_build_count`
/// includes those where it was removed, because a removed modification is
/// still evidence that the part fitted. Calling either of them just "evidence
/// count" is how the wrong one ends up on a screen.
///
/// Grouped by the canonical part, so a build that referenced two now-merged
/// parts counts once rather than twice — and the grouping key is what makes
/// AC4's "the counts combine rather than disappear" true without rewriting
/// `modifications.part_id`.
///
/// Filtered on the reader's visibility and, when given, on the vehicle's
/// variant. An Avanza build whose vehicle was never matched to a catalog
/// variant is not evidence about a Civic; `variant_id` is nullable precisely
/// because free-text cars exist, and a NULL variant matches no filter rather
/// than every filter.
///
/// A part with no builds gets a row of two zeros rather than no row. A client
/// that has to special-case an absent count will get it wrong on the empty
/// catalog, which is the state this platform launches in.
///
/// Complexity: `O(k + m)` for `k` part ids and `m` matching modifications, one
/// query for the whole page. The version of this that takes a single part id
/// is the N+1 — do not add it.
///
/// **An eighth thing the plan got wrong, caught while writing this rather
/// than measured after:** the drafted query counted `DISTINCT b.id`, but
/// `b` (`builds`) is joined unconditionally on `m.build_id` — the visibility
/// and variant predicate lives entirely on the `vehicles` join below it.
/// `b.id` is therefore non-NULL whether or not the build is visible, so
/// `COUNT(DISTINCT b.id)` silently ignores the filter and counts every
/// matching modification's build regardless of who is asking. Counting
/// `DISTINCT v.id` instead is the fix: `v` only joins when the visibility
/// *and* variant predicate holds (its ON clause carries both, exactly as
/// intended), so a filtered-out row leaves `v.id` NULL and `COUNT` — which
/// ignores NULLs — drops it. `builds.vehicle_id` is `UNIQUE`, so a build and
/// its vehicle are a bijection and `COUNT(DISTINCT v.id)` counts the same
/// set of builds `COUNT(DISTINCT b.id)` was meant to, correctly gated this
/// time. This is exactly the class of bug the ON-vs-WHERE note above warns
/// about: it does not show up as a compile error, and it does not show up
/// in a naive integration test either unless that test asserts a *private*
/// build is excluded from another viewer's count — Task 3.3's job.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
/// # Key the result by `part_id`; never zip it against the input by position
///
/// The result is not index-aligned with `part_ids`, in two ways. A part id
/// that does not exist in `parts` returns **no row at all** — the inner join
/// to `parts` drops it — so "a row of zeros rather than an absent row" holds
/// only for parts that exist. And duplicates collapse: `[A, A, B]` returns two
/// rows, not three.
///
/// A caller zipping by position therefore attaches the wrong count to the
/// wrong part, silently and plausibly.
pub async fn evidence_counts(
    conn: &mut PgConnection,
    part_ids: &[Uuid],
    viewer_id: Uuid,
    variant_id: Option<Uuid>,
) -> Result<Vec<EvidenceCount>, sqlx::Error> {
    sqlx::query_as!(
        EvidenceCount,
        r#"
        WITH asked AS (
            SELECT unnest($1::uuid[]) AS part_id
        ),
        canonical AS (
            -- The canonical form of each part asked about, and of every part
            -- merged into it. One hop: canonical_part_id is a cache the merge
            -- use case keeps flat, never a chain.
            SELECT a.part_id AS asked_id,
                   p.id      AS member_id
            FROM asked a
            JOIN parts root ON root.id = a.part_id
            JOIN parts p
              ON p.id = COALESCE(root.canonical_part_id, root.id)
              OR p.canonical_part_id = COALESCE(root.canonical_part_id, root.id)
        )
        SELECT
            c.asked_id AS "part_id!",
            COUNT(DISTINCT v.id) FILTER (WHERE m.removed_at IS NULL) AS "active_build_count!",
            COUNT(DISTINCT v.id)                                     AS "ever_installed_build_count!"
        FROM canonical c
        LEFT JOIN modifications m ON m.part_id = c.member_id
        LEFT JOIN builds b        ON b.id = m.build_id
        LEFT JOIN vehicles v      ON v.id = b.vehicle_id
                                 AND (b.visibility <> 'private' OR v.owner_id = $2)
                                 AND ($3::uuid IS NULL OR v.variant_id = $3)
        GROUP BY c.asked_id
        "#,
        part_ids,
        viewer_id,
        variant_id,
    )
    .fetch_all(conn)
    .await
}
