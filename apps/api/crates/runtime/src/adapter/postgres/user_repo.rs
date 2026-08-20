//! Reading and writing accounts.
//!
//! Every method takes a connection rather than a pool, so the **use case**
//! owns the transaction boundary. A repository holding its own pool turns
//! each call into a separate transaction, and two concurrent requests then
//! interleave into state neither of them expected.

use sqlx::PgConnection;
use uuid::Uuid;

/// An account as authentication needs it.
///
/// Deliberately not `Serialize`. The password hash is in this struct, and a
/// derive would put it one careless `Json(user)` away from a response body.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub id: Uuid,
    pub password_hash: String,
}

/// Find an account by email.
///
/// Email is `CITEXT`, so the comparison is case-insensitive in the database
/// rather than in whichever caller remembered to lowercase.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn find_credentials(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<Credentials>, sqlx::Error> {
    sqlx::query_as!(
        Credentials,
        r#"SELECT id, password_hash FROM users WHERE email = $1::citext"#,
        email
    )
    .fetch_optional(conn)
    .await
}

/// An account on its way to being created.
///
/// A struct rather than four positional arguments: three adjacent `&str` are
/// exactly the shape that gets swapped at a call site and still compiles.
/// `RoleChangeRow` below is the same fix for the same reason.
pub struct NewUser<'a> {
    pub id: Uuid,
    pub email: &'a str,
    pub username: &'a str,
    pub password_hash: &'a str,
}

/// Create an account.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails, including a unique violation
/// on either `users_email_key` or `users_username_key` — which the caller must
/// translate by constraint name, because the two send somebody to different
/// fields.
pub async fn insert(conn: &mut PgConnection, user: NewUser<'_>) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO users (id, email, username, password_hash)
        VALUES ($1, $2::citext, $3::citext, $4)
        "#,
        user.id,
        user.email,
        user.username,
        user.password_hash
    )
    .execute(conn)
    .await
    .map(drop)
}

/// Does an account still exist?
///
/// Used where a session outliving its account would otherwise authenticate.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn exists(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)"#, id)
        .fetch_one(conn)
        .await
        .map(|found| found.unwrap_or(false))
}

/// What an account may do across the whole platform.
///
/// Two values, and `CONTEXT.md` records why the name is this long: `Role` in
/// this crate is the PROCESS role (`web` | `worker` | `migrate`), and
/// community membership will bring a third sense of the word that grants
/// nothing platform-wide. Three unrelated concepts, three names.
///
/// There is deliberately no copy of this in the domain crate. `ServiceCategory`
/// exists twice because a domain policy function consumes it; nothing in
/// `domain` consumes a role, and two variants with no policy function is not a
/// domain model but a second place to keep in sync. When a policy function
/// earns it, the split is additive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "platform_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PlatformRole {
    User,
    Admin,
}

/// This account's platform role, read from the source.
///
/// `None` means there is no such account — which happens when a Redis session
/// outlives the row it points at. Every caller treats that as a refusal; see
/// [`crate::adapter::http::auth::Admin`].
///
/// Nothing caches this. That is what satisfies AC1: a revoked role is refused
/// on the next admin request without waiting for a new token, because the
/// token never carried the role in the first place.
///
/// Complexity: `O(log n)` — a primary-key lookup, one row.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails. A failure is never turned
/// into a default role by any caller.
pub async fn platform_role_of(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<PlatformRole>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT platform_role AS "platform_role: PlatformRole" FROM users WHERE id = $1"#,
        id
    )
    .fetch_optional(conn)
    .await
}

/// Serialise every platform-role change against every other, for this
/// transaction.
///
/// Released on commit or rollback — there is no unlock to forget.
///
/// **Two arguments, deliberately.** `pg_advisory_xact_lock(key, 0)` occupies a
/// different keyspace from the single-argument locks already in use —
/// `hashtext('part_merge')` in `part_repo::lock_merges` — so the two cannot
/// collide even on the same hash. The other two-argument lock in this codebase
/// keys on a person (`usecase::parts::allowance_spent`); this one keys on the
/// platform, because a role change is rare and serialising all of them costs
/// nothing.
///
/// **Only useful inside a transaction.** On a pooled autocommit connection
/// every statement is its own transaction, so the lock would be taken and
/// released before the next read — a guard that looks right and does nothing.
/// That bug was shipped and fixed once already; see `usecase::parts::suggest`.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn lock_platform_role(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query!(r#"SELECT pg_advisory_xact_lock(hashtext('platform_role'), 0)"#)
        .execute(conn)
        .await
        .map(drop)
}

