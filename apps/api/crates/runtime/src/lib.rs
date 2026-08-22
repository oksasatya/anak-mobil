//! AnakMobil backend — one library, three process roles.
//!
//! ```text
//! anakmobil web       # HTTP server
//! anakmobil worker    # background job consumer
//! anakmobil migrate   # apply migrations and exit
//! ```
//!
//! All three run from the same build and share the same configuration. Web
//! and worker are separate *processes* rather than tasks in one process
//! because
//! the worker does CPU-heavy work — image compression above all — and a
//! spike of uploads inside the web process would starve HTTP and SSE.
//! That is a blast-radius decision, not a step toward microservices:
//! still one codebase, one database, one deployable artifact.
//!
//! # Why a library with a thin binary
//!
//! `main.rs` is a launcher; everything else lives here. That mirrors the
//! `cmd/server/main.go` shape of the Go boilerplate, and it buys two things
//! in Rust specifically: integration tests under `tests/` can drive the real
//! router, and a type that exists before its first caller is public API
//! rather than dead code — so the response envelope can be built and tested
//! before the first endpoint uses it, without a blanket `#[allow(dead_code)]`
//! hiding the parts that genuinely are unused.
//!
//! # Startup and shutdown are mirror images
//!
//! Up: configuration, logging, Postgres, migrations, Redis, router, listener.
//! Down: stop accepting, drain, Redis, Postgres — reverse order, so nothing
//! is closed while something that needs it is still finishing.

pub mod adapter;
pub mod platform;
pub mod shared;
pub mod usecase;

use std::fmt;

use platform::config::Config;
use platform::state::AppState;
use platform::{logging, shutdown};

/// Which process this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Web,
    Worker,
    /// Apply migrations and exit. See [`adapter::postgres::migrate`] for why
    /// this exists alongside the migration the web role runs on boot.
    Migrate,
}

/// The one command that is not a process role.
///
/// `grant-admin` takes an email and its reason comes from stdin, neither of
/// which `Role::parse`'s single argument has room for — see
/// [`run_grant_admin`]. Matched by hand in [`run`], before `Role::parse`,
/// the same way `Role::parse` itself hand-matches one argument rather than
/// reaching for an argument parser.
const GRANT_ADMIN: &str = "grant-admin";

/// The other command that is not a process role.
///
/// It prints two numbers and exits, so it is not a description of what the process *is*
/// — `Role` models that. Matched by hand for the same reason `grant-admin` is.
///
/// Shell access rather than an admin session, deliberately, and the same argument
/// `grant-admin` makes: this is an operator's question about the platform, not a
/// person's question about their own data, so it does not want an HTTP surface, a DTO,
/// or a rate limit.
const QUEUE_STATS: &str = "queue-stats";

impl Role {
    const USAGE: &'static str = "usage: anakmobil <web|worker|migrate>\n       \
                                 anakmobil grant-admin <email>\n       \
                                 anakmobil queue-stats";

    /// Read the role from the first command-line argument.
    ///
    /// Matched by hand rather than with an argument parser. There is one
    /// argument with three values; `clap` earns its place when there are
    /// flags, not before.
    ///
    /// # Errors
    ///
    /// Returns a message naming what was given, plus the usage line.
    pub fn parse(arg: Option<&str>) -> Result<Self, String> {
        match arg {
            Some("web") => Ok(Self::Web),
            Some("worker") => Ok(Self::Worker),
            Some("migrate") => Ok(Self::Migrate),
            Some(other) => Err(format!("unknown role `{other}`\n{}", Self::USAGE)),
            None => Err(format!("no role given\n{}", Self::USAGE)),
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Web => "web",
            Self::Worker => "worker",
            Self::Migrate => "migrate",
        })
    }
}

