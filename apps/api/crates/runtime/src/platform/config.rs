//! Configuration, read once at startup from the environment.
//!
//! Two properties matter here and both are acceptance criteria on AM-350:
//!
//! 1. **All bad keys are reported at once.** Fixing configuration one
//!    error per restart is a miserable loop, and it is entirely avoidable
//!    — collect every problem, then fail.
//! 2. **Secrets never print.** [`Secret`] has a `Debug` that shows
//!    nothing, so `dbg!(&config)` or a panic backtrace cannot leak a
//!    database password into a log aggregator.

use std::env::{self, VarError};
use std::fmt;
use std::str::FromStr;

/// A value that must never appear in logs, errors, or debug output.
///
/// The inner value is reachable through [`Secret::expose`], which is
/// deliberately noisy to read at a call site — seeing it in a diff should
/// prompt the question "does this end up in a log?".
#[derive(Clone, PartialEq, Eq)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Which environment the process believes it is running in.
///
/// This drives log format and a handful of safety checks — production
/// refuses to start with a placeholder secret, development does not care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnv {
    Development,
    Production,
}

impl FromStr for AppEnv {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "production" | "prod" => Ok(Self::Production),
            other => Err(format!(
                "expected `development` or `production`, got `{other}`"
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub app_env: AppEnv,
    pub bind_addr: String,
    pub log_level: String,
    pub database_url: Secret<String>,
    pub redis_url: Secret<String>,
}

/// Every problem found while reading the environment, not just the first.
#[derive(Debug, Default)]
pub struct ConfigError {
    problems: Vec<String>,
}

impl ConfigError {
    fn push(&mut self, problem: impl Into<String>) {
        self.problems.push(problem.into());
    }

    fn is_empty(&self) -> bool {
        self.problems.is_empty()
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "configuration is invalid ({} problem(s)):",
            self.problems.len()
        )?;
        for problem in &self.problems {
            writeln!(f, "  - {problem}")?;
        }
        f.write_str("see .env.example for the expected keys")
    }
}

impl std::error::Error for ConfigError {}

/// Read a required key. Records a problem and returns `None` when absent
/// or empty, so the caller can keep collecting instead of bailing out.
fn required(errors: &mut ConfigError, key: &str) -> Option<String> {
    match env::var(key) {
        Ok(value) if value.trim().is_empty() => {
            errors.push(format!("`{key}` is set but empty"));
            None
        }
        Ok(value) => Some(value),
        Err(VarError::NotPresent) => {
            errors.push(format!("`{key}` is required but not set"));
            None
        }
        Err(VarError::NotUnicode(_)) => {
            errors.push(format!("`{key}` is not valid unicode"));
            None
        }
    }
}

/// Read an optional key, falling back to `default`. An empty value is
/// treated as absent — `FOO=` in a `.env` file almost always means "I
/// meant to unset this", not "I want the empty string".
fn optional(key: &str, default: &str) -> String {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => default.to_owned(),
    }
}

/// Read an optional key and parse it, recording a problem if the value is
/// present but malformed.
fn parse_optional<T>(errors: &mut ConfigError, key: &str, default: T) -> T
where
    T: FromStr,
    T::Err: fmt::Display,
{
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => match value.parse::<T>() {
            Ok(parsed) => parsed,
            Err(err) => {
                errors.push(format!("`{key}` is invalid: {err}"));
                default
            }
        },
        _ => default,
    }
}

/// The `host:port` portion of a connection URL, with credentials removed.
///
/// Safe to log. It answers the question an operator actually has at boot
/// — *which server am I pointed at* — without revealing how the process
/// authenticates. `postgres://user:pw@db.internal:5432/anakmobil` becomes
/// `db.internal:5432`.
pub fn connection_target(url: &str) -> &str {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let after_credentials = after_scheme
        .split_once('@')
        .map_or(after_scheme, |(_, rest)| rest);
    after_credentials
        .split(['/', '?'])
        .next()
        .unwrap_or(after_credentials)
}

/// Shape and cross-field checks.
///
/// Takes the values as read rather than a built `Config`, so it can run
/// in the SAME pass as reading. An earlier version validated only after
/// reading succeeded, which meant a malformed `APP_ENV` hid a malformed
/// `DATABASE_URL` until the next restart — the exact one-error-per-restart
/// loop this module exists to avoid.
///
/// Keys that failed to read are `None` and are skipped: they already have
/// a problem recorded, and adding "must start with postgres" on top of
/// "is required but not set" is noise, not help.
fn check_shapes(
    errors: &mut ConfigError,
    app_env: AppEnv,
    bind_addr: &str,
    database_url: Option<&str>,
    redis_url: Option<&str>,
) {
    if !bind_addr.contains(':') {
        errors.push(format!(
            "`BIND_ADDR` must include a port, got `{bind_addr}`"
        ));
    }

    for (key, url, scheme) in [
        ("DATABASE_URL", database_url, "postgres"),
        ("REDIS_URL", redis_url, "redis"),
    ] {
        let Some(url) = url else { continue };

        if !url.starts_with(scheme) {
            errors.push(format!("`{key}` must start with `{scheme}`"));
        }

        if app_env == AppEnv::Production && (url.contains("localhost") || url.contains("127.0.0.1"))
        {
            errors.push(format!("`{key}` points at localhost in production"));
        }
    }
}

impl Config {
    /// Build the configuration from the process environment.
    ///
    /// Reports every problem found in one pass — missing keys, malformed
    /// values, and shape violations together — then fails. The process
    /// never starts half-configured.
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut errors = ConfigError::default();

