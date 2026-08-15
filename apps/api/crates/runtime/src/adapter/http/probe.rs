//! Liveness and readiness.
//!
//! These two answer different questions, and conflating them is the classic
//! way to build a probe that cannot fail:
//!
//! - **`/healthz` — liveness.** Is the process alive? It performs no I/O and
//!   always answers `200`. A dependency being down is not a reason to kill and
//!   restart the process; restarting it will not bring Postgres back.
//! - **`/readyz` — readiness.** Can this instance serve? Postgres and Redis
//!   are both checked, and either being unreachable answers `503`.
//!
//! Redis is checked as strictly as Postgres. It holds sessions and rate-limit
//! state, so an instance that cannot reach it cannot authenticate anyone.
//!
//! An earlier design tolerated a few consecutive Redis failures before
//! reporting not-ready, to stop one blip from rotating every instance out at
//! once. That goal is right and the mechanism was wrong twice over: a counter
//! in this process is racy when two probes overlap, and it is per-process, so
//! replicas disagree about the same outage. The platform already implements
//! exactly this correctly — a probe `failureThreshold` counts consecutive
//! failures with knowledge of when it sent them. Tolerance is configured
//! there; this endpoint's job is to answer honestly.
//!
//! Both responses are flat JSON, deliberately outside the `{meta, data}`
//! envelope. A load balancer is not an API client, and reading `meta.status`
//! to learn a status it already has on the status line serves nobody.

use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use tokio::time::timeout;

use crate::adapter::{postgres, redis};
use crate::platform::state::AppState;

/// A dependency that has not answered within this long is not usable, and a
/// probe that waits longer than the platform's own probe timeout just moves
/// the failure somewhere less legible.
const CHECK_TIMEOUT: Duration = Duration::from_secs(2);

const OK: &str = "ok";
const UNREACHABLE: &str = "unreachable";

#[derive(Serialize)]
struct Liveness {
    status: &'static str,
}

#[derive(Serialize)]
struct Readiness {
    status: &'static str,
    postgres: &'static str,
    redis: &'static str,
}

/// Liveness. No I/O, no dependencies, always `200`.
pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(Liveness { status: "alive" }))
}

/// Readiness. `200` only when every dependency answers.
pub async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    // Concurrently, so the worst case is one timeout rather than two.
    let (postgres_result, redis_result) = tokio::join!(
        timeout(CHECK_TIMEOUT, postgres::check(&state.pool)),
        timeout(CHECK_TIMEOUT, redis::check(&state.redis)),
    );

    let postgres_ok = report("postgres", postgres_result);
    let redis_ok = report("redis", redis_result);
    let ready = postgres_ok && redis_ok;

    let body = Readiness {
        status: if ready { "ready" } else { "not_ready" },
        postgres: label(postgres_ok),
        redis: label(redis_ok),
    };

    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(body))
}

const fn label(ok: bool) -> &'static str {
    if ok { OK } else { UNREACHABLE }
}

/// Reduce a check to a boolean, logging why it failed.
///
/// The reason goes to the log and never to the response: a connection error
/// renders the host, the port, and sometimes the user, and this endpoint is
/// reachable by anything that can reach the service.
fn report<E: std::fmt::Display>(
    dependency: &'static str,
    result: Result<Result<(), E>, tokio::time::error::Elapsed>,
) -> bool {
    match result {
        Ok(Ok(())) => true,
        Ok(Err(err)) => {
            tracing::warn!(dependency, cause = %err, "readiness check failed");
            false
        }
        Err(_) => {
            tracing::warn!(
                dependency,
                timeout_ms = CHECK_TIMEOUT.as_millis(),
                "readiness check timed out"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Boom;

    impl std::fmt::Display for Boom {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("connection refused to db.internal:5432")
        }
    }

    #[test]
    fn a_healthy_check_reports_ready() {
        assert!(report::<Boom>("postgres", Ok(Ok(()))));
    }

    #[test]
    fn a_failed_check_reports_not_ready() {
        assert!(!report("postgres", Ok(Err(Boom))));
    }

    #[tokio::test]
    async fn a_timed_out_check_reports_not_ready() {
        // A hung dependency and a refused one are the same answer to a load
        // balancer, and the timeout is what stops the first from looking
        // healthy simply because nobody waited long enough to find out.
        //
        // The `Elapsed` value is produced by a real timeout rather than
        // constructed, because its constructor is private — which is a fair
        // constraint: it means this asserts the type the production path
        // actually receives.
        let elapsed: Result<Result<(), Boom>, _> =
            timeout(Duration::ZERO, std::future::pending()).await;
        assert!(elapsed.is_err(), "a zero-length timeout must elapse");
        assert!(!report("redis", elapsed));
    }

    #[test]
    fn labels_say_which_dependency_is_down() {
        assert_eq!(label(true), OK);
        assert_eq!(label(false), UNREACHABLE);
    }
}
