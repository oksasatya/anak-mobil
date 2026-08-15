//! Graceful shutdown signalling and its deadline.
//!
//! Both process roles wait on the same future. When it resolves, the role
//! stops accepting new work, lets what is in flight finish, and closes its
//! resources in reverse startup order.
//!
//! Getting this in place before there is anything to shut down is deliberate:
//! retrofitting graceful shutdown once a server, a worker loop, and a
//! connection pool all exist means touching all three.

use std::time::Duration;

use tokio::signal;

/// Resolves on `Ctrl-C` or `SIGTERM`.
///
/// `SIGTERM` is what container platforms send first; a process that only
/// listens for `Ctrl-C` gets killed instead of drained.
///
/// Each caller creates its own listener. Tokio delivers a signal to every
/// registered listener, so two roles — or a server and a worker loop — do not
/// compete for it.
pub async fn signal_received() {
    let ctrl_c = async {
        if let Err(err) = signal::ctrl_c().await {
            // Before the subscriber is guaranteed to exist this could be
            // lost, but by the time a role is awaiting a signal, logging is
            // installed.
            tracing::error!(cause = %err, "cannot listen for ctrl-c");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(err) => tracing::error!(cause = %err, "cannot listen for SIGTERM"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

/// Run teardown, but never longer than `budget`.
///
/// The deadline covers **all** of teardown, not just draining requests. That
/// distinction is the whole reason this helper exists: a timeout wrapped
/// around the drain alone bounds nothing, because `PgPool::close` afterwards
/// waits indefinitely for any connection still checked out — so a single
/// stuck handler turns a 30-second shutdown into an indefinite one, and the
/// platform's `SIGKILL` becomes the real shutdown mechanism.
///
/// Returns `true` when teardown finished within the budget.
pub async fn within(budget: Duration, teardown: impl Future<Output = ()>) -> bool {
    match tokio::time::timeout(budget, teardown).await {
        Ok(()) => true,
        Err(_) => {
            tracing::warn!(
                budget_ms = budget.as_millis(),
                "shutdown deadline reached, exiting with work still in flight"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn teardown_that_finishes_reports_success() {
        assert!(within(Duration::from_secs(5), async {}).await);
    }

    #[tokio::test(start_paused = true)]
    async fn teardown_that_hangs_is_abandoned_at_the_deadline() {
        // The regression this function exists for. `std::future::pending`
        // stands in for a pool close that waits on a connection nobody is
        // going to return.
        let finished = within(Duration::from_secs(30), std::future::pending::<()>()).await;
        assert!(!finished, "a hung teardown must not block shutdown forever");
    }

    #[tokio::test(start_paused = true)]
    async fn the_deadline_covers_teardown_after_the_drain() {
        // Draining is fast, closing hangs. A budget wrapped around the drain
        // alone would report success here; wrapping the whole sequence does
        // not.
        let sequence = async {
            tokio::time::sleep(Duration::from_secs(1)).await; // drain
            std::future::pending::<()>().await; // close
        };
        assert!(!within(Duration::from_secs(30), sequence).await);
    }
}
