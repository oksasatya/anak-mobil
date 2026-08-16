//! Cross-cutting helpers shared by every adapter.
//!
//! The Rust cut of the shared kit in `go-backend-boilerplate`. Only the parts
//! with a consumer today exist:
//!
//! - [`i18n`] — error codes and the messages they render as
//! - [`errors`] — the single failure-to-HTTP mapping
//! - [`request_id`] — the identifier tying a log line to a response
//! - [`response`] — the envelope every endpoint answers in
//! - [`security`] — password hashing and token minting
//!
//! - [`iso_date`] — dates on the wire, because `time`'s own encoding is not one
//! - [`pagination`] — the cursor contract every list endpoint shares
//!
//! Deliberately absent until something needs it: `validation` (no shared
//! request-DTO rules yet). A `datetime` module would hold one RFC 3339
//! helper, which lives in [`response`] instead.

pub mod errors;
pub mod i18n;
pub mod iso_date;
pub mod pagination;
pub mod request_id;
pub mod response;
pub mod security;
