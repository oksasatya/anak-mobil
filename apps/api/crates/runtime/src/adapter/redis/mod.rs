//! The Redis client.
//!
//! Connectivity only. Sessions, rate limiting, and caching arrive with the
//! stories that need them.
//!
//! `ConnectionManager` rather than a bare `Client`: a bare client only parses
//! the URL and never opens a socket, so a readiness check built on one reports
//! healthy against a Redis that is not running. The manager owns a real
//! multiplexed connection and reconnects on its own, which readiness needs —
//! otherwise the first dropped connection stays dropped and the instance never
//! recovers.

use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client};

/// Open a managed connection.
///
/// This *does* touch the network, unlike the Postgres pool, because
/// `ConnectionManager` establishes its first connection eagerly.
///
/// # Errors
///
/// Returns [`redis::RedisError`] when the URL cannot be parsed or the first
/// connection cannot be established.
pub async fn connect(redis_url: &str) -> Result<ConnectionManager, redis::RedisError> {
    let client = Client::open(redis_url)?;
    ConnectionManager::new(client).await
}

/// Is Redis answering right now?
///
/// A real `PING` round trip. The manager may hold a connection that died
/// without telling us; only a command finds that out.
///
/// # Errors
///
/// Returns [`redis::RedisError`] when the command fails or the connection
/// cannot be re-established.
pub async fn check(manager: &ConnectionManager) -> Result<(), redis::RedisError> {
    let mut conn = manager.clone();
    let reply: String = conn.ping().await?;
    if reply.eq_ignore_ascii_case("PONG") {
        Ok(())
    } else {
        // A server that answers something else is not a server we should send
        // session lookups to.
        Err(redis::RedisError::from((
            redis::ErrorKind::UnexpectedReturnType,
            "PING did not answer PONG",
        )))
    }
}
