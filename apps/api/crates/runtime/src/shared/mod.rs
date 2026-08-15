//! Cross-cutting helpers shared by every adapter.
//!
//! The Rust cut of the shared kit in `go-backend-boilerplate`. Only the parts
//! with a consumer today exist:
//!
//! - [`i18n`] — error codes and the messages they render as
//! - [`errors`] — the single failure-to-HTTP mapping
//! - [`request_id`] — the identifier tying a log line to a response
//! - [`response`] — the envelope every endpoint answers in
//!
//! Deliberately absent until something needs them, each arriving with its
//! first consumer rather than as an empty directory: `pagination` (no list
//! endpoint yet), `validation` (no request DTO yet), and `security` (no
//! authentication yet). A `datetime` module would hold one RFC 3339 helper,
//! which lives in [`response`] instead.

pub mod errors;
pub mod i18n;
pub mod request_id;
pub mod response;
