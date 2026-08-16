//! The HTTP adapter: routes, middleware, and the server itself.

pub mod auth;
pub mod catalog;
pub mod probe;
pub mod request_id;
pub mod services;
pub mod summary;
pub mod vehicles;

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::routing::{get, post, put};
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
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/logout", post(auth::logout))
        .route("/catalog/brands", get(catalog::brands))
        .route("/catalog/brands/{id}/models", get(catalog::models))
        .route(
            "/catalog/models/{id}/generations",
            get(catalog::generations),
        )
        .route("/catalog/generations/{id}/variants", get(catalog::variants))
        .route("/catalog/suggestions", post(catalog::suggest))
        // `/vehicles/order` is declared before `/vehicles/{id}` so the literal
        // wins; otherwise "order" is parsed as a vehicle id and every reorder
        // is a 400.
        .route("/vehicles/order", put(vehicles::reorder))
        .route("/vehicles", get(vehicles::list).post(vehicles::create))
        .route("/vehicles/{id}/summary", get(summary::vehicle))
        .route(
            "/vehicles/{id}/services",
            get(services::list).post(services::create),
        )
        .route(
            "/services/{id}",
            get(services::detail)
                .put(services::update)
                .delete(services::delete),
        )
        .route(
            "/vehicles/{id}",
            get(vehicles::detail)
                .put(vehicles::update)
                .delete(vehicles::delete),
        )
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

    // `into_make_service_with_connect_info` is what puts the peer address into
    // the request, and the login rate limiter reads it. Without it the handler
    // still compiles and then fails at runtime on every call — the extractor
    // finds no `ConnectInfo` extension — so the two must change together.
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
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
