//! AnakMobil backend — one binary, two process roles.
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

mod adapter;
mod platform;
mod usecase;

use std::fmt;

use platform::config::Config;
use platform::shutdown;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Web,
    Worker,
}

impl Role {
    const USAGE: &'static str = "usage: anakmobil <web|worker>";

    fn parse(arg: Option<&str>) -> Result<Self, String> {
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // A missing .env is normal — production supplies real environment
    // variables and has no file to load.
    let _ = dotenvy::dotenv();

    let role = Role::parse(std::env::args().nth(1).as_deref()).map_err(anyhow::Error::msg)?;
    let config = Config::from_env()?;

    println!("anakmobil {role} starting ({:?})", config.app_env);

    match role {
        Role::Web => run_web(&config).await,
        Role::Worker => run_worker(&config).await,
    }

    println!("anakmobil {role} stopped");
    Ok(())
}

/// HTTP role.
///
/// The router, middleware stack, and health probes land in AM-351 and
/// AM-352. Until then this proves the binary boots, reads its
/// configuration, and drains on a signal rather than being killed.
async fn run_web(config: &Config) {
    println!("  bind    {}", config.bind_addr);
    println!("  log     {}", config.log_level);
    println!("  db      {}", config.database_target());
    println!("  redis   {}", config.redis_target());
    println!("  http    not wired yet — AM-351 (envelope), AM-352 (router, probes)");

    shutdown::signal_received().await;
    println!("shutdown signal received, draining");
}

/// Background worker role.
///
/// The Postgres-backed queue — lease, retry with backoff, dead-letter —
/// lands in AM-358, and the media pipeline it first serves in AM-359.
async fn run_worker(config: &Config) {
    println!("  log     {}", config.log_level);
    println!("  db      {}", config.database_target());
    println!("  redis   {}", config.redis_target());
    println!("  queue   not wired yet — AM-358 (queue), AM-359 (media)");

    shutdown::signal_received().await;
    println!("shutdown signal received, finishing in-flight jobs");
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
