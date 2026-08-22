//! The job queue's policy and its worker loop.
//!
//! # At-least-once, said out loud
//!
//! A job can run more than once. That is not a defect to be engineered away — a worker
//! can die between finishing its work and recording that it finished, and no amount of
//! care inside this process closes that window. What the queue guarantees is that a job
//! is never *lost*: a lease that expires returns it, and a job that keeps failing stops
//! rather than retrying forever.
//!
//! What follows from that is the consumer's obligation. A side effect inside this
//! database is made idempotent by the transaction. A side effect *outside* it — an
//! object written to storage, a push handed to a provider — is the consumer's to dedupe,
//! and `jobs.effect_key` exists so it has a stable name to dedupe on.
//!
//! # Two kinds of failure, and the difference is the whole retry design
//!
//! [`JobFailure::Transient`] is the world being briefly unavailable: storage
//! unreachable, the database gone. It comes back, so the job comes back, after a delay
//! that grows.
//!
//! [`JobFailure::Permanent`] is the input being wrong: a malformed image, an unknown
//! job kind. It will fail identically on every attempt, so retrying it eight times
//! wastes the worker and delays every other job on the way to the same dead-letter.
//! Spec §7 makes this a requirement rather than an optimisation.

use std::time::Duration;

use sqlx::PgPool;
use uuid::Uuid;

use crate::adapter::postgres::job_repo;

/// How long a claimed job is the claiming worker's, before another may take it.
///
/// **Must exceed the longest a single job can run.** AM-359 bounds image decoding on
/// wall-clock time; that bound has to stay under this number, or a worker's own lease
/// expires under it and a second worker starts the same job while the first is still
/// going. The two numbers live in different modules and nothing connects them
/// automatically, which is exactly why this sentence is here.
pub const LEASE: Duration = Duration::from_secs(300);

/// First retry delay. Doubles per attempt, up to [`BACKOFF_CAP_SECS`].
const BACKOFF_BASE_SECS: u64 = 10;

/// Fifteen minutes. Not reached at the default `max_attempts` of 8; it exists so that
/// raising the cap later cannot produce a delay measured in days.
const BACKOFF_CAP_SECS: u64 = 900;

/// How much of a failure message reaches `jobs.last_error`.
const ERROR_MAX_CHARS: usize = 500;

/// Why a job did not finish.
#[derive(Debug)]
pub enum JobFailure {
    /// The world was briefly unavailable. Retry after a growing delay.
    Transient(String),
    /// The input is wrong and will be wrong next time. Dead-letter immediately.
    Permanent(String),
}

impl JobFailure {
    /// What the failure said. Reaches `jobs.last_error`, truncated; never a payload,
    /// and never a credential. A `reqwest` error's `Display` includes the URL, so
    /// `format!("{err}")` on a timed-out presigned PUT writes its `X-Amz-Signature`
    /// here -- readable by anyone with database access or `queue-stats`. Build this
    /// message from what failed, not from the error's own rendering, when the error
    /// came from a signed or credentialed request.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Transient(message) | Self::Permanent(message) => message,
        }
    }
}

/// What a handler reports back to the loop.
pub type JobOutcome = Result<(), JobFailure>;

/// What the loop does with a failed job.
#[derive(Debug, PartialEq, Eq)]
enum Settlement {
    Retry(Duration),
    Dead,
}

/// How long to wait before the next attempt.
///
/// `attempts` is the attempt just consumed, one-based — the claim increments before the
/// handler runs — so the first retry waits [`BACKOFF_BASE_SECS`].
///
/// The shift is clamped before it happens. `attempts` comes from a database column that
/// nothing forbids from holding a large number, and `1u64 << 64` panics in a debug
/// build; clamping is cheaper than a `checked_shl` whose `None` arm would need a
/// meaning invented for it.
fn backoff(attempts: i32) -> Duration {
    let shift = attempts.saturating_sub(1).clamp(0, 20) as u32;
    let seconds = BACKOFF_BASE_SECS
        .saturating_mul(1_u64 << shift)
        .min(BACKOFF_CAP_SECS);
    Duration::from_secs(seconds)
}

