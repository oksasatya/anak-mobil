//! Merging duplicate parts, and putting a merge back.
//!
//! `part_merges` is the truth and `parts.canonical_part_id` is a one-hop cache
//! recomputed from it. Neither operation ever writes `modifications.part_id` —
//! that is what keeps evidence from being destroyed by a bad merge, and it is
//! why undo is possible at all.
//!
//! **No endpoint calls either of these.** The merge endpoint belongs to AM-88
//! and needs the admin role from AM-83, which does not exist yet. A
//! destructive many-row operation with no access check is worse than no
//! operation, so this ships as a use case and its tests. Rust will not warn
//! that the functions are unused, because `runtime` is a library.

use std::collections::HashMap;

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::adapter::postgres::part_repo;

/// Why a merge or an unmerge did not happen.
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    /// The two parts already resolve to the same canonical form. Also the
    /// cycle guard — see [`merge`].
    #[error("kedua part sudah menjadi satu")]
    SamePart,
    /// Somebody else merged this source first.
    #[error("part ini sudah pernah digabungkan")]
    AlreadyMerged,
    /// No such part, or no such merge.
    #[error("data tidak ditemukan")]
    NotFound,
    #[error("penggabungan ini sudah dibatalkan")]
    AlreadyUndone,
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
}

/// Point every count that used `source` at `target` instead.
///
/// `modifications.part_id` is **not** rewritten — deliberately, and it is what
/// keeps evidence from being destroyed by a bad merge. The counts combine
/// because [`evidence_counts`](crate::adapter::postgres::build_repo::evidence_counts)
/// groups by the canonical part, not because the references moved.
///
/// Returns the id of the merge, which is what [`unmerge`] takes.
///
/// # Errors
///
/// [`MergeError::SamePart`] when the two already resolve to the same part,
/// which is also the cycle guard. [`MergeError::AlreadyMerged`] when the
/// source already has a merge that stands. [`MergeError::NotFound`] when
/// either part does not exist.
pub async fn merge(
    pool: &PgPool,
    curator_id: Uuid,
    source: Uuid,
    target: Uuid,
) -> Result<Uuid, MergeError> {
    let mut tx = pool.begin().await?;
    part_repo::lock_merges(&mut tx).await?;

    // EVERY read below is after the lock, and the cycle guard specifically has
    // to be. The partial unique index does NOT help here: `A → B` and `B → A`
    // are different index keys and do not conflict, so two curators racing
    // those two merges would both be admitted and close a loop. Cycle
    // prevention rests entirely on `lock_merges` — see its `ponytail:` note
    // before moving to per-component row locks.
    //
    // Both parts must also exist. A merge into a part that was never created
    // is a curator typing an id, not a 500.
    let source_root = root_of(&mut tx, source).await?;
    let target_root = root_of(&mut tx, target).await?;
    if source_root == target_root {
        // Already one part. Also the cycle guard: an edge back into the
        // source's own component is the only way a loop could be created, and
        // that edge is exactly the one this refuses.
        return Err(MergeError::SamePart);
    }

    let id = Uuid::now_v7();
    part_repo::insert_merge(&mut tx, id, curator_id, source, target)
        .await
        .map_err(already_merged)?;

    recompute(&mut tx, &[source_root, target_root]).await?;

    tx.commit().await?;
    Ok(id)
}

/// Put a merge back, exactly as things were before it.
///
/// # Errors
///
/// [`MergeError::NotFound`] when there is no such merge,
/// [`MergeError::AlreadyUndone`] when it has been undone already.
pub async fn unmerge(pool: &PgPool, curator_id: Uuid, merge_id: Uuid) -> Result<(), MergeError> {
    let mut tx = pool.begin().await?;
    part_repo::lock_merges(&mut tx).await?;

    // Read and guard AFTER the lock, never before it. Two undos of the same
    // merge racing outside the lock would both read `undone_at IS NULL` and
    // both recompute — the second from a log the first had already changed.
    // The `AND undone_at IS NULL` in `mark_merge_undone` is the second layer.
    let row = part_repo::find_merge(&mut tx, merge_id)
        .await?
        .ok_or(MergeError::NotFound)?;
    if row.undone_at.is_some() {
        return Err(MergeError::AlreadyUndone);
    }

    // Resolved BEFORE the undo and resolved to the component ROOT, not used
    // raw. The plan passed `row.source_part_id` and `row.target_part_id`
    // straight through, and that is wrong whenever the merge being undone sits
    // in the middle of a chain: with A → B, B → C and C → D standing, every
    // part is flattened onto D, so undoing B → C would hand `component_members`
    // two ids that nothing points at. It would return {B, C} alone, A would
    // keep a stale `canonical_part_id` of D, and the "undo restores the
    // previous state exactly" claim would be false in the one case the whole
    // append-only design exists for.
    let source_root = root_of(&mut tx, row.source_part_id).await?;
    let target_root = root_of(&mut tx, row.target_part_id).await?;

    part_repo::mark_merge_undone(&mut tx, merge_id, curator_id).await?;
    recompute(&mut tx, &[source_root, target_root]).await?;

    tx.commit().await?;
    Ok(())
}

