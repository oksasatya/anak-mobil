//! The HTTP adapter: routes, middleware, and the server itself.

pub mod probe;
pub mod request_id;

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use tokio::net::TcpListener;

use crate::platform::state::AppState;

/// How long in-flight requests get to finish once shutdown starts.
///
/// Shared with the rest of teardown rather than owned by it — see
/// [`crate::platform::shutdown`] for why one deadline has to cover the whole
/// sequence.
pub const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// Build the router.
///
/// The request-id middleware wraps everything, including unmatched paths, so
/// a `404` still gets an identifier and a log line. That is asserted below
/// rather than assumed — a middleware that silently skips the fallback would
/// leave exactly the requests most worth tracing untraceable.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(probe::healthz))
        .route("/readyz", get(probe::readyz))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id::middleware))
}

/// Serve until a shutdown signal arrives, then drain.
///
/// # Errors
///
/// Returns an error when the address cannot be bound or the server fails
/// while running.
pub async fn serve<F>(addr: &str, router: Router, shutdown: F) -> anyhow::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind(addr).await?;
    let local: SocketAddr = listener.local_addr()?;
    tracing::info!(%local, "http listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// A router with the same middleware stack but no dependencies, so the
    /// wiring can be tested without Postgres or Redis.
    fn bare_router() -> Router {
        Router::new()
            .route("/healthz", get(probe::healthz))
            .layer(axum::middleware::from_fn(request_id::middleware))
    }

    async fn send(router: Router, path: &str) -> axum::response::Response {
        let request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("building a test request");
        router
            .oneshot(request)
            .await
            .expect("the router is infallible")
    }

    #[tokio::test]
    async fn liveness_answers_without_touching_a_dependency() {
        // The point of liveness: this router has no pool and no Redis, and it
        // still answers. A liveness probe that needed one would restart the
        // process every time a database blinked.
        let response = send(bare_router(), "/healthz").await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn every_response_carries_a_request_id() {
        let response = send(bare_router(), "/healthz").await;
        let id = response
            .headers()
            .get(request_id::HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(!id.is_empty(), "no x-request-id header on the response");
    }

    #[tokio::test]
    async fn two_requests_get_two_identifiers() {
        let first = send(bare_router(), "/healthz").await;
        let second = send(bare_router(), "/healthz").await;
        assert_ne!(
            first.headers().get(request_id::HEADER),
            second.headers().get(request_id::HEADER),
        );
    }

    #[tokio::test]
    async fn an_unmatched_path_still_gets_an_identifier() {
        // Codex raised this against the design: a layer that only wraps
        // matched routes leaves 404s unlogged and untraceable. Asserted here
        // rather than trusted, because the answer depends on how axum applies
        // the layer and that is not something to remember.
        let response = send(bare_router(), "/reset/a-token-that-should-never-be-logged").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let id = response
            .headers()
            .get(request_id::HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(
            !id.is_empty(),
            "a 404 got no x-request-id, so the layer skipped the fallback"
        );
    }
}