/// Retry, or stop trying.
///
/// A permanent failure never retries, whatever the attempt count. A transient one
/// retries until the attempt just consumed reaches the cap — `>=` rather than `>`,
/// because `attempts` has already been incremented by the claim, so `attempts == 8`
/// with `max_attempts == 8` means the eighth and final attempt has just been spent.
fn settle_for(attempts: i32, max_attempts: i32, failure: &JobFailure) -> Settlement {
    match failure {
        JobFailure::Permanent(_) => Settlement::Dead,
        JobFailure::Transient(_) if attempts >= max_attempts => Settlement::Dead,
        JobFailure::Transient(_) => Settlement::Retry(backoff(attempts)),
    }
}

/// Cut a failure message down to what `last_error` will hold.
///
/// By characters, not bytes. Slicing a `&str` at a byte index that is not a character
/// boundary panics, and a message carrying Indonesian text or a file name is precisely
/// where a multi-byte character lands on the boundary.
///
/// Control characters are mapped to a space BEFORE the take, not stripped after, so
/// truncation still counts 500 visible characters. Postgres `text` cannot hold `U+0000`
/// at all -- `UPDATE … SET last_error = $n` is rejected with `invalid byte sequence for
/// encoding "UTF8": 0x00`, which is the one statement in this queue that is worst to
/// lose: `mark_dead` failing this way leaves the row `leased` forever, cycling on every
/// lease expiry with no dead-letter row to show for it. A newline or a `\x1b[` escape
/// would separately forge lines in an operator's terminal via `queue-stats`.
fn trim_error(message: &str) -> String {
    message
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(ERROR_MAX_CHARS)
        .collect()
}

pub use crate::adapter::postgres::job_repo::Job;

/// How long to wait before asking again, when the queue was empty.
const IDLE_POLL: Duration = Duration::from_secs(1);

/// Claim, run, settle, repeat — until the shutdown signal arrives.
///
/// # One job at a time
///
/// AC1's "several workers at once" is several *processes*, which the claim's
/// `SKIP LOCKED` already serves. In-process concurrency is a separate axis, and it is
/// not free: a semaphore to bound it, a `JoinSet` to collect it, and a settle path that
/// can no longer assume one outstanding job.
///
/// ponytail: sequential on purpose. When one replica is measurably the bottleneck, the
/// upgrade is a `tokio::sync::Semaphore` around a `JoinSet` of claim-run-settle tasks,
/// each with its own connection — not a second queue.
///
/// # Where the shutdown check sits, and why it is not a `select!` around the handler
///
/// `tokio::select!` cancels the branch that loses, so a signal arriving while a handler
/// is running would drop that handler's future at whatever await point it had reached —
/// a job half-done, its lease still held, and a log line saying the worker shut down
/// cleanly. So the signal is only *checked* between jobs. A process killed mid-job is
/// what the lease exists for: the job comes back.
///
/// # A database failure does not kill the worker
///
/// It logs and waits. A worker that exits on every hiccup becomes a restart loop whose
/// recovery is slower than simply asking again a second later, and a settle that fails
/// leaves the job leased — which the lease then returns. At-least-once, working as
/// described.
pub async fn run<H, Fut>(pool: &PgPool, handle: H, shutdown: impl Future<Output = ()>)
where
    H: Fn(Job) -> Fut,
    Fut: Future<Output = JobOutcome>,
{
    let worker = Uuid::now_v7();
    // Logged once so an operator can join `jobs.leased_by` to a process without this
    // table needing to carry a hostname.
    tracing::info!(%worker, "worker loop started");

    let mut shutdown = std::pin::pin!(shutdown);

    loop {
        let stopping = tokio::select! {
            biased;
            () = shutdown.as_mut() => true,
            () = std::future::ready(()) => false,
        };
        if stopping {
            break;
        }

        match claim_one(pool, worker).await {
            Ok(Some(job)) => run_one(pool, worker, &handle, job).await,
            Ok(None) => {
                tokio::select! {
                    () = shutdown.as_mut() => break,
                    () = tokio::time::sleep(IDLE_POLL) => {}
                }
            }
            Err(err) => {
                tracing::error!(cause = %err, "could not claim a job");
                // A backed-off wait, not the idle one: a down connection already burns
                // the pool's acquire_timeout, but a query that fails instantly (e.g. a
                // worker deployed ahead of the web role's migration) would otherwise log
                // this line once a second, forever.
                tokio::select! {
                    () = shutdown.as_mut() => break,
                    () = tokio::time::sleep(IDLE_POLL * 5) => {}
                }
            }
        }
    }

    tracing::info!(%worker, "worker loop stopped");
}