/// Read the environment, install logging, and run the requested role until a
/// shutdown signal arrives.
///
/// # Errors
///
/// Returns an error when the role is unknown, the configuration is invalid,
/// logging cannot be installed, or a dependency cannot be reached at boot.
pub async fn run() -> anyhow::Result<()> {
    // A missing .env is normal — production supplies real environment
    // variables and has no file to load.
    let _ = dotenvy::dotenv();

    let mut args = std::env::args().skip(1);
    let command = args.next();

    // Matched before `Role::parse`, because this is not a process role. It
    // takes an email, which a role does not, and the reason it needs comes
    // from stdin rather than from another argument — see `run_grant_admin`.
    if command.as_deref() == Some(GRANT_ADMIN) {
        let email = args.next().ok_or_else(|| anyhow::Error::msg(Role::USAGE))?;
        let config = Config::from_env()?;
        logging::init(config.app_env, &config.log_level)?;
        return run_grant_admin(&config, &email).await;
    }

    if command.as_deref() == Some(QUEUE_STATS) {
        let config = Config::from_env()?;
        logging::init(config.app_env, &config.log_level)?;
        return run_queue_stats(&config).await;
    }

    let role = Role::parse(command.as_deref()).map_err(anyhow::Error::msg)?;

    // Configuration is read before logging is installed, so its failures are
    // the one thing that cannot be logged. They surface through the error
    // return, which is correct: the process is about to exit, and a
    // structured log nobody is collecting yet helps nobody.
    let config = Config::from_env()?;
    logging::init(config.app_env, &config.log_level)?;

    tracing::info!(
        %role,
        app_env = ?config.app_env,
        // The *target*, never the URL — `database_target` strips credentials,
        // so a boot line cannot put a password in a log aggregator.
        database = config.database_target(),
        redis = config.redis_target(),
        "starting"
    );

    match role {
        Role::Web => run_web(&config).await?,
        Role::Worker => run_worker(&config).await?,
        Role::Migrate => run_migrate(&config).await?,
    }

    tracing::info!(%role, "stopped");
    Ok(())
}

/// Apply migrations and exit.
async fn run_migrate(config: &Config) -> anyhow::Result<()> {
    let pool = adapter::postgres::connect(config.database_url.expose())?;
    adapter::postgres::migrate::run(&pool).await?;
    pool.close().await;
    Ok(())
}

/// HTTP role.
async fn run_web(config: &Config) -> anyhow::Result<()> {
    let pool = adapter::postgres::connect(config.database_url.expose())?;

    // Before the listener binds, not after (AM-352 AC2). A process that
    // accepts a request against a schema it does not expect fails in ways
    // that are far harder to read than a startup that refuses.
    //
    // This is also the first real connection, so an unreachable database
    // stops the web role here rather than at the first request.
    adapter::postgres::migrate::run(&pool).await?;

    // Eager, because `ConnectionManager` establishes its first connection on
    // construction. A Redis that is down at boot fails the process here.
    let redis = adapter::redis::connect(config.redis_url.expose()).await?;

    let state = AppState {
        pool: pool.clone(),
        redis: redis.clone(),
        sessions: adapter::redis::session::SessionStore::new(redis.clone()),
        limiter: adapter::redis::rate_limit::RateLimiter::new(redis.clone()),
    };
    let router = adapter::http::router(state);

    adapter::http::serve(&config.bind_addr, router, shutdown::signal_received()).await?;

    // `serve` returns once the signal arrived and in-flight requests drained.
    // The rest of teardown runs under the same deadline as the drain, because
    // a budget that stops at the drain bounds nothing: `PgPool::close` waits
    // indefinitely for a connection still checked out by a stuck handler.
    tracing::info!("draining complete, closing connections");
    let closed = shutdown::within(adapter::http::DRAIN_TIMEOUT, async move {
        // Reverse of startup: Redis was opened last, so it closes first.
        drop(redis);
        pool.close().await;
    })
    .await;

    if closed {
        tracing::info!("connections closed");
    }
    Ok(())
}

