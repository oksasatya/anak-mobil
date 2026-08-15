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

/// Create an account.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails, including a unique violation
/// when the email is already registered — which the caller must translate
/// without revealing that the address exists.
pub async fn insert(
    conn: &mut PgConnection,
    id: Uuid,
    email: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash) VALUES ($1, $2::citext, $3)"#,
        id,
        email,
        password_hash
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