/// Take the next job, or report that there was none.
async fn claim_one(pool: &PgPool, worker: Uuid) -> Result<Option<Job>, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    job_repo::claim(&mut conn, worker, LEASE).await
}

/// Run one job and record what happened.
///
/// Extracted from [`run`] so neither has a shape clippy's cognitive-complexity lint has
/// to argue about, and so the settle decision reads as one `match` rather than as three
/// arms nested inside a loop.
async fn run_one<H, Fut>(pool: &PgPool, worker: Uuid, handle: &H, job: Job)
where
    H: Fn(Job) -> Fut,
    Fut: Future<Output = JobOutcome>,
{
    let id = job.id;
    // Captured before `job` moves into `handle`: the two settle warnings below want the
    // kind, and it is the one field of a `Job` that is not private (F9's own reasoning
    // keeps logical names visible) and the one that makes a dead-letter line actionable
    // without a `psql` session.
    let kind = job.kind.clone();
    let attempts = job.attempts;
    let max_attempts = job.max_attempts;

    let outcome = handle(job).await;

    if let Err(err) = settle(pool, worker, id, &kind, attempts, max_attempts, outcome).await {
        // The job stays leased. Its lease expires and another worker takes it — which
        // is the at-least-once contract doing its job, not a leak.
        tracing::error!(job = %id, cause = %err, "could not record a job's outcome");
    }
}

