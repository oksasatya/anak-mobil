//! `usecase::roles::set_role` against a real database.
//!
//! These call the use case directly rather than through HTTP. The endpoint
//! arrives in a later task; the properties below are about a transaction and
//! a lock, and putting a router in front of them would only add ways for the
//! test to be wrong.

#![expect(
    clippy::expect_used,
    reason = "test helpers abort rather than propagate"
)]

use anakmobil_runtime::adapter::http;
use anakmobil_runtime::adapter::postgres::user_repo::{self, PlatformRole};
use anakmobil_runtime::adapter::redis::rate_limit::RateLimiter;
use anakmobil_runtime::adapter::redis::session::SessionStore;
use anakmobil_runtime::platform::state::AppState;
use anakmobil_runtime::usecase::roles::{self, Actor, RoleError};
use sqlx::PgPool;
use uuid::Uuid;

// Same harness as `tests/build_list_flow.rs`, copied rather than shared —
// each integration test file is its own binary, so there is nothing to
// import from. This is the LOUD harness (PR #18): a missing DATABASE_URL /
// REDIS_URL panics unless AM_SKIP_INTEGRATION is set, rather than returning
// silently and reporting green having executed nothing. Do NOT replace this
// with `tests/part_merge_flow.rs`'s `pool!` macro, which still returns
// silently.
macro_rules! app {
    () => {{
        let (Ok(database_url), Ok(redis_url)) =
            (std::env::var("DATABASE_URL"), std::env::var("REDIS_URL"))
        else {
            assert!(
                std::env::var("AM_SKIP_INTEGRATION").is_ok(),
                "DATABASE_URL and REDIS_URL are unset. Run `make be-test`, which loads .env. \
                 To skip the integration suites deliberately, set AM_SKIP_INTEGRATION=1."
            );
            eprintln!("SKIPPED: AM_SKIP_INTEGRATION is set");
            return;
        };
        let Ok(pool) = anakmobil_runtime::adapter::postgres::connect(&database_url) else {
            panic!(
                "DATABASE_URL is set but unusable. The database is part of this \
                 suite, so a green board without it would prove nothing."
            );
        };
        if anakmobil_runtime::adapter::postgres::migrate::run(&pool)
            .await
            .is_err()
        {
            panic!(
                "could not migrate the test database. Is Postgres running? \
                 `make db-up`. A suite that skips here reports green having \
                 executed nothing."
            );
        }
        let Ok(redis) = anakmobil_runtime::adapter::redis::connect(&redis_url).await else {
            panic!(
                "REDIS_URL is set but unreachable. Sessions live in Redis, so \
                 every authenticated test below would be meaningless."
            );
        };
        let app = http::router(AppState {
            pool: pool.clone(),
            redis: redis.clone(),
            sessions: SessionStore::new(redis.clone()),
            limiter: RateLimiter::new(redis),
        });
        (app, pool)
    }};
}

async fn a_user(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO users (id, email, password_hash) VALUES ($1, $2, 'x')",
        id,
        format!("roles-{id}@example.com"),
    )
    .execute(pool)
    .await
    .expect("creating a user");
    id
}

async fn an_admin(pool: &PgPool) -> Uuid {
    let id = a_user(pool).await;
    sqlx::query!("UPDATE users SET platform_role = 'admin' WHERE id = $1", id)
        .execute(pool)
        .await
        .expect("promoting");
    id
}

#[tokio::test]
async fn promoting_writes_the_audit_row_and_the_column_together() {
    let (_app, pool) = app!();
    let actor = an_admin(&pool).await;
    let target = a_user(&pool).await;

    let change = roles::set_role(
        &pool,
        Actor::Admin(actor),
        target,
        PlatformRole::Admin,
        "catalog curation",
    )
    .await
    .expect("the promotion should succeed")
    .expect("a real change returns Some");

    assert_eq!(change.from_role, PlatformRole::User);
    assert_eq!(change.to_role, PlatformRole::Admin);
    assert_eq!(change.target_user_id, target);

    let mut conn = pool.acquire().await.expect("a connection");
    assert_eq!(
        user_repo::platform_role_of(&mut conn, target)
            .await
            .expect("reading the role"),
        Some(PlatformRole::Admin)
    );

    let row = sqlx::query!(
        r#"
        SELECT actor_id,
               from_role AS "from_role: PlatformRole",
               to_role   AS "to_role: PlatformRole",
               reason
        FROM role_changes WHERE target_user_id = $1
        "#,
        target
    )
    .fetch_one(&pool)
    .await
    .expect("exactly one audit row");
    assert_eq!(row.actor_id, Some(actor));
    assert_eq!(row.from_role, PlatformRole::User);
    assert_eq!(row.to_role, PlatformRole::Admin);
    assert_eq!(row.reason, "catalog curation");
}

