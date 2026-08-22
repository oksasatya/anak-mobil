//! The job queue's SQL.
//!
//! # Every function takes a connection, and that is the point
//!
//! Nothing here takes a `PgPool`. Spec §4 requires a caller to flip a media row's state
//! and insert its job in ONE transaction: crash between two separate operations and the
//! media sits in `processing` with no job to move it, which no lease can rescue because
//! no lease was ever taken. A repository holding its own pool would make that
//! transaction impossible to write.
//!
//! # "Next claimable at" is one indexed expression
//!
//! `COALESCE(leased_until, run_at)`. The migration's `jobs_lease_matches_state`
//! constraint is what makes that meaningful: `leased_until` is non-NULL exactly while
//! the job is leased, so the expression is the lease's expiry for a leased job and the
//! scheduled time for a queued one.
//!
//! That is also why there is no reaper. A separate task sweeping expired leases would
//! be a second writer racing the claimer for the same rows, to produce a state the
//! claim predicate can simply read.

use std::time::Duration;

use sqlx::PgConnection;
use uuid::Uuid;

/// A job, as the worker receives it.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub payload: serde_json::Value,
    /// See the column comment: a stable logical name for this job's side effect.
    pub effect_key: Option<String>,
    /// The attempt just consumed, one-based. The claim increments before the handler
    /// runs, so a worker killed mid-job has still spent this attempt.
    pub attempts: i32,
    pub max_attempts: i32,
}

/// What a caller supplies to queue work.
///
/// Three fields. `run_at` and `max_attempts` come from column defaults because nothing
/// today wants a different value, and the retry path sets `run_at` itself.
#[derive(Debug)]
pub struct NewJob<'a> {
    pub kind: &'a str,
    pub payload: &'a serde_json::Value,
    /// `None` means this job has no side effect worth deduping, and enqueuing it twice
    /// queues it twice.
    pub effect_key: Option<&'a str>,
}

/// Queue one job, inside the caller's transaction.
///
/// Returns `None` when an identical live effect key is already queued or leased — not
/// an error, because "this work is already on its way" is the outcome the caller wanted.
///
/// The `ON CONFLICT` names its arbiter index rather than using a bare `DO NOTHING`, so
/// a primary-key collision still surfaces as an error instead of being reported as a
/// duplicate effect. The `WHERE` clause restates the index predicate exactly, which is
/// what Postgres requires to infer a partial index as the arbiter.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the insert fails.
pub async fn enqueue(
    conn: &mut PgConnection,
    job: &NewJob<'_>,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (id, kind, payload, effect_key)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (effect_key)
            WHERE effect_key IS NOT NULL AND state IN ('queued', 'leased')
            DO NOTHING
        RETURNING id
        "#,
        Uuid::now_v7(),
        job.kind,
        job.payload,
        job.effect_key,
    )
    .fetch_optional(conn)
    .await
}

