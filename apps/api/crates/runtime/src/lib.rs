//! AnakMobil backend — one library, two process roles.
//!
//! ```text
//! anakmobil web      # HTTP server
//! anakmobil worker   # background job consumer
//! ```
//!
//! Both roles run from the same build and share the same configuration.
//! They are separate *processes* rather than tasks in one process because
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
//! Up: configuration, logging, Postgres, Redis, router, listener.
//! Down: stop accepting, drain, Redis, Postgres — reverse order, so nothing
//! is closed while something that needs it is still finishing.

pub mod adapter;
pub mod platform;
pub mod shared;
pub mod usecase;

use std::fmt;

use platform::config::Config;
use platform::{logging, shutdown};

/// Which process this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Web,
    Worker,
}

impl Role {
    const USAGE: &'static str = "usage: anakmobil <web|worker>";

    /// Read the role from the first command-line argument.
    ///
    /// Matched by hand rather than with an argument parser. There is one
    /// argument with two values; `clap` earns its place when there are
    /// subcommands and flags, not before.
    ///
    /// # Errors
    ///
    /// Returns a message naming what was given, plus the usage line.
    pub fn parse(arg: Option<&str>) -> Result<Self, String> {
        match arg {
            Some("web") => Ok(Self::Web),
            Some("worker") => Ok(Self::Worker),
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

    let role = Role::parse(std::env::args().nth(1).as_deref()).map_err(anyhow::Error::msg)?;

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
        Role::Worker => run_worker().await,
    }

    tracing::info!(%role, "stopped");
    Ok(())
}

/// HTTP role.
///
/// The router, middleware, and probes land with the HTTP adapter.
async fn run_web(config: &Config) -> anyhow::Result<()> {
    tracing::info!(bind = %config.bind_addr, "http not wired yet");
    shutdown::signal_received().await;
    tracing::info!("shutdown signal received, draining");
    Ok(())
}

/// Background worker role.
///
/// The Postgres-backed queue — lease, retry with backoff, dead-letter — lands
/// in AM-358, and the media pipeline it first serves in AM-359.
///
/// No HTTP probe port here, deliberately. One was designed and dropped: a
/// listener running beside a job loop answers `200` while that loop is
/// deadlocked, which is precisely the failure it would have been added to
/// catch. Detecting a wedged worker needs a progress heartbeat from the loop
/// itself, so it arrives with the loop in AM-358.
async fn run_worker() {
    tracing::info!("worker queue not wired yet — AM-358 (queue), AM-359 (media)");
    shutdown::signal_received().await;
    tracing::info!("shutdown signal received, finishing in-flight jobs");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_roles_parse() {
        assert_eq!(Role::parse(Some("web")), Ok(Role::Web));
        assert_eq!(Role::parse(Some("worker")), Ok(Role::Worker));
    }

    #[test]
    fn unknown_role_names_itself_and_shows_usage() {
        let err = Role::parse(Some("webb")).unwrap_err();
        assert!(err.contains("webb"));
        assert!(err.contains("web|worker"));
    }

    #[test]
    fn missing_role_shows_usage() {
        let err = Role::parse(None).unwrap_err();
        assert!(err.contains("no role given"));
        assert!(err.contains("web|worker"));
    }

    #[test]
    fn role_renders_as_its_argument() {
        assert_eq!(Role::Web.to_string(), "web");
        assert_eq!(Role::Worker.to_string(), "worker");
    }
}
