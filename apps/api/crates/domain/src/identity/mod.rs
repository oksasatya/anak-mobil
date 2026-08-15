//! Accounts, sessions, and the two platform roles.
//!
//! Entities: `User`, `Session`. Value objects: `UserId`, `Email`,
//! `PasswordHash`, `Role` (admin | user).
//!
//! Policy candidates: role resolution, session expiry, and the rules for
//! what a suspended account may still do.
//!
//! Note that platform role (admin/user) is a different thing from
//! community membership role (owner/admin/member), which belongs to
//! [`crate::knowledge`]. Do not merge them.
