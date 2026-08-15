//! Adapters — everything that touches the outside world.
//!
//! Inbound: HTTP handlers and their serde DTOs. Outbound: Postgres
//! repositories, the Redis client, object storage, and the LLM and
//! embedding clients.
//!
//! # Adapters translate, they do not decide
//!
//! A handler parses a request, calls one use case, and maps the result to
//! a response. A repository runs SQL and maps rows to and from domain
//! types. Neither branches on a business rule — that belongs in a policy
//! function or a use case.
//!
//! # Repositories take a connection, not a pool
//!
//! Every repository method accepts `&mut PgConnection` so the *caller*
//! owns the transaction. A repository that holds its own pool turns each
//! call into a separate transaction, and two concurrent requests can then
//! interleave into stale derived state.
//!
//! Repositories are concrete structs, not traits. A trait is added only
//! when the adapter will genuinely be swapped, when orchestration needs
//! an I/O seam to test without a network, or when a second implementation
//! actually exists. `LlmPort` and `EmbeddingPort` qualify (AM-363,
//! AM-364); Postgres repositories do not, because `#[sqlx::test]` gives a
//! transactional test database that exercises the real SQL.

pub mod http;
pub mod postgres;
pub mod redis;