/// Take the next due job, for `lease`.
///
/// One statement, and that is deliberate: exclusion, fairness, the lease, and the
/// attempt increment all happen together or none of them does. Split across a SELECT and
/// an UPDATE, two workers read the same row before either wrote.
///
/// `SKIP LOCKED` is what makes several workers *concurrent* rather than merely correct.
/// Plain `FOR UPDATE` also hands each job to one worker — by making every other worker
/// wait for the first, which is a queue with one consumer wearing a costume.
///
/// The lease is computed by Postgres, never by the caller. With several replicas, a
/// lease measured against each worker's own clock has a different length on each machine.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the statement fails.
pub async fn claim(
    conn: &mut PgConnection,
    worker: Uuid,
    lease: Duration,
) -> Result<Option<Job>, sqlx::Error> {
    sqlx::query_as!(
        Job,
        r#"
        UPDATE jobs SET
            state        = 'leased',
            leased_until = now() + ($2::double precision * interval '1 second'),
            leased_by    = $1,
            attempts     = attempts + 1
        WHERE id = (
            SELECT id FROM jobs
            WHERE state IN ('queued', 'leased')
              AND COALESCE(leased_until, run_at) <= now()
            ORDER BY COALESCE(leased_until, run_at)
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING
            id           AS "id!",
            kind         AS "kind!",
            payload      AS "payload!",
            effect_key   AS "effect_key?",
            attempts     AS "attempts!",
            max_attempts AS "max_attempts!"
        "#,
        worker,
        lease.as_secs_f64(),
    )
    .fetch_optional(conn)
    .await
}

// The three settles below share one predicate: `id = $1 AND state = 'leased' AND
// leased_by = $2`. Scoped to the worker holding the lease, not merely to the job. A
// worker that stalled past its lease and woke to find another worker running its job
// must not be able to terminate that job -- its UPDATE has to affect nothing, so it can
// report having lost the lease instead of silently colliding. Each returns `true` only
// when it actually changed a row.
//
// `false` collapses three different situations into one bit: another worker holds the
// lease, the row is already terminal (a duplicate settle -- at-least-once makes this
// reachable, not exotic), or the row is gone entirely. The caller's response is the same
// in all three, so this is not a correctness gap, but a log line that says "lost the
// lease" for a row that was actually deleted or double-settled is misleading the exact
// person reading it to diagnose something.

/// Finish a job.
///
/// Clears `last_error`. Decided rather than defaulted: `attempts` already records that
/// a job retried, so the diagnostic value of keeping a stale failure text on a `done`
/// row is weak, while `last_error` is a free-text channel with no retention policy --
/// `done` rows are never cleaned up -- and no schema-level shape bound. A message built
/// from a `reqwest` error's `Display` can carry a credential (see [`JobFailure::message`]),
/// and this repository's own sanitisation only strips control characters; a presigned
/// URL survives that intact. A success should not be the reason a credential outlives it
/// indefinitely.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the statement fails.
pub async fn mark_done(
    conn: &mut PgConnection,
    id: Uuid,
    worker: Uuid,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        UPDATE jobs SET state = 'done', leased_until = NULL, leased_by = NULL, last_error = NULL
        WHERE id = $1 AND state = 'leased' AND leased_by = $2
        "#,
        id,
        worker,
    )
    .execute(conn)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Return a job to the queue, due after `delay`.
///
/// The delay is applied against Postgres's clock for the same reason the lease is.
///
/// `error` is written verbatim to `jobs.last_error`: this is the first place that
/// column is ever populated. Never a payload, and never a credential -- a `reqwest`
/// error's `Display` includes the URL, so a presigned request that times out would
/// otherwise write its signature here, readable by anyone with database access.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the statement fails.
pub async fn reschedule(
    conn: &mut PgConnection,
    id: Uuid,
    worker: Uuid,
    delay: Duration,
    error: &str,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        UPDATE jobs SET
            state        = 'queued',
            run_at       = now() + ($3::double precision * interval '1 second'),
            leased_until = NULL,
            leased_by    = NULL,
            last_error   = $4
        WHERE id = $1 AND state = 'leased' AND leased_by = $2
        "#,
        id,
        worker,
        delay.as_secs_f64(),
        error,
    )
    .execute(conn)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Stop trying.
///
/// Terminal. `dead` sits outside `jobs_claimable_idx`'s predicate, so the job is not
/// merely filtered out of the claim -- it is not in the index the claim reads.
///
/// `error` is written verbatim to `jobs.last_error`. Never a payload, and never a
/// credential -- see [`reschedule`].
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the statement fails.
pub async fn mark_dead(
    conn: &mut PgConnection,
    id: Uuid,
    worker: Uuid,
    error: &str,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        UPDATE jobs SET
            state        = 'dead',
            leased_until = NULL,
            leased_by    = NULL,
            last_error   = $3
        WHERE id = $1 AND state = 'leased' AND leased_by = $2
        "#,
        id,
        worker,
        error,
    )
    .execute(conn)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// AC5's two numbers.
#[derive(Debug, Clone)]
pub struct QueueStats {
    /// How long the oldest job the platform still owes work on has been waiting.
    ///
    /// `None` when nothing is pending. Counts **leased** jobs as well as queued ones:
    /// read as queued-only, this falls to zero precisely when a wedged worker is holding
    /// everything, which is the case it exists to reveal.
    pub oldest_pending_age_seconds: Option<f64>,
    /// How many jobs gave up.
    pub dead: i64,
}

/// Read the queue's health.
///
/// Two scalar subselects in one round trip. No metrics service, no exporter, no second
/// store — the answer is already in this table.
///
/// Cost is O(p + d) — p pending (queued + leased) rows, d dead rows — never O(n) against
/// total history, because both partial indexes exclude `done` rows. If this is ever
/// scraped on a timer rather than run by hand, add
/// `CREATE INDEX … ON jobs (created_at) WHERE state IN ('queued', 'leased')`, which drops
/// the `min` from O(p) to O(log p).
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn stats(conn: &mut PgConnection) -> Result<QueueStats, sqlx::Error> {
    sqlx::query_as!(
        QueueStats,
        r#"
        SELECT
            (SELECT extract(epoch FROM now() - min(created_at))::double precision
               FROM jobs WHERE state IN ('queued', 'leased'))
                AS "oldest_pending_age_seconds?",
            (SELECT count(*) FROM jobs WHERE state = 'dead') AS "dead!"
        "#
    )
    .fetch_one(conn)
    .await
}
