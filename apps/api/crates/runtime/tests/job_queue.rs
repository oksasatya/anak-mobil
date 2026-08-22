//! The job queue against a real Postgres.
//!
//! The unit tests in `usecase/jobs.rs` cover the retry curve and cannot touch the SQL,
//! and the SQL is where the interesting failures live: two workers claiming one job, a
//! lease that expires under a worker that died, an effect key queued twice. None of
//! that can be asserted against a fake, and the compiler checking the query proves only
//! that the columns exist.
//!
//! # Every test in this file runs one at a time, and that is not fastidiousness
//!
//! One of them runs the real worker loop, and the loop claims by availability alone — it
//! cannot filter by kind, because a worker has to take every kind there is. Run
//! concurrently with its neighbours, it would claim their jobs out from under them.
//!
//! `cargo` runs the tests inside one binary on parallel threads, so the serialisation
//! has to come from the file itself: every test takes [`QUEUE`] first. No other test
//! binary in this workspace touches `jobs`, so that lock is the whole fence.
//!
//! Two belts remain buckled anyway, because a development database can hold a row left
//! by an interrupted run: helpers release a job they did not enqueue rather than
//! deleting it, and the loop test's handler answers `Transient` for a kind that is not
//! its own.

#![expect(
    clippy::expect_used,
    reason = "test helpers abort rather than propagate"
)]

use anakmobil_runtime::adapter::postgres::job_repo::{self, NewJob};
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Taken by every test in this file. See the module comment.
static QUEUE: Mutex<()> = Mutex::const_new(());

/// A pool, or a loud failure.
///
/// Same discipline as `tests/garage_flow.rs`: a missing `DATABASE_URL` used to `return`,
/// and cargo reported the test as PASSING, because it captures stderr for passing tests.
/// Skipping is now something you say out loud.
macro_rules! pool {
    () => {{
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            assert!(
                std::env::var("AM_SKIP_INTEGRATION").is_ok(),
                "DATABASE_URL is unset. Run `make be-test`, which loads .env. To skip \
                 the integration suites deliberately, set AM_SKIP_INTEGRATION=1."
            );
            eprintln!("SKIPPED: AM_SKIP_INTEGRATION is set");
            return;
        };
        let Ok(pool) = anakmobil_runtime::adapter::postgres::connect(&database_url) else {
            panic!("DATABASE_URL is set but unusable.");
        };
        if anakmobil_runtime::adapter::postgres::migrate::run(&pool)
            .await
            .is_err()
        {
            panic!("could not migrate the test database. Is Postgres running? `make db-up`.");
        }
        pool
    }};
}

/// A job of a kind nothing handles, tagged so a test can find its own rows.
async fn a_queued_job(pool: &PgPool, kind: &str) -> Uuid {
    let payload = serde_json::json!({ "marker": Uuid::now_v7() });
    let mut conn = pool.acquire().await.expect("a connection");
    job_repo::enqueue(
        &mut conn,
        &NewJob {
            kind,
            payload: &payload,
            effect_key: None,
        },
    )
    .await
    .expect("enqueueing")
    .expect("a fresh job is never a duplicate")
}

/// Hand a job back to the queue without touching its attempt count.
///
/// Used on a job this test did not enqueue — a leftover from an interrupted run. It is
/// released rather than deleted, because deleting somebody else's row to tidy up is how
/// a test suite starts destroying evidence.
async fn release(pool: &PgPool, id: Uuid) {
    let mut conn = pool.acquire().await.expect("a connection");
    sqlx::query!(
        "UPDATE jobs SET state = 'queued', leased_until = NULL, leased_by = NULL \
         WHERE id = $1",
        id
    )
    .execute(&mut *conn)
    .await
    .expect("releasing a job");
}

/// Delete a job this test created.
async fn forget(pool: &PgPool, id: Uuid) {
    let mut conn = pool.acquire().await.expect("a connection");
    sqlx::query!("DELETE FROM jobs WHERE id = $1", id)
        .execute(&mut *conn)
        .await
        .expect("cleaning up");
}