/// Write the outcome to the row.
///
/// Seven parameters, at the workspace's `too_many_arguments` ceiling of seven and over
/// the ≤5 this repository aims for. Left as parameters deliberately: a struct bundling
/// them would have exactly one construction site and one read site, which is an
/// abstraction with no second caller to justify it. If a reviewer disagrees, fold this
/// back into `run_one` and take the cognitive-complexity cost instead — do not add the
/// struct.
async fn settle(
    pool: &PgPool,
    worker: Uuid,
    id: Uuid,
    kind: &str,
    attempts: i32,
    max_attempts: i32,
    outcome: JobOutcome,
) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await?;

    let settled = match outcome {
        Ok(()) => job_repo::mark_done(&mut conn, id, worker).await?,
        Err(failure) => {
            let message = trim_error(failure.message());
            match settle_for(attempts, max_attempts, &failure) {
                Settlement::Retry(delay) => {
                    tracing::warn!(
                        job = %id,
                        kind = %kind,
                        attempts,
                        retry_in_s = delay.as_secs(),
                        "job failed, retrying"
                    );
                    job_repo::reschedule(&mut conn, id, worker, delay, &message).await?
                }
                Settlement::Dead => {
                    tracing::warn!(job = %id, kind = %kind, attempts, "job dead-lettered");
                    job_repo::mark_dead(&mut conn, id, worker, &message).await?
                }
            }
        }
    };

    if !settled {
        // Another worker holds this job now, which means this one stalled past its own
        // lease and the work has just been done twice. Worth saying out loud: it is the
        // signal that LEASE is shorter than a real job takes.
        tracing::warn!(job = %id, %worker, "lost the lease before settling");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transient() -> JobFailure {
        JobFailure::Transient("storage unreachable".to_owned())
    }

    fn permanent() -> JobFailure {
        JobFailure::Permanent("not an image".to_owned())
    }

    #[test]
    fn the_delay_doubles_from_the_base() {
        assert_eq!(backoff(1), Duration::from_secs(10));
        assert_eq!(backoff(2), Duration::from_secs(20));
        assert_eq!(backoff(3), Duration::from_secs(40));
        assert_eq!(backoff(7), Duration::from_secs(640));
    }

    #[test]
    fn the_delay_stops_growing_at_the_cap() {
        assert_eq!(backoff(20), Duration::from_secs(BACKOFF_CAP_SECS));
        // The regression this guards: `1u64 << (attempts - 1)` panics in a debug build
        // once the shift passes 63, and `attempts` is a database column.
        assert_eq!(backoff(i32::MAX), Duration::from_secs(BACKOFF_CAP_SECS));
    }

    #[test]
    fn a_zero_or_negative_attempt_count_does_not_panic() {
        assert_eq!(backoff(0), Duration::from_secs(10));
        assert_eq!(backoff(-5), Duration::from_secs(10));
    }

    #[test]
    fn a_transient_failure_retries_below_the_cap() {
        assert_eq!(
            settle_for(1, 8, &transient()),
            Settlement::Retry(Duration::from_secs(10))
        );
        assert_eq!(
            settle_for(7, 8, &transient()),
            Settlement::Retry(Duration::from_secs(640))
        );
    }

    #[test]
    fn the_last_attempt_dead_letters_rather_than_retrying() {
        // AC3's terminal state. `attempts` has already been incremented by the claim,
        // so 8 of 8 means the eighth attempt has been spent, not that a ninth is due.
        assert_eq!(settle_for(8, 8, &transient()), Settlement::Dead);
        assert_eq!(settle_for(9, 8, &transient()), Settlement::Dead);
    }

    #[test]
    fn a_permanent_failure_never_retries() {
        // Spec §7. A malformed image fails identically every time; eight attempts to
        // learn that delays every other job for twenty minutes.
        assert_eq!(settle_for(1, 8, &permanent()), Settlement::Dead);
    }

    #[test]
    fn a_long_message_is_truncated() {
        let long = "x".repeat(ERROR_MAX_CHARS * 2);
        assert_eq!(trim_error(&long).chars().count(), ERROR_MAX_CHARS);
    }

    #[test]
    fn truncation_never_splits_a_character() {
        // The panic this exists to prevent: `&s[..500]` on a string whose 500th byte is
        // mid-character. `€` is U+20AC — THREE bytes — so byte 500 falls inside the 167th
        // character (500 = 3·166 + 2) and a byte-based cut panics.
        //
        // This used to say `ø` while claiming "every character here is three bytes". `ø`
        // is U+00F8, which is TWO, so byte 500 landed on an exact character boundary
        // (500 / 2 = 250) and the byte-slicing implementation this test forbids would not
        // have panicked at all — it would have returned 250 characters and reddened the
        // count assertion instead. The test still failed against a wrong implementation,
        // so it was never an assertion that could not fail; but the specific regression it
        // names went unexercised while its own comment said otherwise.
        //
        // The justification was wrong too, and in a way worth keeping corrected: the doc
        // on `trim_error` used to argue that Indonesian text is where a multi-byte
        // character lands on the boundary. Indonesian is Latin-script and overwhelmingly
        // ASCII. The realistic source is a file name or an emoji echoed by an upstream
        // error.
        let multibyte = "€".repeat(ERROR_MAX_CHARS + 50);
        let trimmed = trim_error(&multibyte);
        assert_eq!(trimmed.chars().count(), ERROR_MAX_CHARS);
    }

    #[test]
    fn a_short_message_survives_intact() {
        assert_eq!(trim_error("storage unreachable"), "storage unreachable");
    }

    #[test]
    fn a_failure_reports_its_own_message() {
        assert_eq!(transient().message(), "storage unreachable");
        assert_eq!(permanent().message(), "not an image");
    }
}