        let app_env = parse_optional(&mut errors, "APP_ENV", AppEnv::Development);
        let bind_addr = optional("BIND_ADDR", "0.0.0.0:8080");
        let log_level = optional("LOG_LEVEL", "info");
        let database_url = required(&mut errors, "DATABASE_URL");
        let redis_url = required(&mut errors, "REDIS_URL");

        check_shapes(
            &mut errors,
            app_env,
            &bind_addr,
            database_url.as_deref(),
            redis_url.as_deref(),
        );

        match (errors.is_empty(), database_url, redis_url) {
            (true, Some(database_url), Some(redis_url)) => Ok(Self {
                app_env,
                bind_addr,
                log_level,
                database_url: Secret::new(database_url),
                redis_url: Secret::new(redis_url),
            }),
            _ => Err(errors),
        }
    }

    /// Where the database lives, credentials removed — safe to log.
    pub fn database_target(&self) -> &str {
        connection_target(self.database_url.expose())
    }

    /// Where Redis lives, credentials removed — safe to log.
    pub fn redis_target(&self) -> &str {
        connection_target(self.redis_url.expose())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Config {
        Config {
            app_env: AppEnv::Development,
            bind_addr: "0.0.0.0:8080".to_owned(),
            log_level: "info".to_owned(),
            database_url: Secret::new("postgres://localhost/anakmobil".to_owned()),
            redis_url: Secret::new("redis://localhost:6379".to_owned()),
        }
    }

    #[test]
    fn secret_never_prints_its_value() {
        let secret = Secret::new("hunter2".to_owned());
        assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
        assert_eq!(format!("{secret}"), "<redacted>");
        assert!(!format!("{:?}", base()).contains("anakmobil"));
    }

    #[test]
    fn all_problems_are_reported_together() {
        let mut errors = ConfigError::default();
        errors.push("first");
        errors.push("second");
        let rendered = errors.to_string();
        assert!(rendered.contains("2 problem(s)"));
        assert!(rendered.contains("first"));
        assert!(rendered.contains("second"));
    }

    /// Run the shape checks over a well-formed set, letting a test
    /// override just the field it cares about.
    fn shapes(
        app_env: AppEnv,
        bind_addr: &str,
        database_url: Option<&str>,
        redis_url: Option<&str>,
    ) -> ConfigError {
        let mut errors = ConfigError::default();
        check_shapes(&mut errors, app_env, bind_addr, database_url, redis_url);
        errors
    }

    const DB: &str = "postgres://localhost/anakmobil";
    const REDIS: &str = "redis://localhost:6379";

    #[test]
    fn bind_addr_without_port_is_rejected() {
        let errors = shapes(AppEnv::Development, "0.0.0.0", Some(DB), Some(REDIS));
        assert!(errors.to_string().contains("must include a port"));
    }

    #[test]
    fn wrong_url_scheme_is_rejected() {
        let errors = shapes(
            AppEnv::Development,
            "0.0.0.0:8080",
            Some("mysql://localhost/x"),
            Some(REDIS),
        );
        assert!(errors.to_string().contains("must start with `postgres`"));
    }

    #[test]
    fn production_rejects_localhost() {
        let errors = shapes(AppEnv::Production, "0.0.0.0:8080", Some(DB), Some(REDIS));
        let rendered = errors.to_string();
        assert!(rendered.contains("DATABASE_URL"));
        assert!(rendered.contains("REDIS_URL"));
        assert!(rendered.contains("2 problem(s)"));
    }

    #[test]
    fn development_allows_localhost() {
        let errors = shapes(AppEnv::Development, "0.0.0.0:8080", Some(DB), Some(REDIS));
        assert!(errors.is_empty());
    }

    #[test]
    fn a_missing_key_does_not_also_get_a_shape_complaint() {
        // `None` means the key was already reported as missing. Piling
        // "must start with postgres" on top of that is noise.
        let errors = shapes(AppEnv::Development, "0.0.0.0:8080", None, Some(REDIS));
        assert!(errors.is_empty());
    }

    #[test]
    fn shape_problems_surface_alongside_read_problems() {
        // The regression this test exists for: an earlier version bailed
        // out after reading, so a malformed APP_ENV hid a malformed
        // BIND_ADDR until the next restart.
        let mut errors = ConfigError::default();
        errors.push("`APP_ENV` is invalid: expected `development` or `production`");
        check_shapes(
            &mut errors,
            AppEnv::Development,
            "0.0.0.0",
            Some("mysql://x"),
            Some(REDIS),
        );

        let rendered = errors.to_string();
        assert!(rendered.contains("3 problem(s)"), "got: {rendered}");
        assert!(rendered.contains("APP_ENV"));
        assert!(rendered.contains("BIND_ADDR"));
        assert!(rendered.contains("DATABASE_URL"));
    }

    #[test]
    fn connection_target_strips_credentials() {
        assert_eq!(
            connection_target("postgres://user:pw@db.internal:5432/anakmobil"),
            "db.internal:5432"
        );
        assert_eq!(
            connection_target("redis://localhost:6379"),
            "localhost:6379"
        );
        assert_eq!(
            connection_target("postgres://db:5432/x?sslmode=require"),
            "db:5432"
        );
        assert_eq!(connection_target("db.internal:5432"), "db.internal:5432");
    }

    #[test]
    fn connection_target_never_leaks_a_password() {
        let url = "postgres://admin:sup3rs3cret@db.internal:5432/anakmobil";
        assert!(!connection_target(url).contains("sup3rs3cret"));
    }

    #[test]
    fn app_env_parses_short_and_long_forms() {
        assert_eq!("dev".parse::<AppEnv>(), Ok(AppEnv::Development));
        assert_eq!("PRODUCTION".parse::<AppEnv>(), Ok(AppEnv::Production));
        assert!("staging".parse::<AppEnv>().is_err());
    }
}