/// The root of the component this part belongs to.
///
/// One hop, because the cache is kept flat by [`recompute`].
async fn root_of(conn: &mut PgConnection, id: Uuid) -> Result<Uuid, MergeError> {
    let part = part_repo::find(conn, id)
        .await?
        .ok_or(MergeError::NotFound)?;
    Ok(part.canonical_part_id.unwrap_or(part.id))
}

/// Rebuild `canonical_part_id` for everything reachable from these roots.
///
/// Called by both operations, which is the point: one recompute means merge
/// and unmerge cannot disagree about what the cache should say.
///
/// The roots must be component roots — [`part_repo::component_members`]
/// resolves them one hop, which is complete only because this function is what
/// keeps the cache flat.
async fn recompute(tx: &mut PgConnection, roots: &[Uuid]) -> Result<(), MergeError> {
    let members = part_repo::component_members(tx, roots).await?;
    let merges = part_repo::live_merges(tx).await?;

    let resolved = resolve_canonical(&members, &merges);
    let (ids, canonical): (Vec<Uuid>, Vec<Option<Uuid>>) = resolved.into_iter().unzip();

    part_repo::set_canonical(tx, &ids, &canonical).await?;
    Ok(())
}

/// A unique violation on `part_merges_one_live_per_source` is a second curator
/// having merged this part first — a conflict, not an internal failure.
fn already_merged(err: sqlx::Error) -> MergeError {
    match &err {
        sqlx::Error::Database(db) if db.is_unique_violation() => MergeError::AlreadyMerged,
        _ => MergeError::Database(err),
    }
}

/// Where each of these parts resolves to, given the merges that still stand.
///
/// `None` means the part is its own canonical form.
///
/// Recomputed from the log rather than patched one hop at a time. Patching is
/// the mistake the append-only log exists to avoid: flattening `A → C` when
/// `B → C` is applied destroys the `A → B` edge, and undo can then never
/// restore the previous state.
///
/// Complexity: `O(P + M)` time and space — `P` parts, `M` standing merges. The
/// memo is what buys it, and it has to be consulted *during* a walk rather
/// than only at its start: an entry-only memo re-walks every prefix when the
/// parts arrive in the opposite order to the chain, which is `O(M²)`.
fn resolve_canonical(parts: &[Uuid], merges: &[(Uuid, Uuid)]) -> Vec<(Uuid, Option<Uuid>)> {
    let edges: HashMap<Uuid, Uuid> = merges.iter().copied().collect();
    let mut memo: HashMap<Uuid, Uuid> = HashMap::new();

    parts
        .iter()
        .map(|&part| {
            let terminal = terminal_for(part, &edges, &mut memo);
            // Never `Some(part)`: `parts_canonical_not_self` is a CHECK
            // constraint, so a resolver that let a cycle settle on its own
            // start would turn a refused merge into a failed UPDATE at the far
            // end of the transaction.
            (part, (terminal != part).then_some(terminal))
        })
        .collect()
}

