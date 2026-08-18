//! Applying migrations.
//!
//! `sqlx::migrate!` embeds the SQL files into the binary at compile time, so
//! a deployed image carries its own migrations and cannot be run against a
//! directory that has drifted from the code.
//!
//! The path resolves relative to `CARGO_MANIFEST_DIR`, which is why
//! `migrations/` lives in this crate rather than at the workspace root.
//!
//! # Concurrency
//!
//! sqlx takes a Postgres advisory lock before applying anything, so several
//! replicas booting at once do not race: one applies, the rest wait and then
//! find there is nothing to do. This is not something to re-implement.
//!
//! # Why this also exists as its own process role
//!
//! Migrations run on boot (AM-352 AC2 — before the port is listened on), and
//! that is right for a service that must never serve against a schema it does
//! not expect. But it has two failure modes that a standalone path avoids.
//!
//! A migration that fails means *every* replica refuses to start, so a broken
//! migration is an outage rather than a failed deploy. And a long migration —
//! an index over a large table — holds startup open long enough for the
//! platform's health check to give up and kill the process **mid-migration**.
//!
//! `anakmobil migrate` runs them and exits. Use it in CI, and use it
//! deliberately for anything that will take a while.

use sqlx::PgPool;
use sqlx::migrate::MigrateError;

/// Apply every migration that has not been applied yet.
///
/// Idempotent: running it against an up-to-date database does nothing, which
/// is what makes it safe on every boot.
///
/// # Errors
///
/// Returns [`MigrateError`] when a migration fails, or when an
/// already-applied migration's checksum no longer matches the file — that
/// means a migration was edited after being applied, and continuing would
/// leave two databases with the same version number and different schemas.
pub async fn run(pool: &PgPool) -> Result<(), MigrateError> {
    let migrator = sqlx::migrate!("./migrations");

    // Counting `iter()` directly reports double: a reversible migration is
    // embedded as two entries sharing one version, one up and one down.
    let embedded = migrator
        .iter()
        .filter(|m| !m.migration_type.is_down_migration())
        .count();

    tracing::info!(migrations = embedded, "applying migrations");
    migrator.run(pool).await?;
    tracing::info!("migrations up to date");

    Ok(())
}

#[cfg(test)]
mod tests {
    /// Versions are unique and ascending.
    ///
    /// A duplicate timestamp is easy to produce — two `sqlx migrate add` calls
    /// in the same second — and yields a migrator that applies one file and
    /// silently ignores the other.
    ///
    /// Only the up migrations are counted. A reversible pair is embedded as
    /// two entries sharing one version, so comparing every entry would flag
    /// every correct migration as a duplicate.
    #[test]
    fn embedded_migrations_are_unique_and_ordered() {
        let migrator = sqlx::migrate!("./migrations");
        let versions: Vec<i64> = migrator
            .iter()
            .filter(|m| !m.migration_type.is_down_migration())
            .map(|m| m.version)
            .collect();

        assert!(!versions.is_empty(), "no migrations were embedded");

        let mut sorted = versions.clone();
        sorted.sort_unstable();
        sorted.dedup();

        assert_eq!(
            versions.len(),
            sorted.len(),
            "two migrations share a version: {versions:?}"
        );
        assert_eq!(versions, sorted, "migrations are not in ascending order");
    }

    /// Every migration is reversible.
    ///
    /// Not because rollbacks are routine — they are not — but because writing
    /// the down migration is what forces you to notice a change that cannot be
    /// undone, while it is still cheap to choose a different one.
    #[test]
    fn every_migration_has_a_down() {
        let migrator = sqlx::migrate!("./migrations");

        let ups: Vec<i64> = migrator
            .iter()
            .filter(|m| !m.migration_type.is_down_migration())
            .map(|m| m.version)
            .collect();
        let downs: Vec<i64> = migrator
            .iter()
            .filter(|m| m.migration_type.is_down_migration())
            .map(|m| m.version)
            .collect();

        for version in &ups {
            assert!(
                downs.contains(version),
                "migration {version} has no down migration"
            );
        }
    }
}
