//! Cross-cutting plumbing: configuration, shutdown, and later logging,
//! the database pool, and the Redis client.
//!
//! Nothing here holds business rules. If a decision belongs to the
//! product rather than to the process, it belongs in the domain crate.

pub mod config;
pub mod shutdown;