/// Claim until `id` comes up, releasing anything else that appears on the way.
///
/// The claim cannot be told which job to take — that is the whole design — so a test
/// that wants a specific one has to claim its way to it.
/// The failure is reported by `expect` rather than by `panic!`, and that is a
/// constraint rather than a preference. `clippy.toml` allows `unwrap`, `expect`, and
/// `panic` in tests, but the three lints do not agree on what "in a test" means: an
/// integration-test crate is enough for `expect_used`, while `clippy::panic` wants to
/// be inside a `#[test]` function. This is a plain helper, so a `panic!` here is denied
/// while the `expect` calls beside it are not. Fixing the cause rather than allowing
/// the lint is `apps/api/CLAUDE.md`'s own rule.
async fn claim_this(pool: &PgPool, worker: Uuid, id: Uuid) -> job_repo::Job {
    let mut conn = pool.acquire().await.expect("a connection");
    let mut found = None;

    for _ in 0..16 {
        let taken = job_repo::claim(&mut conn, worker, Duration::from_secs(300))
            .await
            .expect("claiming");
        // An empty queue and an exhausted loop are the same failure — the fixture never
        // offered `id` — so both leave `found` as None and are reported together below.
        let Some(job) = taken else { break };
        if job.id == id {
            found = Some(job);
            break;
        }
        drop(conn);
        release(pool, job.id).await;
        conn = pool.acquire().await.expect("a connection");
    }

    assert!(
        found.is_some(),
        "job {id} never came up: the queue emptied, or 16 claims went by"
    );
    found.expect("checked immediately above")
}

/// Claim repeatedly and assert `id` is never handed out, releasing whatever is.
async fn assert_unclaimable(pool: &PgPool, id: Uuid, why: &str) {
    let mut conn = pool.acquire().await.expect("a connection");
    for _ in 0..16 {
        let taken = job_repo::claim(&mut conn, Uuid::now_v7(), Duration::from_secs(300))
            .await
            .expect("claiming");
        let Some(other) = taken else { return };
        assert_ne!(other.id, id, "{why}");
        drop(conn);
        release(pool, other.id).await;
        conn = pool.acquire().await.expect("a connection");
    }
}

#[tokio::test]
async fn four_workers_claim_four_different_jobs() {
    // AC1. The property SKIP LOCKED buys is not "no duplicates" -- plain FOR UPDATE
    // would also produce none, by serialising every worker behind the first. It is that
    // four workers claiming at the same moment get four DIFFERENT jobs and none of them
    // waits.
    //
    // WHAT THIS TEST ACTUALLY CATCHES, corrected after it was measured rather than
    // reasoned about. It catches deleting `FOR UPDATE` outright: two claimers then read
    // the same head id, the outer `WHERE id = $x` re-evaluates to true after the other
    // commits, and both lease it -- the dedupe assertion fires.
    //
    // It does NOT catch deleting `SKIP LOCKED`, and this comment used to claim the
    // opposite. Built without it and measured: the second claimer blocks on the held
    // lock for 1.62 s, and when the first commits, EvalPlanQual re-evaluates the
    // subquery's predicate against the now-`leased` tuple -- `COALESCE(leased_until,
    // run_at) <= now()` is false at `now()+300s` -- excludes it, and takes the NEXT row.
    // So a serialising claim hands out four different jobs and every assertion here
    // still passes. The old comment's stated tell, "claimers returning None", is exactly
    // backwards: being let through is what makes the claimer move on and succeed.
    //
    // The production symptom that therefore escapes this test is throughput of one
    // worker regardless of replica count, each claim waiting a full statement for the one
    // ahead. Pinning it needs a held lock and a `lock_timeout`, not more claimers.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let mut queued = Vec::new();
    for _ in 0..4 {
        queued.push(a_queued_job(&pool, "test.claim").await);
    }

    let mut claiming = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let pool = pool.clone();
        claiming.spawn(async move {
            let mut conn = pool.acquire().await.expect("a connection");
            job_repo::claim(&mut conn, Uuid::now_v7(), Duration::from_secs(300))
                .await
                .expect("claiming")
        });
    }

    let mut claimed: Vec<Uuid> = Vec::new();
    while let Some(result) = claiming.join_next().await {
        if let Some(job) = result.expect("the claim task") {
            claimed.push(job.id);
        }
    }

    let mut unique = claimed.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        claimed.len(),
        unique.len(),
        "two workers claimed the same job: {claimed:?}"
    );
    assert_eq!(
        claimed.len(),
        4,
        "a claimer came back empty on a full queue"
    );

    for id in claimed {
        if !queued.contains(&id) {
            // A leftover from an interrupted run. Give it back.
            release(&pool, id).await;
        }
    }
    for id in queued {
        forget(&pool, id).await;
    }
}