/// Background worker role.
///
/// Claims from the Postgres queue, runs the job, and records the outcome; a lease that
/// expires hands an unfinished job to the next worker. See [`usecase::jobs`] for the
/// retry curve, the failure taxonomy, and why the loop is sequential.
///
/// Deliberately does **not** run migrations. One role owns applying them, and a worker
/// starting against a schema the web role has not migrated yet is exactly the case
/// expand-and-contract makes safe: a column is added in one release and only removed in
/// a later one, so an older reader keeps working.
///
/// No HTTP probe port here, deliberately. One was designed and dropped: a listener
/// running beside a job loop answers `200` while that loop is deadlocked, which is
/// precisely the failure it would have been added to catch.
///
/// The comment this replaces promised a progress heartbeat instead, "arriving with the
/// loop in AM-358". It has arrived, and it needed no new machinery: a wedged loop stops
/// settling, so the **age of the oldest pending job** — `anakmobil queue-stats` — climbs
/// without bound. That is the same signal a heartbeat table would have carried, read
/// from a column that already exists.
async fn run_worker(config: &Config) -> anyhow::Result<()> {
    let pool = adapter::postgres::connect(config.database_url.expose())?;

    usecase::jobs::run(&pool, dispatch, shutdown::signal_received()).await;

    tracing::info!("shutdown signal received, in-flight job finished");
    // Bounded, mirroring `run_web`: the loop is designed to survive a database outage by
    // logging and polling again, so a `pool.close()` left unbounded would wait on
    // connections it cannot close gracefully and turn the platform's own SIGTERM grace
    // period into a SIGKILL.
    let closed = shutdown::within(adapter::http::DRAIN_TIMEOUT, async move {
        pool.close().await;
    })
    .await;
    if closed {
        tracing::info!("connections closed");
    }
    Ok(())
}

/// Every job kind this build knows how to run.
///
/// None yet. `media.process` arrives with AM-359, and until it does an unknown kind is a
/// **permanent** failure rather than a transient one: a build that does not know a kind
/// will not learn it by waiting, and eight attempts to discover that would delay every
/// other job for twenty minutes to reach the same dead-letter.
///
/// The job's kind is logged; its payload never is. A payload carries a media id today
/// and something private tomorrow.
async fn dispatch(job: usecase::jobs::Job) -> usecase::jobs::JobOutcome {
    Err(usecase::jobs::JobFailure::Permanent(format!(
        "unknown job kind `{}`",
        job.kind
    )))
}

/// Grant the first platform admin, when the platform has none.
///
/// The way back in when there are zero admins — which is a legitimate state,
/// because there is no last-admin guard. Requiring shell access to the server
/// is a higher authority than any admin session, which is what makes it a
/// recovery path rather than a back door.
///
/// The reason is read from **stdin**, never from `argv`. An operational
/// reason is not a secret, but `--reason "granting Budi admin for catalog
/// curation"` lands in shell history and in every `ps` listing on the box.
/// Reading it from the terminal costs nothing and leaks nothing.
///
/// The zero-admin precondition is checked inside `set_role`'s transaction and
/// its lock, not here. Checking it here and then calling would be
/// check-then-act: two operators running this at once would both see zero.
async fn run_grant_admin(config: &Config, email: &str) -> anyhow::Result<()> {
    let pool = adapter::postgres::connect(config.database_url.expose())?;

    let reason = read_reason().await?;

    let mut conn = pool.acquire().await?;
    let target = adapter::postgres::user_repo::find_id_by_email(&mut conn, email)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no account is registered with that address"))?;
    drop(conn);

    let change = usecase::roles::set_role(
        &pool,
        usecase::roles::Actor::Bootstrap,
        target,
        adapter::postgres::user_repo::PlatformRole::Admin,
        &reason,
    )
    .await?;

    match change {
        Some(_) => println!("granted: {target} is now a platform admin"),
        None => println!("no change: {target} is already a platform admin"),
    }

    pool.close().await;
    Ok(())
}

