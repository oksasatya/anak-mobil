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

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub redis: ConnectionManager,
}