#[tokio::test]
async fn a_live_lease_hides_the_job_from_everyone_else() {
    // AC1's other half, at one job rather than four.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let id = a_queued_job(&pool, "test.lease").await;

    let claimed = claim_this(&pool, Uuid::now_v7(), id).await;
    assert_eq!(claimed.id, id);

    assert_unclaimable(&pool, id, "a leased job was handed out twice").await;
    forget(&pool, id).await;
}

#[tokio::test]
async fn an_expired_lease_hands_the_job_to_the_next_worker() {
    // AC2, and the reason there is no separate reaper: expiry IS the claim predicate.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let id = a_queued_job(&pool, "test.expiry").await;

    // A zero lease expires at the instant it is taken. `claim_this` cannot be used here
    // -- it takes a five-minute lease -- so the first claim is made by hand.
    let mut conn = pool.acquire().await.expect("a connection");
    let first = loop {
        let taken = job_repo::claim(&mut conn, Uuid::now_v7(), Duration::ZERO)
            .await
            .expect("claiming")
            .expect("our job is queued");
        if taken.id == id {
            break taken;
        }
        drop(conn);
        release(&pool, taken.id).await;
        conn = pool.acquire().await.expect("a connection");
    };
    assert_eq!(
        first.attempts, 1,
        "the claim increments the attempt counter"
    );
    drop(conn);

    // The second claim runs in a later transaction, so Postgres's now() has moved past
    // leased_until and the job is due again.
    let again = claim_this(&pool, Uuid::now_v7(), id).await;
    assert_eq!(
        again.attempts, 2,
        "a re-claim after an expired lease burns another attempt"
    );

    forget(&pool, id).await;
}

#[tokio::test]
async fn one_live_job_per_effect_key() {
    // AC4's enqueue half. Without this, `POST /media/{id}/complete` called twice queues
    // two jobs for one media, and "processing twice has the same effect as once" is
    // left resting entirely on the object key -- which spec §10 shows is not enough for
    // an effect that is not a file.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let key = format!("test.effect:{}", Uuid::now_v7());
    let payload = serde_json::json!({});
    let mut conn = pool.acquire().await.expect("a connection");

    let new = NewJob {
        kind: "test.effect",
        payload: &payload,
        effect_key: Some(key.as_str()),
    };
    let first = job_repo::enqueue(&mut conn, &new)
        .await
        .expect("enqueueing")
        .expect("the first is never a duplicate");
    let second = job_repo::enqueue(&mut conn, &new)
        .await
        .expect("enqueueing");
    assert_eq!(second, None, "a live effect key must not queue twice");

    let count = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM jobs WHERE effect_key = $1"#,
        key.as_str()
    )
    .fetch_one(&mut *conn)
    .await
    .expect("counting");
    assert_eq!(count, 1);

    drop(conn);
    forget(&pool, first).await;
}

#[tokio::test]
async fn an_effect_key_is_reusable_once_its_job_is_terminal() {
    // The uniqueness is scoped to the LIVE states on purpose: a re-upload of the same
    // media, or a notification for a second event, must be able to queue again. A
    // constraint over every row would make the first job the last one forever.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let key = format!("test.effect:{}", Uuid::now_v7());
    let payload = serde_json::json!({});
    let mut conn = pool.acquire().await.expect("a connection");

    let new = NewJob {
        kind: "test.effect",
        payload: &payload,
        effect_key: Some(key.as_str()),
    };
    let first = job_repo::enqueue(&mut conn, &new)
        .await
        .expect("enqueueing")
        .expect("the first is never a duplicate");
    sqlx::query!("UPDATE jobs SET state = 'done' WHERE id = $1", first)
        .execute(&mut *conn)
        .await
        .expect("finishing it");

    let second = job_repo::enqueue(&mut conn, &new)
        .await
        .expect("enqueueing")
        .expect("a finished effect key must be reusable, or a re-upload is never processed");

    drop(conn);
    forget(&pool, first).await;
    forget(&pool, second).await;
}

/// What state is this job in? `::text` because the column is a native enum and the test
/// wants a string, not a Rust type mirroring it.
async fn state_of(pool: &PgPool, id: Uuid) -> String {
    let mut conn = pool.acquire().await.expect("a connection");
    sqlx::query_scalar!(
        r#"SELECT state::text AS "state!" FROM jobs WHERE id = $1"#,
        id
    )
    .fetch_one(&mut *conn)
    .await
    .expect("reading the state")
}