/// How many accounts are admins right now.
///
/// The bootstrap precondition, and it is read **inside** the same lock and the
/// same transaction as the write. Counting and then inserting across two
/// statements is check-then-act: two operators running `grant-admin`
/// concurrently both see zero and both succeed. That is the defect the AM-361
/// fix pass closed twice and it does not get to appear a third time.
///
/// Complexity: `O(U)` — a sequential scan over `users`, no index.
/// ponytail: correct today. `grant-admin` is a hand-typed operational command
/// run perhaps twice in the platform's life, and an index maintained on every
/// account write to serve it would cost more than it saves. If a count of
/// admins ever appears on a request path, add
/// `CREATE INDEX users_admins_idx ON users (platform_role) WHERE platform_role = 'admin'`.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn admin_count(conn: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT count(*) FROM users WHERE platform_role = 'admin'"#)
        .fetch_one(conn)
        .await
        .map(|count| count.unwrap_or(0))
}

/// One row of the audit trail, on its way to being written.
///
/// A struct rather than seven positional arguments: `clippy::too_many_arguments`
/// is a denied warning at eight, and two adjacent `PlatformRole` values and two
/// adjacent `Uuid`s are exactly the shape that gets swapped at a call site.
/// `shared::validation::DecimalSpec` is the same fix for the same reason.
pub struct RoleChangeRow<'a> {
    pub id: Uuid,
    /// `None` for a bootstrap — an operational command has no signed-in human
    /// behind it.
    pub actor_id: Option<Uuid>,
    pub target_user_id: Uuid,
    pub from_role: PlatformRole,
    pub to_role: PlatformRole,
    pub reason: &'a str,
}

/// Record a role change. Returns when it was recorded.
///
/// Called **before** the column is updated, and its failure fails the whole
/// change: a privilege that exists with no record of how it was granted is
/// worse than a privilege that failed to be granted.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails, including a `CHECK` violation
/// on `role_changes_real_change` — which the use case's no-op branch means no
/// client should ever be able to reach.
pub async fn insert_role_change(
    conn: &mut PgConnection,
    row: RoleChangeRow<'_>,
) -> Result<time::OffsetDateTime, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        INSERT INTO role_changes
            (id, actor_id, target_user_id, from_role, to_role, reason)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING created_at
        "#,
        row.id,
        row.actor_id,
        row.target_user_id,
        row.from_role as PlatformRole,
        row.to_role as PlatformRole,
        row.reason,
    )
    .fetch_one(conn)
    .await
}

/// Write the new platform role.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn set_platform_role(
    conn: &mut PgConnection,
    id: Uuid,
    role: PlatformRole,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE users SET platform_role = $2 WHERE id = $1"#,
        id,
        role as PlatformRole,
    )
    .execute(conn)
    .await
    .map(drop)
}

/// Find an account by email, for the operational command that takes one.
///
/// Email is `CITEXT`, so the comparison is case-insensitive in the database
/// rather than in whichever caller remembered to lowercase.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn find_id_by_email(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT id FROM users WHERE email = $1::citext"#, email)
        .fetch_optional(conn)
        .await
}

/// An account as the app's bootstrap needs it.
///
/// Deliberately a different struct from [`Credentials`]: that one carries the
/// password hash and is deliberately not `Serialize`. Keeping the two apart is
/// what makes it impossible to hand a hash to a response by widening a query.
#[derive(Debug, Clone)]
pub struct Profile {
    pub id: Uuid,
    pub email: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    /// Derived, never stored. A person who has a car has finished onboarding,
    /// and a stored completion flag is a second source of truth that can
    /// disagree with the first.
    pub has_vehicles: bool,
}

/// This account's profile and whether it has any car.
///
/// One query rather than two. The `::text` casts are required: `email` and
/// `username` are `CITEXT`, and sqlx has no mapping for it — without the cast
/// the macro fails on an unknown type rather than at runtime.
///
/// Complexity: `O(log n)` — a primary-key lookup plus an index probe into
/// `vehicles_owner_position_idx`, which leads on `owner_id`.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn profile_of(conn: &mut PgConnection, id: Uuid) -> Result<Option<Profile>, sqlx::Error> {
    sqlx::query_as!(
        Profile,
        r#"
        SELECT
            u.id,
            u.email::text        AS "email!",
            u.username::text     AS "username?",
            u.display_name       AS "display_name?",
            EXISTS(SELECT 1 FROM vehicles v WHERE v.owner_id = u.id) AS "has_vehicles!"
        FROM users u
        WHERE u.id = $1
        "#,
        id
    )
    .fetch_optional(conn)
    .await
}

/// Is this canonical username already held?
///
/// `CITEXT`, so the comparison is case-insensitive in the database rather than
/// in whichever caller remembered to lowercase — the same reason `email` is.
///
/// Complexity: `O(log n)` — an index probe into `users_username_key`.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn username_exists(conn: &mut PgConnection, username: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM users WHERE username = $1::citext)"#,
        username
    )
    .fetch_one(conn)
    .await
    .map(|found| found.unwrap_or(false))
}

/// Write the display name. Already trimmed and length-checked by the caller.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn set_display_name(
    conn: &mut PgConnection,
    id: Uuid,
    display_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE users SET display_name = $2 WHERE id = $1"#,
        id,
        display_name
    )
    .execute(conn)
    .await
    .map(drop)
}
