//! Merging duplicate parts, against a real database.
//!
//! These call the use case directly rather than through HTTP, because no
//! endpoint ships in this slice — AM-88 owns the merge endpoint and needs the
//! admin role from AM-83. A route added only so a test can reach a function is
//! a public surface with no access check, which is exactly what this slice
//! deliberately did not ship.
//!
//! Task 4.3 extends this file with the evidence-count flow. What is here is
//! the part that cannot wait for it: the two properties the schema stopped
//! being able to enforce on its own, and the one property that makes the whole
//! design true.

#![expect(
    clippy::expect_used,
    reason = "test helpers abort rather than propagate"
)]

use std::collections::HashMap;

use anakmobil_runtime::adapter::postgres::build_repo;
use anakmobil_runtime::usecase::part_merge::{self, MergeError};
use sqlx::PgPool;
use uuid::Uuid;

/// No router and no Redis: nothing here goes through HTTP.
macro_rules! pool {
    () => {{
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!("SKIPPED: set DATABASE_URL to run the part merge tests");
            return;
        };
        let Ok(pool) = anakmobil_runtime::adapter::postgres::connect(&database_url) else {
            eprintln!("SKIPPED: DATABASE_URL unusable");
            return;
        };
        if anakmobil_runtime::adapter::postgres::migrate::run(&pool)
            .await
            .is_err()
        {
            eprintln!("SKIPPED: could not migrate the test database");
            return;
        }
        pool
    }};
}

async fn a_curator(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO users (id, email, password_hash) VALUES ($1, $2, 'x')",
        id,
        format!("kurator-{id}@example.com"),
    )
    .execute(pool)
    .await
    .expect("creating a curator");
    id
}

/// A part with a product name nothing else will collide with — `parts_identity`
/// is `NULLS NOT DISTINCT`, so two spec-less parts of the same name are one row.
async fn a_part(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query!(
        r#"
        INSERT INTO parts (id, category, brand, product_name, status)
        VALUES ($1, 'wheels', 'Merek', $2, 'approved')
        "#,
        id,
        format!("Velg {id}"),
    )
    .execute(pool)
    .await
    .expect("creating a part");
    id
}

/// A vehicle owned by `owner` — enough to hang a build off of. Free text,
/// since these tests are not about the catalog.
async fn a_vehicle(pool: &PgPool, owner: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO vehicles (id, owner_id, described_as) VALUES ($1, $2, $3)",
        id,
        owner,
        format!("Test car {id}"),
    )
    .execute(pool)
    .await
    .expect("creating a vehicle");
    id
}

/// A build on that vehicle. Left at the default (private) visibility — every
/// read below passes the vehicle's own owner as the viewer, and
/// `evidence_counts`' filter admits a viewer's own builds regardless.
async fn a_build_on(pool: &PgPool, vehicle_id: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO builds (id, vehicle_id) VALUES ($1, $2)",
        id,
        vehicle_id,
    )
    .execute(pool)
    .await
    .expect("creating a build");
    id
}

/// Fit `part` to `build`, and return the modification's id.
async fn a_modification(pool: &PgPool, build_id: Uuid, part_id: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO modifications (id, build_id, part_id) VALUES ($1, $2, $3)",
        id,
        build_id,
        part_id,
    )
    .execute(pool)
    .await
    .expect("creating a modification");
    id
}

/// Take a modification off the car without deleting its row — the same
/// `removed_at` semantics `usecase::builds::remove_modification` writes.
async fn remove_modification(pool: &PgPool, id: Uuid) {
    sqlx::query!(
        "UPDATE modifications SET removed_at = now() WHERE id = $1",
        id
    )
    .execute(pool)
    .await
    .expect("removing a modification");
}

async fn canonical_of(pool: &PgPool, ids: &[Uuid]) -> HashMap<Uuid, Option<Uuid>> {
    sqlx::query!(
        "SELECT id, canonical_part_id FROM parts WHERE id = ANY($1)",
        ids
    )
    .fetch_all(pool)
    .await
    .expect("reading the canonical cache")
    .into_iter()
    .map(|row| (row.id, row.canonical_part_id))
    .collect()
}