/// The three schema properties every other test in this file silently depends on.
///
/// This asserts on the schema itself rather than on behaviour, which is unusual enough to
/// justify. The plan that produced this table named three things as its highest structural
/// risk, and a review then found that **none of them was pinned by anything**: delete the
/// partial predicate from `jobs_claimable_idx`, or weaken `jobs_lease_matches_state` to a
/// one-way implication, or widen `jobs_one_live_per_effect` past the live states, and every
/// behavioural test here still passes. The queue keeps working and quietly degrades with
/// total history instead of with pending work — which is precisely the failure the plan
/// called "a rewrite if found late".
///
/// Behaviour cannot catch these because each wrong version is still *correct*, just
/// slower or laxer, on any table small enough to test against. So the assertion has to be
/// on the definition.
#[tokio::test]
async fn the_schema_keeps_the_three_properties_everything_else_assumes() {
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let mut conn = pool.acquire().await.expect("a connection");

    let claimable = sqlx::query_scalar!(
        r#"SELECT pg_get_indexdef(c.oid) AS "def!"
             FROM pg_class c WHERE c.relname = 'jobs_claimable_idx'"#
    )
    .fetch_one(&mut *conn)
    .await
    .expect("jobs_claimable_idx must exist");

    // Ordered by the COALESCE expression, not by run_at: the claim reads it for both its
    // WHERE and its ORDER BY, and an index on run_at alone cannot serve the ordering, so
    // LIMIT 1 stops being "take the first leaf" and becomes "sort everything first".
    assert!(
        claimable.contains("COALESCE(leased_until, run_at)"),
        "the claim index must be on the COALESCE expression, got: {claimable}"
    );
    // Partial: terminal rows must not be in it at all, or the hot path slows down with
    // the platform's whole history rather than with the backlog.
    assert!(
        claimable.contains("WHERE")
            && claimable.contains("'queued'")
            && claimable.contains("'leased'"),
        "the claim index must be partial over the live states, got: {claimable}"
    );

    let lease = sqlx::query_scalar!(
        r#"SELECT pg_get_constraintdef(oid) AS "def!"
             FROM pg_constraint WHERE conname = 'jobs_lease_matches_state'"#
    )
    .fetch_one(&mut *conn)
    .await
    .expect("jobs_lease_matches_state must exist");

    // An equivalence, not an implication. The claim predicate's meaning depends on a
    // leased row ALWAYS carrying a lease and a non-leased row NEVER carrying one; a
    // one-way version permits a queued row with a stale leased_until.
    assert!(
        lease.contains("= (leased_until IS NOT NULL)"),
        "the lease constraint must be an equivalence, got: {lease}"
    );

    let effect = sqlx::query_scalar!(
        r#"SELECT pg_get_indexdef(c.oid) AS "def!"
             FROM pg_class c WHERE c.relname = 'jobs_one_live_per_effect'"#
    )
    .fetch_one(&mut *conn)
    .await
    .expect("jobs_one_live_per_effect must exist");

    // Scoped to the live states only. Unconditional, a terminal job's effect key would be
    // unusable forever — and the symptom would surface in AM-359 as a photo that can never
    // be re-uploaded, not here.
    assert!(
        effect.contains("UNIQUE") && effect.contains("'queued'") && effect.contains("'leased'"),
        "the effect-key index must be unique over the live states only, got: {effect}"
    );
}