#[tokio::test]
async fn setting_the_role_somebody_already_has_writes_nothing_and_is_not_an_error() {
    // 204 upstream, and it is what makes a retry safe: a dropped connection
    // after a successful PATCH leaves the client unsure, and retrying lands
    // here. Without this branch the CHECK constraint fires and surfaces as a
    // generic 500 — a correct database rejecting a reasonable client.
    let (_app, pool) = app!();
    let actor = an_admin(&pool).await;
    let target = an_admin(&pool).await;

    let result = roles::set_role(
        &pool,
        Actor::Admin(actor),
        target,
        PlatformRole::Admin,
        "already there",
    )
    .await
    .expect("a no-op is not an error");
    assert!(result.is_none());

    let count = sqlx::query_scalar!(
        "SELECT count(*) FROM role_changes WHERE target_user_id = $1",
        target
    )
    .fetch_one(&pool)
    .await
    .expect("counting");
    assert_eq!(count, Some(0), "a no-op wrote an audit row");
}

#[tokio::test]
async fn an_actor_demoted_since_the_extractor_ran_is_refused_under_the_lock() {
    // The finding a checklist and a second model each found half of. The
    // extractor checked the actor's role before the handler ran; re-reading
    // it inside the transaction is what stops an admin demoted in between
    // from completing the mutation they had already started.
    let (_app, pool) = app!();
    let actor = a_user(&pool).await; // never an admin — the same state as demoted
    let target = a_user(&pool).await;

    let err = roles::set_role(
        &pool,
        Actor::Admin(actor),
        target,
        PlatformRole::Admin,
        "should be refused",
    )
    .await
    .expect_err("a non-admin actor must be refused");
    assert!(matches!(err, RoleError::NotAdmin));

    let count = sqlx::query_scalar!(
        "SELECT count(*) FROM role_changes WHERE target_user_id = $1",
        target
    )
    .fetch_one(&pool)
    .await
    .expect("counting");
    assert_eq!(count, Some(0));
}

#[tokio::test]
async fn a_target_that_does_not_exist_is_not_found() {
    let (_app, pool) = app!();
    let actor = an_admin(&pool).await;

    let err = roles::set_role(
        &pool,
        Actor::Admin(actor),
        Uuid::now_v7(),
        PlatformRole::Admin,
        "nobody",
    )
    .await
    .expect_err("an unknown target");
    assert!(matches!(err, RoleError::NotFound));
}

#[tokio::test]
async fn an_admin_may_demote_themselves_and_the_platform_may_reach_zero_admins() {
    // There is deliberately no last-admin guard. The alternative collapses on
    // contact with the bootstrap rule: if `grant-admin` only succeeds at zero
    // admins and nothing may ever reach zero, `grant-admin` is dead code from
    // the second day and there is no recovery path at all.
    let (_app, pool) = app!();
    let actor = an_admin(&pool).await;

    let change = roles::set_role(
        &pool,
        Actor::Admin(actor),
        actor,
        PlatformRole::User,
        "stepping down",
    )
    .await
    .expect("self-demotion is allowed")
    .expect("a real change");
    assert_eq!(change.from_role, PlatformRole::Admin);
    assert_eq!(change.to_role, PlatformRole::User);
}

#[tokio::test]
async fn a_bootstrap_is_refused_once_the_platform_has_an_admin() {
    let (_app, pool) = app!();
    let _existing = an_admin(&pool).await;
    let target = a_user(&pool).await;

    let err = roles::set_role(
        &pool,
        Actor::Bootstrap,
        target,
        PlatformRole::Admin,
        "should be refused",
    )
    .await
    .expect_err("the platform already has an admin");
    assert!(matches!(err, RoleError::AdminExists));
}

