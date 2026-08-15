//! Answers, evidence, confidence, and usage quota.
//!
//! Entities: `Answer`, `Conversation`. Value objects: `AnswerId`,
//! `Confidence`, `Evidence`, `SafetyWarning`, `QuotaPeriod`.
//!
//! Policy candidates: confidence derivation, quota arithmetic, and the
//! safety-topic classification that decides whether an answer must carry
//! a warning.
//!
//! # Confidence is computed from constraint match, not embedding distance
//!
//! Semantic similarity is not vehicle identity. A question about PCD for
//! one variant can retrieve a problem from a different generation that
//! merely reads similarly. Confidence must fall out of how well the
//! evidence matches brand/model/generation/variant plus source
//! provenance — never out of a vector distance, which would make the
//! badge decorative while the answer is about the wrong car.
//!
//! # Answers are persisted whole
//!
//! An answer is stored complete — prose, confidence, evidence, warnings —
//! before it is considered done. Streaming is transport for the typing
//! experience only. A dropped connection must never leave a user reading
//! an answer whose safety warning never arrived.
