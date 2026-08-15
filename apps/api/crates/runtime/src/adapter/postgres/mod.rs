//! The Postgres connection pool.
//!
//! Connectivity only. Tables, migrations, and queries arrive with AM-353;
//! nothing here writes SQL, which is why the `sqlx` `macros` feature is not
//! enabled yet — `query!` needs either a live schema or a committed `.sqlx`
//! cache, and neither exists.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Fail a connection attempt rather than hanging on an unreachable host.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Build the pool.
///
/// Does not verify the credentials — `connect_lazy` opens no socket, so a
/// database that is down at boot does not stop the process from starting.
/// That is deliberate: a service that refuses to start when Postgres is
/// briefly unavailable cannot report *why* it is unhealthy, and a restart
/// loop is harder to diagnose than a running process answering `/readyz`
/// with a reason.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the URL cannot be parsed.
pub fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .min_connections(1)
        .max_connections(10)
        .acquire_timeout(CONNECT_TIMEOUT)
        .connect_lazy(database_url)
}

/// Is Postgres reachable right now?
///
/// `acquire` rather than a query: sqlx defaults `test_before_acquire` to
/// true, so an idle connection is validated before it is handed over and a
/// new one requires a real round trip. It proves connectivity without needing
/// a schema to query, which is what lets readiness work before AM-353.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when no connection can be obtained.
pub async fn check(pool: &PgPool) -> Result<(), sqlx::Error> {
    pool.acquire().await.map(drop)
}
