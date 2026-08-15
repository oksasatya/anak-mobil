//! Structured logging.
//!
//! JSON in production, because the log is read by a machine and a grep for
//! `"error.code":"internal_error"` should be exact. Human-readable in
//! development, because it is read by a person.
//!
//! # What must never appear in a log line
//!
//! Tokens, passwords, number plates, and VINs. That is a repository rule
//! (AM-352 AC2), and it is kept structurally rather than by remembering:
//!
//! - The request span records a fixed set of scalar fields — never a request
//!   body, never a whole struct. A VIN cannot leak from a value that is never
//!   passed to a logging macro.
//! - The unmatched-route label is a constant, not the URI. A request to
//!   `/reset/<token>` that matches nothing logs `unmatched`, because the URI
//!   is exactly where a secret ends up when someone puts one in a path.
//! - Configuration secrets are wrapped in `Secret<T>`, whose `Debug` prints
//!   `Secret(<redacted>)`, so a stray `?config` cannot spill one.
//!
//! None of that protects a future `tracing::error!(error = %err)` somewhere
//! else in the tree — an error's `Display` can carry anything it was handed.
//! The rule that covers that case is in `shared::errors`: a cause is rendered
//! at exactly one place, on the 5xx path, into the log and never into a body.

use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;

use crate::platform::config::AppEnv;

/// Logging could not be initialised.
#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    #[error("`LOG_LEVEL` is not a valid filter directive: {0}")]
    Filter(String),
    #[error("a logging subscriber was already installed")]
    AlreadyInstalled,
}

/// The levels a `LOG_LEVEL` may open with.
const LEVELS: [&str; 5] = ["trace", "debug", "info", "warn", "error"];

/// Reject a directive that parses but sets no global level.
///
/// `EnvFilter` is more permissive than it looks. `LOG_LEVEL=nonsense` does
/// **not** fail to parse — it is read as a *target* named `nonsense`, which
/// leaves the global default unset and the service logging almost nothing.
/// A typo would then be invisible: the process starts, serves traffic, and
/// goes quiet, and the first person to notice is whoever needs a log during
/// an incident.
///
/// So the first directive must be a bare level. Everything after it is passed
/// through untouched, which keeps the useful compound form — `info,sqlx=warn`
/// — working.
fn check_directive(log_level: &str) -> Result<(), LoggingError> {
    let first = log_level
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    if LEVELS.contains(&first.as_str()) {
        Ok(())
    } else {
        Err(LoggingError::Filter(format!(
            "must begin with one of {}, got `{first}`",
            LEVELS.join(", ")
        )))
    }
}

/// Install the global subscriber.
///
/// Called once, immediately after configuration is read and before anything
/// else — a line written before this runs is not structured, which is why
/// configuration failures surface through the error return rather than a log.
///
/// # Errors
///
/// Returns [`LoggingError`] when `LOG_LEVEL` does not open with a level, is
/// not a valid filter directive, or when a subscriber is already installed.
pub fn init(app_env: AppEnv, log_level: &str) -> Result<(), LoggingError> {
    check_directive(log_level)?;
    let filter =
        EnvFilter::try_new(log_level).map_err(|err| LoggingError::Filter(err.to_string()))?;

    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true);

    let installed = match app_env {
        // `flatten_event` lifts the event's own fields to the top level, so a
        // query is `.error.code` rather than `.fields["error.code"]`.
        AppEnv::Production => builder.json().flatten_event(true).try_init(),
        AppEnv::Development => builder
            // Colour only when something is actually attached to the terminal;
            // a piped or redirected dev log otherwise fills with escape codes.
            .with_ansi(std::io::stdout().is_terminal())
            .with_target(false)
            .try_init(),
    };

    installed.map_err(|_| LoggingError::AlreadyInstalled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directive_that_sets_no_global_level_is_rejected() {
        // The regression this check exists for. `EnvFilter` accepts each of
        // these happily and reads them as target names, leaving the global
        // level unset — the service starts, serves, and logs nothing.
        for directive in ["nonsense", "not a level", "infoo", "sqlx=warn", ""] {
            let err = check_directive(directive)
                .expect_err(&format!("`{directive}` should have been rejected"));
            assert!(matches!(err, LoggingError::Filter(_)), "got {err:?}");
        }
    }

    #[test]
    fn every_level_is_accepted() {
        for level in LEVELS {
            assert!(check_directive(level).is_ok(), "`{level}` should be valid");
            assert!(EnvFilter::try_new(level).is_ok(), "`{level}` should parse");
        }
    }

    #[test]
    fn a_compound_directive_still_works() {
        // The useful form: a global default plus a quieter target.
        for directive in [
            "info,sqlx=warn",
            "debug,hyper=info,sqlx=warn",
            "  INFO , sqlx=warn",
        ] {
            assert!(
                check_directive(directive).is_ok(),
                "`{directive}` should be valid"
            );
        }
    }

    #[test]
    fn the_message_names_what_was_given_and_what_was_expected() {
        // This message is what an operator reads when the process refuses to
        // start. "invalid LOG_LEVEL" would send them to the source; naming
        // both halves does not.
        let message = check_directive("verbose")
            .expect_err("should reject")
            .to_string();
        assert!(
            message.contains("verbose"),
            "should quote what was given: {message}"
        );
        assert!(
            message.contains("info"),
            "should list the valid levels: {message}"
        );
    }
}