#[tokio::test]
async fn a_transient_failure_comes_back_and_a_delayed_one_does_not() {
    // AC3's first half: the delay is real. A reschedule with no delay is claimable at
    // once; a reschedule with a minute on it is not.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let worker = Uuid::now_v7();
    let id = a_queued_job(&pool, "test.retry").await;
    let job = claim_this(&pool, worker, id).await;
    assert_eq!(job.attempts, 1);

    let mut conn = pool.acquire().await.expect("a connection");
    let settled = job_repo::reschedule(&mut conn, id, worker, Duration::ZERO, "storage down")
        .await
        .expect("rescheduling");
    drop(conn);
    assert!(
        settled,
        "the lease holder must be able to settle its own job"
    );
    assert_eq!(state_of(&pool, id).await, "queued");

    let again = claim_this(&pool, worker, id).await;
    assert_eq!(again.attempts, 2, "a retry burns another attempt");

    // Now push it into the future and confirm it hides.
    let mut conn = pool.acquire().await.expect("a connection");
    let delayed = job_repo::reschedule(
        &mut conn,
        id,
        worker,
        Duration::from_secs(60),
        "storage down",
    )
    .await
    .expect("rescheduling");
    drop(conn);
    // Without this, a call that matched zero rows (a units bug, a mismatched predicate)
    // and a call that genuinely delayed the job would look identical below: the job
    // would still read `leased` from `claim_this`'s five-minute lease either way, and
    // `assert_unclaimable` would pass for the wrong reason.
    assert!(
        delayed,
        "the lease holder must be able to delay its own job"
    );
    assert_eq!(state_of(&pool, id).await, "queued");
    assert_unclaimable(
        &pool,
        id,
        "a job with a future run_at must not be claimable",
    )
    .await;

    forget(&pool, id).await;
}

#[tokio::test]
async fn a_lost_lease_cannot_be_settled_by_the_worker_that_lost_it() {
    // The race the `leased_by` predicate closes. A stalls past its lease, B takes the
    // job, A wakes and tries to finish it. A must fail, not terminate B's work — on ALL
    // THREE settle paths. `mark_done` alone left `reschedule` and `mark_dead` unpinned:
    // deleting `AND leased_by = $2` from either would still show nothing red here, and
    // `reschedule`'s failure mode is the worse of the two -- it would hand B's still-
    // running job to a THIRD worker.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let stalled = Uuid::now_v7();
    let id = a_queued_job(&pool, "test.lostlease").await;
    claim_this(&pool, stalled, id).await;

    let mut conn = pool.acquire().await.expect("a connection");
    // B takes it over -- what an expired lease and a fresh claim would have produced.
    sqlx::query!(
        "UPDATE jobs SET leased_by = $2 WHERE id = $1",
        id,
        Uuid::now_v7()
    )
    .execute(&mut *conn)
    .await
    .expect("handing the lease over");

    let settled = job_repo::mark_done(&mut conn, id, stalled)
        .await
        .expect("marking done");
    assert!(
        !settled,
        "a worker that lost its lease must not be able to finish the job"
    );

    let rescheduled = job_repo::reschedule(&mut conn, id, stalled, Duration::ZERO, "late")
        .await
        .expect("rescheduling");
    assert!(
        !rescheduled,
        "a worker that lost its lease must not be able to return B's still-running job \
         to the queue for a third worker to pick up"
    );

    let dead_lettered = job_repo::mark_dead(&mut conn, id, stalled, "late")
        .await
        .expect("dead-lettering");
    assert!(
        !dead_lettered,
        "a worker that lost its lease must not be able to kill the job B is still running"
    );
    drop(conn);
    assert_eq!(state_of(&pool, id).await, "leased");

    forget(&pool, id).await;
}

#[tokio::test]
async fn a_dead_lettered_job_is_never_claimed_again() {
    // AC3's terminal half. `dead` is outside the claim index's predicate, so this is
    // structural rather than a filter somebody remembered to write.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let worker = Uuid::now_v7();
    let id = a_queued_job(&pool, "test.dead").await;
    claim_this(&pool, worker, id).await;

    let mut conn = pool.acquire().await.expect("a connection");
    let settled = job_repo::mark_dead(&mut conn, id, worker, "not an image")
        .await
        .expect("dead-lettering");
    assert!(settled);
    let error = sqlx::query_scalar!("SELECT last_error FROM jobs WHERE id = $1", id)
        .fetch_one(&mut *conn)
        .await
        .expect("reading the error");
    drop(conn);

    assert_eq!(state_of(&pool, id).await, "dead");
    assert_eq!(error.as_deref(), Some("not an image"));

    // Pins `AND state = 'leased'` specifically. `mark_dead` already nulled `leased_by`,
    // so without this a dropped state clause is invisible: every settle also clears
    // `leased_by`, so `leased_by = $2` alone happens to suffice today (ledger F15).
    // Nothing in the schema forbids a non-NULL `leased_by` on a terminal row (F4), so
    // restoring it here isolates the state clause -- if it were dropped, this second
    // `mark_dead` would still match on `leased_by` alone and return `true`.
    let mut conn = pool.acquire().await.expect("a connection");
    sqlx::query!("UPDATE jobs SET leased_by = $2 WHERE id = $1", id, worker)
        .execute(&mut *conn)
        .await
        .expect("simulating a stale leased_by on a terminal row");
    let restale = job_repo::mark_dead(&mut conn, id, worker, "again")
        .await
        .expect("dead-lettering again");
    drop(conn);
    assert!(
        !restale,
        "state = 'leased' must gate the settle -- a matching leased_by is not enough on \
         its own"
    );

    assert_unclaimable(&pool, id, "a dead job was claimed again").await;

    forget(&pool, id).await;
}

