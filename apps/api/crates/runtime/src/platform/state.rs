//! Shared application state.
//!
//! Built once in the composition root and handed to handlers by axum's
//! `State` extractor. Both fields are internally reference-counted, so
//! cloning is a refcount bump rather than a new pool or a new connection.
//!
//! There is no dependency-injection framework, and that is the Rust idiom
//! rather than a gap. The Go boilerplate this mirrors needs Uber Fx and a
//! `ValidateApp` test to prove at startup that every dependency was provided;
//! here a missing field is a compile error, so the test has nothing left to
//! catch.

use redis::aio::ConnectionManager;
use sqlx::PgPool;

use crate::adapter::redis::rate_limit::RateLimiter;
use crate::adapter::redis::session::SessionStore;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    /// Kept for the readiness probe, which checks the connection itself
    /// rather than anything built on it.
    pub redis: ConnectionManager,
    pub sessions: SessionStore,
    pub limiter: RateLimiter,
}