/// Print the queue's two numbers and exit.
///
/// The age of the oldest job still owed work, and how many gave up. `println!` rather
/// than a log line: this is output a person asked for, not an event.
async fn run_queue_stats(config: &Config) -> anyhow::Result<()> {
    let pool = adapter::postgres::connect(config.database_url.expose())?;
    let mut conn = pool.acquire().await?;
    let stats = adapter::postgres::job_repo::stats(&mut conn).await?;
    drop(conn);

    match stats.oldest_pending_age_seconds {
        Some(age) => println!("oldest pending job: {age:.0}s"),
        None => println!("oldest pending job: none"),
    }
    println!("dead jobs: {}", stats.dead);

    pool.close().await;
    Ok(())
}

/// Read one line of reason from the terminal.
///
/// `spawn_blocking` rather than a direct read: this crate builds tokio
/// without the `io-std` feature, so there is no async stdin to reach for.
/// One line, and the feature stays out of the dependency list.
async fn read_reason() -> anyhow::Result<String> {
    eprint!("reason: ");
    let line = tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        Ok::<_, std::io::Error>(line)
    })
    .await??;
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_roles_parse() {
        assert_eq!(Role::parse(Some("web")), Ok(Role::Web));
        assert_eq!(Role::parse(Some("worker")), Ok(Role::Worker));
        assert_eq!(Role::parse(Some("migrate")), Ok(Role::Migrate));
    }

    #[test]
    fn unknown_role_names_itself_and_shows_usage() {
        let err = Role::parse(Some("webb")).unwrap_err();
        assert!(err.contains("webb"));
        assert!(err.contains("web|worker|migrate"));
    }

    #[test]
    fn missing_role_shows_usage() {
        let err = Role::parse(None).unwrap_err();
        assert!(err.contains("no role given"));
        assert!(err.contains("web|worker|migrate"));
    }

    #[test]
    fn role_renders_as_its_argument() {
        assert_eq!(Role::Web.to_string(), "web");
        assert_eq!(Role::Worker.to_string(), "worker");
        assert_eq!(Role::Migrate.to_string(), "migrate");
    }

    #[test]
    fn grant_admin_is_not_a_process_role() {
        // The prohibition, pinned. `Role` models the process a binary IS —
        // web, worker, migrate — and `CONTEXT.md` calls that a property of the
        // deployment rather than of any person. A fourth arm here would have
        // nowhere to put an email or a reason.
        let err = Role::parse(Some(GRANT_ADMIN)).unwrap_err();
        assert!(err.contains("unknown role"));
    }

    #[test]
    fn the_usage_line_names_every_way_to_start_this_binary() {
        // A recovery path nobody can discover is a recovery path that does not
        // exist.
        let err = Role::parse(Some("webb")).unwrap_err();
        assert!(err.contains("web|worker|migrate"));
        assert!(err.contains(GRANT_ADMIN));
        assert!(err.contains(QUEUE_STATS));
    }

    #[test]
    fn queue_stats_is_not_a_process_role() {
        // Same prohibition as `grant-admin`. `Role` is what the process IS; this prints
        // and exits.
        let err = Role::parse(Some(QUEUE_STATS)).unwrap_err();
        assert!(err.contains("unknown role"));
    }

    #[tokio::test]
    async fn an_unknown_kind_dead_letters_rather_than_retrying() {
        // AC3: an unknown kind must be Permanent, not Transient. Before this test,
        // flipping the variant reddened nothing in the suite — the only thing that
        // caught it was a manual `make be-worker` run, which happens once and never
        // again. A Transient classification here would retry eight times over ~21
        // minutes with every real job queued behind it waiting.
        let job = usecase::jobs::Job {
            id: uuid::Uuid::now_v7(),
            kind: "nope".to_owned(),
            payload: serde_json::json!({}),
            effect_key: None,
            attempts: 1,
            max_attempts: 8,
        };
        let outcome = dispatch(job).await;
        assert!(
            matches!(outcome, Err(usecase::jobs::JobFailure::Permanent(_))),
            "an unknown job kind must dead-letter on the first attempt, not retry"
        );
    }
}