#[tokio::test]
async fn re_granting_the_sole_admin_is_a_safe_no_op_not_a_failure() {
    // The retry case `grant-admin` exists for: a first grant succeeded, the
    // connection dropped, the operator re-runs it. The target IS now the sole
    // admin, so the bootstrap precondition (`admin_count > 0`) is satisfied by
    // the target itself — checking it before the no-op made this return
    // `AdminExists` and exit non-zero, contradicting the command's whole point.
    // It must land on the no-op branch: nothing written, `Ok(None)`.
    let (_app, pool) = app!();
    let admin = an_admin(&pool).await;

    let outcome = roles::set_role(
        &pool,
        Actor::Bootstrap,
        admin,
        PlatformRole::Admin,
        "re-run after a dropped connection",
    )
    .await
    .expect("a re-grant of the existing admin is not an error");
    assert!(
        outcome.is_none(),
        "re-granting the sole admin must be a no-op, not {outcome:?}"
    );
}

#[tokio::test]
async fn an_empty_or_whitespace_reason_is_refused_on_both_entrances() {
    // `reason TEXT NOT NULL` accepts the empty string, which would make the
    // trail useless while looking complete. The guard lives in the use case
    // rather than at the HTTP boundary, because the CLI is a second entrance
    // and a guard on one door of two is the defect AM-361's ledger records.
    let (_app, pool) = app!();
    let actor = an_admin(&pool).await;
    let target = a_user(&pool).await;

    for blank in ["", "   ", "\n\t"] {
        let err = roles::set_role(
            &pool,
            Actor::Admin(actor),
            target,
            PlatformRole::Admin,
            blank,
        )
        .await
        .expect_err("a blank reason");
        assert!(
            matches!(err, RoleError::InvalidReason(_)),
            "accepted {blank:?}"
        );
    }

    let err = roles::set_role(
        &pool,
        Actor::Admin(actor),
        target,
        PlatformRole::Admin,
        &"x".repeat(1_001),
    )
    .await
    .expect_err("an unbounded reason");
    assert!(matches!(err, RoleError::InvalidReason(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_concurrent_promotions_of_one_person_produce_exactly_one_audit_row() {
    // The lock's job: without it both callers read `from_role = user`, both
    // write a row claiming `user → admin`, and one of those rows is a lie
    // about a change that did not happen. It is NOT protecting a last admin —
    // there is no last-admin rule.
    //
    // Two real tasks on two connections, not `tokio::join!` on one task —
    // matching `tests/part_merge_flow.rs`'s
    // `two_curators_merging_the_same_part_do_not_both_succeed`: `join!`
    // interleaves at await points on a single task but never has both futures
    // actually in flight at the same instant, which is the thing being
    // tested. The plan's own draft used `tokio::join!`; this is the
    // established, tested pattern for proving a database-side lock, and the
    // deviation is intentional.
    //
    // Twenty rounds, because a race that fails one time in ten passes a
    // single run and reports itself fixed.
    let (_app, pool) = app!();

    for round in 0..20 {
        let actor_a = an_admin(&pool).await;
        let actor_b = an_admin(&pool).await;
        let target = a_user(&pool).await;

        let first = tokio::spawn({
            let pool = pool.clone();
            async move {
                roles::set_role(
                    &pool,
                    Actor::Admin(actor_a),
                    target,
                    PlatformRole::Admin,
                    "a",
                )
                .await
            }
        });
        let second = tokio::spawn({
            let pool = pool.clone();
            async move {
                roles::set_role(
                    &pool,
                    Actor::Admin(actor_b),
                    target,
                    PlatformRole::Admin,
                    "b",
                )
                .await
            }
        });

        let (first, second) = (
            first.await.expect("the first promotion task"),
            second.await.expect("the second promotion task"),
        );

        let changed = [&first, &second]
            .iter()
            .filter(|r| matches!(r, Ok(Some(_))))
            .count();
        assert_eq!(
            changed, 1,
            "round {round}: {changed} callers claimed the change"
        );
        assert!(
            first.is_ok() && second.is_ok(),
            "round {round}: {first:?} {second:?}"
        );

        let rows = sqlx::query_scalar!(
            "SELECT count(*) FROM role_changes WHERE target_user_id = $1",
            target
        )
        .fetch_one(&pool)
        .await
        .expect("counting");
        assert_eq!(rows, Some(1), "round {round}: the audit trail is not true");
    }
}
