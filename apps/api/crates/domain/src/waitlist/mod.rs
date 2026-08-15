//! Pre-launch waitlist signups from the public landing page.
//!
//! Entities: `WaitlistEntry`. Value objects: `WaitlistId`, `Email`.
//!
//! Policy candidates: duplicate-email resolution, consent recording.
//!
//! # Consent is data, not a checkbox
//!
//! An email address is personal data. The record has to carry what the
//! person agreed to and when, in a form that survives being exported or
//! deleted on request.
