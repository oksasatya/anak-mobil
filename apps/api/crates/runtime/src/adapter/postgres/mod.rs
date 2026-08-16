//! The Postgres connection pool, the migration runner, and the repositories.
//!
//! Queries go through the `sqlx` macros, so they are checked against the real
//! schema at compile time. That needs either a reachable `DATABASE_URL` or the
//! committed `.sqlx` cache — CI uses the cache, which is why
//! `cargo sqlx prepare` has to run whenever a query changes.
//!
//! Repositories arrive with the stories that need them, each bringing its own
//! migration.

pub mod catalog_repo;
pub mod migrate;
pub mod part_repo;
pub mod service_repo;
pub mod summary_repo;
pub mod user_repo;
pub mod vehicle_repo;

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Fail a connection attempt rather than hanging on an unreachable host.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Build the pool.
///
/// `connect_lazy` opens no socket, so constructing the pool cannot fail on a
/// database that is briefly unreachable — the first real use connects.
///
/// For the **worker** role that means the process starts and reports its
/// state through the probe rather than through a restart loop. For the
/// **web** role it does not, and deliberately so: migrations run before the
/// listener binds, so an unreachable database stops startup. A service that
/// cannot reach its database cannot serve anything either, and refusing to
/// start is more honest than listening on a port and answering 503 forever.
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