/// What the cache SHOULD say, worked out from the log by a second, deliberately
/// naive implementation.
///
/// Independent on purpose. Calling the production resolver here would assert
/// that it agrees with itself, which is not a property. This one walks each
/// part's chain from scratch with no memo — `O(P × M)` and obviously correct,
/// which is the trade a test wants.
async fn expected_from_the_log(pool: &PgPool, ids: &[Uuid]) -> HashMap<Uuid, Option<Uuid>> {
    let edges: HashMap<Uuid, Uuid> = sqlx::query!(
        "SELECT source_part_id, target_part_id FROM part_merges WHERE undone_at IS NULL"
    )
    .fetch_all(pool)
    .await
    .expect("reading the merge log")
    .into_iter()
    .map(|row| (row.source_part_id, row.target_part_id))
    .collect();

    ids.iter()
        .map(|&start| {
            let mut current = start;
            let mut seen = vec![start];
            while let Some(&next) = edges.get(&current) {
                if seen.contains(&next) {
                    break;
                }
                seen.push(next);
                current = next;
            }
            (start, (current != start).then_some(current))
        })
        .collect()
}

/// The property that makes the whole design true: `parts.canonical_part_id` is
/// a cache, so it must equal what the log says at every point.
///
/// Nothing in the schema enforces this, and divergence between the two is
/// otherwise undetectable — a merge that recomputed the wrong set of rows
/// leaves plausible, wrong counts and no error anywhere.
async fn assert_cache_matches_the_log(pool: &PgPool, ids: &[Uuid], after: &str) {
    let stored = canonical_of(pool, ids).await;
    let expected = expected_from_the_log(pool, ids).await;
    assert_eq!(
        stored, expected,
        "canonical_part_id diverged from the merge log after {after}"
    );
}

#[tokio::test]
async fn an_undo_records_who_did_it() {
    // The CHECK constraint is a one-way implication — `undone_by IS NULL OR
    // undone_at IS NOT NULL` — because the equivalence made account deletion
    // impossible. That was the right fix, and its cost is that an UPDATE
    // setting only `undone_at` is now ACCEPTED by the database. The resulting
    // row is byte-identical to one whose curator later closed their account,
    // so "undone by somebody since departed" and "undone with no actor ever
    // recorded" become indistinguishable. Only the statement and this test
    // stand between those two.
    let pool = pool!();
    let curator = a_curator(&pool).await;
    let (source, target) = (a_part(&pool).await, a_part(&pool).await);

    let merge_id = part_merge::merge(&pool, curator, source, target)
        .await
        .expect("the merge");
    part_merge::unmerge(&pool, curator, merge_id)
        .await
        .expect("the undo");

    let row = sqlx::query!(
        "SELECT undone_by, undone_at FROM part_merges WHERE id = $1",
        merge_id
    )
    .fetch_one(&pool)
    .await
    .expect("reading the merge");

    assert_eq!(
        row.undone_by,
        Some(curator),
        "the undo recorded no actor, and the schema can no longer catch that"
    );
    assert!(row.undone_at.is_some(), "the undo recorded no time");
}

#[tokio::test]
async fn an_undone_merge_survives_the_account_that_undid_it() {
    // The regression test for the constraint 4.1 had to relax. An equivalence
    // between `undone_at` and `undone_by` reads as tidier and makes this
    // DELETE fail: ON DELETE SET NULL clears `undone_by` and the check then
    // rejects the row it just created. Restoring it must turn this red rather
    // than breaking account deletion in production.
    let pool = pool!();
    let curator = a_curator(&pool).await;
    let (source, target) = (a_part(&pool).await, a_part(&pool).await);

    let merge_id = part_merge::merge(&pool, curator, source, target)
        .await
        .expect("the merge");
    part_merge::unmerge(&pool, curator, merge_id)
        .await
        .expect("the undo");

    sqlx::query!("DELETE FROM users WHERE id = $1", curator)
        .execute(&pool)
        .await
        .expect("closing the curator's account must not be blocked by the merge log");

    let row = sqlx::query!(
        "SELECT undone_by, undone_at FROM part_merges WHERE id = $1",
        merge_id
    )
    .fetch_one(&pool)
    .await
    .expect("the merge should still exist");

    assert!(
        row.undone_by.is_none(),
        "the link to the deleted account must be cleared"
    );
    assert!(
        row.undone_at.is_some(),
        "the undo itself is history and must survive the account"
    );
}