/// Walk to the end of the chain, remembering every part passed on the way.
///
/// The step cap and the `path` check are guards for a bug elsewhere, not for
/// expected input: [`merge`] refuses a merge whose target already resolves to
/// the source, so a cycle cannot be created. If that guard is ever wrong, this
/// returns a wrong answer rather than hanging while holding the merge lock.
fn terminal_for(start: Uuid, edges: &HashMap<Uuid, Uuid>, memo: &mut HashMap<Uuid, Uuid>) -> Uuid {
    if let Some(&known) = memo.get(&start) {
        return known;
    }

    let mut path = vec![start];
    let mut current = start;

    let terminal = loop {
        let Some(&next) = edges.get(&current) else {
            break current; // the end of the chain
        };
        if let Some(&known) = memo.get(&next) {
            break known; // splice onto an answer already paid for
        }
        // ponytail: `path.contains` is a linear scan, so the cycle check is
        // O(chain²) in the worst case. A chain is a handful of merges deep in
        // any realistic catalog and the memo means each chain is walked once,
        // so a HashSet would allocate per walk to save nothing. Swap it in if
        // chains ever run to hundreds.
        //
        // Neither half of this condition is dead code, which was measured
        // rather than assumed: removing both makes the cycle tests hang
        // forever, and removing either one alone still terminates with the
        // right answer. They are each other's fallback, and the thing they
        // are protecting is a hung request holding the global merge lock.
        if path.contains(&next) || path.len() > edges.len() {
            break current; // a cycle merge() should have refused
        }
        path.push(next);
        current = next;
    };

    for part in path {
        memo.insert(part, terminal);
    }
    terminal
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Uuid {
        Uuid::from_bytes([byte; 16])
    }

    #[test]
    fn a_part_nobody_merged_is_its_own_canonical_form() {
        let resolved = resolve_canonical(&[id(1)], &[]);
        assert_eq!(resolved, vec![(id(1), None)]);
    }

    #[test]
    fn a_merged_part_points_at_its_target() {
        let resolved = resolve_canonical(&[id(1), id(2)], &[(id(1), id(2))]);
        assert_eq!(resolved, vec![(id(1), Some(id(2))), (id(2), None)]);
    }

    #[test]
    fn a_chain_flattens_to_one_hop() {
        // The failure the whole design exists for. A → B and B → C must leave
        // A pointing at C, not at B: a count for C that resolves one hop would
        // otherwise miss everything under A.
        let resolved = resolve_canonical(&[id(1), id(2), id(3)], &[(id(1), id(2)), (id(2), id(3))]);
        assert_eq!(
            resolved,
            vec![(id(1), Some(id(3))), (id(2), Some(id(3))), (id(3), None)]
        );
    }

    #[test]
    fn undoing_the_second_merge_restores_the_first_exactly() {
        // The log is the truth, so removing one edge and recomputing gives the
        // state before it. Flattening at merge time would have destroyed the
        // A → B edge and made this impossible.
        let resolved = resolve_canonical(&[id(1), id(2), id(3)], &[(id(1), id(2))]);
        assert_eq!(
            resolved,
            vec![(id(1), Some(id(2))), (id(2), None), (id(3), None)]
        );
    }

    #[test]
    fn a_long_chain_is_walked_once_per_edge_not_once_per_part() {
        // Not a timing assertion — a correctness one at a length where a
        // missing memo would still give the right answer slowly, so this test
        // pins the answer and the Big O comment pins the cost.
        let ids: Vec<Uuid> = (1..=50).map(id).collect();
        let merges: Vec<(Uuid, Uuid)> = ids.windows(2).map(|pair| (pair[0], pair[1])).collect();

        let resolved = resolve_canonical(&ids, &merges);
        let last = *ids.last().expect("fifty ids");

        for (part, canonical) in &resolved[..49] {
            assert_eq!(*canonical, Some(last), "{part} did not reach the end");
        }
        assert_eq!(resolved[49], (last, None));
    }

    #[test]
    fn a_long_chain_resolves_the_same_whichever_end_it_is_walked_from() {
        // The memo is only consulted when a walk STARTS on a known part unless
        // it is also consulted mid-walk. Iterating the chain backwards is what
        // separates those two implementations: the entry-only version answers
        // correctly but re-walks every prefix, and the same input in the other
        // order is the cheapest way to notice.
        let ids: Vec<Uuid> = (1..=50).map(id).collect();
        let merges: Vec<(Uuid, Uuid)> = ids.windows(2).map(|pair| (pair[0], pair[1])).collect();

        let backwards: Vec<Uuid> = ids.iter().rev().copied().collect();
        let resolved = resolve_canonical(&backwards, &merges);
        let last = *ids.last().expect("fifty ids");

        assert_eq!(resolved[0], (last, None));
        for (part, canonical) in &resolved[1..] {
            assert_eq!(*canonical, Some(last), "{part} did not reach the end");
        }
    }

    #[test]
    fn a_cycle_terminates_rather_than_hanging() {
        // merge() refuses to create one. This is the guard for a bug in that
        // guard: a wrong answer is recoverable, a hung request holding the
        // merge lock is not.
        let resolved = resolve_canonical(&[id(1), id(2)], &[(id(1), id(2)), (id(2), id(1))]);
        assert_eq!(resolved.len(), 2, "it returned, which is the assertion");
    }

    #[test]
    fn a_cycle_never_points_a_part_at_itself() {
        // parts_canonical_not_self is a CHECK constraint. A resolver that let
        // a cycle settle on its own start would turn a refused merge into a
        // failed UPDATE at the far end of the transaction.
        let resolved = resolve_canonical(&[id(1), id(2)], &[(id(1), id(2)), (id(2), id(1))]);
        for (part, canonical) in resolved {
            assert_ne!(
                canonical,
                Some(part),
                "{part} was made its own canonical form"
            );
        }
    }

    #[test]
    fn two_sources_merged_into_one_target_both_point_at_it() {
        let resolved = resolve_canonical(&[id(1), id(2), id(3)], &[(id(1), id(3)), (id(2), id(3))]);
        assert_eq!(
            resolved,
            vec![(id(1), Some(id(3))), (id(2), Some(id(3))), (id(3), None)]
        );
    }
}
