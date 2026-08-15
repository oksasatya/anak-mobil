//! Problems, solutions, communities, posts, and discovery.
//!
//! Entities: `Problem`, `Solution`, `Community`, `Post`, `Report`.
//! Value objects: `MembershipRole` (owner | admin | member),
//! `ContentStatus` (visible | reported | hidden | deleted).
//!
//! Policy candidates: report aggregation per object, membership
//! permission resolution, solution-acceptance rules.
//!
//! # ContentStatus is load-bearing beyond this module
//!
//! [`crate::ai`] must never cite content that has been reported, hidden,
//! or deleted — a platform that frames community text as "evidence" with
//! a confidence badge does more damage with bad content than a plain feed
//! would. Status changes here have to be observable by the indexer.