#[tokio::test]
async fn the_cache_never_diverges_from_the_log() {
    // The highest-value assertion in this slice. Every operation below is
    // followed by an independent recompute from the log, so a merge that
    // updated the wrong set of rows is caught at the step that did it rather
    // than as a wrong count three tasks later.
    //
    // The chain is four deep on purpose. Undoing B → C in an A → B → C → D
    // chain is the case where every part is flattened onto D, so the merge
    // row's own source and target are ids that nothing points at — passing
    // them to `component_members` raw returns {B, C} and leaves A holding a
    // stale D. Three parts is not enough to produce it.
    let pool = pool!();
    let curator = a_curator(&pool).await;
    let a = a_part(&pool).await;
    let b = a_part(&pool).await;
    let c = a_part(&pool).await;
    let d = a_part(&pool).await;
    let all = [a, b, c, d];

    let a_to_b = part_merge::merge(&pool, curator, a, b)
        .await
        .expect("a → b");
    assert_cache_matches_the_log(&pool, &all, "a → b").await;

    let b_to_c = part_merge::merge(&pool, curator, b, c)
        .await
        .expect("b → c");
    assert_cache_matches_the_log(&pool, &all, "b → c").await;

    part_merge::merge(&pool, curator, c, d)
        .await
        .expect("c → d");
    assert_cache_matches_the_log(&pool, &all, "c → d").await;

    // Everything is flattened onto d — one hop, never a chain. This is what
    // lets `evidence_counts` resolve with a single join.
    let flattened = canonical_of(&pool, &all).await;
    for part in [a, b, c] {
        assert_eq!(
            flattened.get(&part),
            Some(&Some(d)),
            "a chain was left in the cache instead of being flattened"
        );
    }

    part_merge::unmerge(&pool, curator, b_to_c)
        .await
        .expect("undoing b → c");
    assert_cache_matches_the_log(&pool, &all, "undoing b → c from the middle").await;

    // Stated as the outcome rather than only as the property, so a reader can
    // see what "restores the previous state exactly" means here.
    let restored = canonical_of(&pool, &all).await;
    assert_eq!(restored.get(&a), Some(&Some(b)), "a should resolve to b");
    assert_eq!(restored.get(&b), Some(&None), "b should stand alone");
    assert_eq!(
        restored.get(&c),
        Some(&Some(d)),
        "c should still resolve to d"
    );

    part_merge::unmerge(&pool, curator, a_to_b)
        .await
        .expect("undoing a → b");
    assert_cache_matches_the_log(&pool, &all, "undoing a → b").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_curators_merging_the_same_part_do_not_both_succeed() {
    // Two real tasks on two connections, not `tokio::join!` on one task —
    // `join!` interleaves at await points but never has both in flight at the
    // same instant, which is the thing being tested.
    //
    // Exactly one is Ok and the other is AlreadyMerged: never two Oks, and
    // never a Database error, because a unique violation reaching a caller as
    // a 500 is a conflict wearing the wrong status. Without the partial unique
    // index the second silently overwrites the first and neither reports the
    // loss.
    let pool = pool!();
    let curator = a_curator(&pool).await;
    let source = a_part(&pool).await;
    let one_way = a_part(&pool).await;
    let the_other = a_part(&pool).await;

    let first = tokio::spawn({
        let pool = pool.clone();
        async move { part_merge::merge(&pool, curator, source, one_way).await }
    });
    let second = tokio::spawn({
        let pool = pool.clone();
        async move { part_merge::merge(&pool, curator, source, the_other).await }
    });

    let (first, second) = (
        first.await.expect("the first merge task"),
        second.await.expect("the second merge task"),
    );

    let winners = usize::from(first.is_ok()) + usize::from(second.is_ok());
    assert_eq!(
        winners, 1,
        "expected exactly one merge to stand, got first={first:?} second={second:?}"
    );

    let loser = if first.is_err() { first } else { second };
    assert!(
        matches!(loser, Err(MergeError::AlreadyMerged)),
        "the losing merge surfaced as {loser:?} rather than a conflict"
    );

    // And the survivor left a flat, consistent cache behind it.
    assert_cache_matches_the_log(&pool, &[source, one_way, the_other], "a contested merge").await;
}

#[tokio::test]
async fn undoing_twice_is_refused_rather_than_silently_repeated() {
    // A second undo must not recompute from a log it already changed, and must
    // not overwrite who undid it the first time.
    let pool = pool!();
    let curator = a_curator(&pool).await;
    let (source, target) = (a_part(&pool).await, a_part(&pool).await);

    let merge_id = part_merge::merge(&pool, curator, source, target)
        .await
        .expect("the merge");
    part_merge::unmerge(&pool, curator, merge_id)
        .await
        .expect("the undo");

    let again = part_merge::unmerge(&pool, curator, merge_id).await;
    assert!(
        matches!(again, Err(MergeError::AlreadyUndone)),
        "a second undo returned {again:?}"
    );
}

#[tokio::test]
async fn merging_a_part_into_its_own_component_is_refused() {
    // The cycle guard, and the reason `resolve_canonical`'s walk terminates.
    // With a → b standing, merging b → a would close a loop.
    let pool = pool!();
    let curator = a_curator(&pool).await;
    let (a, b) = (a_part(&pool).await, a_part(&pool).await);

    part_merge::merge(&pool, curator, a, b)
        .await
        .expect("a → b");

    let backwards = part_merge::merge(&pool, curator, b, a).await;
    assert!(
        matches!(backwards, Err(MergeError::SamePart)),
        "merging back into the source's own component returned {backwards:?}"
    );
}

#[tokio::test]
async fn a_merge_never_rewrites_a_modification() {
    // Tidak boleh ada penulisan ulang modifications.part_id saat merge. The
    // anti-goal, asserted directly with a raw query against the row a real
    // build would have written: evidence survives a wrong merge only because
    // the original reference is still there.
    let pool = pool!();
    let curator = a_curator(&pool).await;
    let source = a_part(&pool).await;
    let target = a_part(&pool).await;
    let vehicle = a_vehicle(&pool, curator).await;
    let build = a_build_on(&pool, vehicle).await;
    let modification = a_modification(&pool, build, source).await;

    part_merge::merge(&pool, curator, source, target)
        .await
        .expect("the merge");

    let stored_part_id = sqlx::query_scalar!(
        "SELECT part_id FROM modifications WHERE id = $1",
        modification
    )
    .fetch_one(&pool)
    .await
    .expect("reading the modification back");

    assert_eq!(
        stored_part_id, source,
        "the merge rewrote modifications.part_id — a bad merge would now be unrecoverable"
    );
}

#[tokio::test]
async fn merging_combines_the_evidence_rather_than_losing_it() {
    // AC4, stated as the ticket states it. Two builds fitting two duplicate
    // parts; after the merge, a count for the target is 2 — a union of the
    // builds, not a sum of two separate counts, so fitting the same part
    // twice on one car still would not double it (covered in build_list_flow).
    let pool = pool!();
    let curator = a_curator(&pool).await;
    let source = a_part(&pool).await;
    let target = a_part(&pool).await;

    let vehicle_a = a_vehicle(&pool, curator).await;
    let build_a = a_build_on(&pool, vehicle_a).await;
    a_modification(&pool, build_a, source).await;

    let vehicle_b = a_vehicle(&pool, curator).await;
    let build_b = a_build_on(&pool, vehicle_b).await;
    a_modification(&pool, build_b, target).await;

    part_merge::merge(&pool, curator, source, target)
        .await
        .expect("the merge");

    let mut conn = pool.acquire().await.expect("acquiring a connection");
    let counted = build_repo::evidence_counts(&mut conn, &[target], curator, None)
        .await
        .expect("counting")
        .into_iter()
        .next()
        .expect("a row for the target");

    assert_eq!(
        counted.active_build_count, 2,
        "the merged target's count did not combine both builds"
    );
    assert_eq!(
        counted.ever_installed_build_count, 2,
        "the merged target's ever-installed count did not combine both builds"
    );
}

#[tokio::test]
async fn undoing_a_merge_restores_the_counts_exactly() {
    // Not "some earlier value" — the exact ones. Task 4.2 already proved the
    // cache is restored; this proves the number a person actually reads is.
    let pool = pool!();
    let curator = a_curator(&pool).await;
    let source = a_part(&pool).await;
    let target = a_part(&pool).await;

    let vehicle_a = a_vehicle(&pool, curator).await;
    let build_a = a_build_on(&pool, vehicle_a).await;
    a_modification(&pool, build_a, source).await;

    let vehicle_b = a_vehicle(&pool, curator).await;
    let build_b = a_build_on(&pool, vehicle_b).await;
    a_modification(&pool, build_b, target).await;

    let merge_id = part_merge::merge(&pool, curator, source, target)
        .await
        .expect("the merge");

    {
        let mut conn = pool.acquire().await.expect("acquiring a connection");
        let merged = build_repo::evidence_counts(&mut conn, &[target], curator, None)
            .await
            .expect("counting after the merge")
            .into_iter()
            .next()
            .expect("a row for the target");
        assert_eq!(
            merged.active_build_count, 2,
            "the merge did not combine the counts in the first place"
        );
    }

    part_merge::unmerge(&pool, curator, merge_id)
        .await
        .expect("the undo");

    let mut conn = pool.acquire().await.expect("acquiring a connection");
    let restored = build_repo::evidence_counts(&mut conn, &[source, target], curator, None)
        .await
        .expect("counting after the undo");

    let source_count = restored
        .iter()
        .find(|row| row.part_id == source)
        .expect("a row for the source");
    let target_count = restored
        .iter()
        .find(|row| row.part_id == target)
        .expect("a row for the target");

    assert_eq!(
        source_count.active_build_count, 1,
        "the source's count did not return to what it was before the merge"
    );
    assert_eq!(
        target_count.active_build_count, 1,
        "the target's count did not return to what it was before the merge"
    );
}

#[tokio::test]
async fn a_removed_modification_still_counts_after_a_merge() {
    // The two counts and the merge grouping interacting: active drops when a
    // modification is removed, ever-installed does not — and that has to
    // stay true after the removed modification's part is merged away.
    let pool = pool!();
    let curator = a_curator(&pool).await;
    let source = a_part(&pool).await;
    let target = a_part(&pool).await;

    let removed_vehicle = a_vehicle(&pool, curator).await;
    let removed_build = a_build_on(&pool, removed_vehicle).await;
    let removed_modification = a_modification(&pool, removed_build, source).await;
    remove_modification(&pool, removed_modification).await;

    let active_vehicle = a_vehicle(&pool, curator).await;
    let active_build = a_build_on(&pool, active_vehicle).await;
    a_modification(&pool, active_build, target).await;

    part_merge::merge(&pool, curator, source, target)
        .await
        .expect("the merge");

    let mut conn = pool.acquire().await.expect("acquiring a connection");
    let counted = build_repo::evidence_counts(&mut conn, &[target], curator, None)
        .await
        .expect("counting")
        .into_iter()
        .next()
        .expect("a row for the target");

    assert_eq!(
        counted.active_build_count, 1,
        "a removed modification counted as active after its part was merged away"
    );
    assert_eq!(
        counted.ever_installed_build_count, 2,
        "a removed modification stopped counting as ever-installed after its part was merged away"
    );
}