#[tokio::test]
async fn a_control_character_in_the_failure_message_still_reaches_dead() {
    // F14, escalating F5. Before `trim_error` sanitised control characters, a NUL byte
    // in a settle message killed the very statement that records the failure: Postgres
    // `text` cannot hold `U+0000`, `UPDATE ... SET last_error = $n` was rejected, the
    // row stayed `leased`, and -- because `mark_dead` is the ONLY transition into `dead`
    // and `settle_for` routes every `Permanent` failure there regardless of `attempts`
    // -- the job cycled forever on lease expiry with no dead-letter row for
    // `queue-stats` to count. Drives the real loop, not `job_repo::mark_dead` directly,
    // so it pins the sanitisation actually happening on the path a handler uses. The
    // realistic source is a decoder or `from_utf8_lossy` echoing bytes of a malformed
    // upload -- AM-359's own territory.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let id = a_queued_job(&pool, "test.poison").await;

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let stop = std::sync::Mutex::new(Some(stop));

    anakmobil_runtime::usecase::jobs::run(
        &pool,
        |job| {
            let stop = &stop;
            async move {
                if job.kind != "test.poison" {
                    // A leftover from an interrupted run. See the loop-draining test.
                    return Err(anakmobil_runtime::usecase::jobs::JobFailure::Transient(
                        "not this test's job".to_owned(),
                    ));
                }
                if let Some(sender) = stop.lock().expect("the stop channel").take() {
                    let _ = sender.send(());
                }
                Err(anakmobil_runtime::usecase::jobs::JobFailure::Permanent(
                    "malformed byte right \0 there".to_owned(),
                ))
            }
        },
        async {
            let _ = stopped.await;
        },
    )
    .await;

    assert_eq!(
        state_of(&pool, id).await,
        "dead",
        "an unsanitised NUL must not leave the job stuck `leased` forever"
    );
    let mut conn = pool.acquire().await.expect("a connection");
    let error = sqlx::query_scalar!("SELECT last_error FROM jobs WHERE id = $1", id)
        .fetch_one(&mut *conn)
        .await
        .expect("reading the error");
    drop(conn);
    assert!(
        !error.unwrap_or_default().contains('\0'),
        "the control character must be sanitised before it reaches the row"
    );

    forget(&pool, id).await;
}

