//! Graceful shutdown signalling.
//!
//! Both process roles wait on the same future. When it resolves, the role
//! stops accepting new work, lets what is in flight finish, and closes
//! its resources in reverse startup order.
//!
//! Getting this in place before there is anything to shut down is
//! deliberate: retrofitting graceful shutdown once a server, a worker
//! loop, and a connection pool all exist means touching all three.

use tokio::signal;

/// Resolves on `Ctrl-C` or `SIGTERM`.
///
/// `SIGTERM` is what container platforms send first; a process that only
/// listens for `Ctrl-C` gets killed instead of drained.
pub async fn signal_received() {
    let ctrl_c = async {
        if let Err(err) = signal::ctrl_c().await {
            eprintln!("failed to listen for ctrl-c: {err}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(err) => eprintln!("failed to listen for SIGTERM: {err}"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}