#[tokio::test]
async fn the_loop_drains_what_it_is_given_and_stops_when_told() {
    // The loop's whole contract in one test: it claims, it runs the handler, it settles
    // the result, and it stops on the signal BETWEEN jobs rather than abandoning one
    // mid-flight. The third job's handler fires the signal and then yields once, so if
    // `run_one` ever wrapped `handle(job)` directly in a `select!` against shutdown —
    // the exact thing AC1 forbids — the still-suspended handler would be dropped right
    // there, this job would never reach `settle`, and the assertion below would find it
    // still `leased` instead of `done`. Without the yield the handler never suspends and
    // a `select!` around it has nothing to cancel, so that regression would pass green.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let mine: Vec<Uuid> = {
        let mut ids = Vec::new();
        for _ in 0..3 {
            ids.push(a_queued_job(&pool, "test.loop").await);
        }
        ids
    };

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let stop = std::sync::Mutex::new(Some(stop));
    let handled = std::sync::Mutex::new(Vec::<Uuid>::new());

    anakmobil_runtime::usecase::jobs::run(
        &pool,
        |job| {
            // These two rebindings are load-bearing, not noise. Without them the
            // `async move` block below would move `handled` and `stop` out of the
            // closure's environment, making the closure `FnOnce` — and `run` needs
            // `Fn`. Taking a reference first means the block moves a `&`, which is Copy.
            let handled = &handled;
            let stop = &stop;
            async move {
                if job.kind != "test.loop" {
                    // A leftover from an interrupted run. Transient, so it goes back to
                    // the queue rather than being finished or killed on somebody else's
                    // behalf. QUEUE already stops the concurrent case.
                    return Err(anakmobil_runtime::usecase::jobs::JobFailure::Transient(
                        "not this test's job".to_owned(),
                    ));
                }
                let count = {
                    let mut seen = handled.lock().expect("the handled list");
                    seen.push(job.id);
                    seen.len()
                };
                if count == 3 {
                    // Scoped so the `MutexGuard` drops before the `.await` below --
                    // `std::sync::Mutex` held across an await is `await_holding_lock`.
                    let sender = stop.lock().expect("the stop channel").take();
                    if let Some(sender) = sender {
                        let _ = sender.send(());
                    }
                    // The one suspension point this handler has. Without it the future
                    // completes on its first poll and can never be cancelled, so a
                    // `select!` around `handle(job)` would have nothing to catch it with.
                    tokio::task::yield_now().await;
                }
                Ok(())
            }
        },
        async {
            let _ = stopped.await;
        },
    )
    .await;

    let seen = handled.into_inner().expect("the handled list");
    assert_eq!(seen.len(), 3, "the loop must drain what it was given");
    for id in &mine {
        assert!(seen.contains(id), "job {id} was never handled");
        assert_eq!(
            state_of(&pool, *id).await,
            "done",
            "the loop must settle a job it ran, even the one that triggered shutdown"
        );
    }

    for id in mine {
        forget(&pool, id).await;
    }
}

#[tokio::test]
async fn a_leased_job_still_counts_as_pending_and_a_dead_one_is_counted() {
    // AC5, and the one thing about it that is easy to get wrong. Read as
    // `state = 'queued'` alone, the age would fall to zero exactly when a wedged worker
    // is holding every job leased -- the situation the number exists to reveal. So the
    // job this test leaves pending is LEASED, and nothing is queued at all.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let worker = Uuid::now_v7();
    let mut conn = pool.acquire().await.expect("a connection");

    let pending = sqlx::query_scalar!(
        r#"SELECT count(*) AS "pending!" FROM jobs WHERE state IN ('queued', 'leased')"#
    )
    .fetch_one(&mut *conn)
    .await
    .expect("counting");
    assert_eq!(
        pending, 0,
        "an interrupted run left work in the queue; clear it with \
         `DELETE FROM jobs WHERE kind LIKE 'test.%'` and run again"
    );

    let before = job_repo::stats(&mut conn).await.expect("reading the stats");
    assert_eq!(
        before.oldest_pending_age_seconds, None,
        "nothing is pending yet"
    );
    drop(conn);

    // One job, claimed and left leased. Nothing is in `queued`.
    let leased = a_queued_job(&pool, "test.stats").await;
    claim_this(&pool, worker, leased).await;

    // A second job, dead-lettered.
    let doomed = a_queued_job(&pool, "test.stats").await;
    claim_this(&pool, worker, doomed).await;
    let mut conn = pool.acquire().await.expect("a connection");
    job_repo::mark_dead(&mut conn, doomed, worker, "poison")
        .await
        .expect("dead-lettering");

    let after = job_repo::stats(&mut conn).await.expect("reading the stats");
    drop(conn);

    // `is_some()` alone passes for any non-NULL number, so it survives swapping
    // `min(created_at)` for `min(run_at)` -- a plausible edit, since the claim
    // predicate next door is built on `COALESCE(leased_until, run_at)`. With this queue
    // mid-backoff, `min(run_at)` would be in the future and the age would be NEGATIVE:
    // an operator would read a healthy queue while work piles up, which is the exact
    // failure this metric exists to catch. A generous bound is loud on either mutation
    // (`state = 'queued'` alone reads zero pending; a future `run_at` reads negative).
    let age = after.oldest_pending_age_seconds.expect(
        "a leased job is still owed work; reading `pending` as `queued` only \
                 would report a healthy queue while a wedged worker holds everything",
    );
    assert!(
        (0.0..300.0).contains(&age),
        "oldest pending age {age} is out of the sane bound"
    );
    assert_eq!(
        after.dead,
        before.dead + 1,
        "a dead-lettered job must show up in the dead count"
    );

    forget(&pool, leased).await;
    forget(&pool, doomed).await;
}
