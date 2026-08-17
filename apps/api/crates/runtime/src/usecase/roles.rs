//! Changing a platform role, from either entrance.
//!
//! Two write paths, one use case, one transaction:
//!
//! ```text
//! anakmobil grant-admin <email>      → actor_id NULL, only when the admin count is zero
//! PATCH /admin/users/{id}/role       → actor_id is the calling admin
//!                     ↓ both
//!         usecase::roles::set_role()
//! ```
//!
//! A guard that lives on one of two entrances is a guard on neither, which is
//! the finding AM-361's ledger records against the parts queue. So the reason
//! check, the lock, the re-reads, and the audit write are all here, and both
//! callers are thin.

use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::adapter::postgres::user_repo::{self, PlatformRole, RoleChangeRow};

/// The longest reason worth storing.
///
/// Bounded because `role_changes` is append-only: nothing can ever trim a row
/// that turned out to hold a paragraph, or a paste. A thousand characters is
/// far more than a sentence explaining a promotion and far less than a way to
/// use an audit table as storage.
const MAX_REASON: usize = 1_000;

/// Who is asking, and what that entitles them to.
#[derive(Debug, Clone, Copy)]
pub enum Actor {
    /// A signed-in admin. Their role is re-read under the lock — the extractor
    /// checked it before the handler ran, and an admin demoted in between
    /// would otherwise still complete the mutation they had already started.
    Admin(Uuid),
    /// The operational command. There is no actor to re-read; its own
    /// precondition is that the platform has no admin yet, checked under the
    /// lock but only once a real change would happen — so re-running it for an
    /// address that is already an admin is a safe no-op, not a failure.
    Bootstrap,
}

/// What changed. Deliberately carries no email and no reason — it answers what
/// changed, not who anybody is.
#[derive(Debug, Clone, Copy)]
pub struct RoleChange {
    pub target_user_id: Uuid,
    pub from_role: PlatformRole,
    pub to_role: PlatformRole,
    pub created_at: OffsetDateTime,
}

/// Why a role change did not happen.
#[derive(Debug, thiserror::Error)]
pub enum RoleError {
    #[error("no such account")]
    NotFound,
    /// The caller is not an admin — either they never were, or they were
    /// demoted between the extractor and this transaction. The same answer
    /// either way, because they are the same situation.
    #[error("the caller is not a platform admin")]
    NotAdmin,
    /// The bootstrap precondition failed: somebody is already an admin, so the
    /// operational command is not the way in.
    #[error("the platform already has an admin")]
    AdminExists,
    /// The message names the problem in Bahasa Indonesia and reaches the
    /// client, so it carries only what the caller supplied.
    #[error("{0}")]
    InvalidReason(String),
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
}

/// Change somebody's platform role, or report that there was nothing to change.
///
/// `Ok(None)` means the account already had that role: nothing was written,
/// and the caller answers `204`. That branch is what makes a retry safe — a
/// dropped connection after a successful call leaves the client unsure, and
/// retrying lands here rather than on the `CHECK` constraint, which would
/// surface as a generic 500.
///
/// # The order inside the transaction is the design
///
/// 1. take the platform-role advisory lock
/// 2. re-read the actor's role (HTTP only) — a demoted admin is refused here
/// 3. re-read the target's role; a no-op returns `Ok(None)` for either entrance
/// 4. only then, and only on a real change, the bootstrap precondition (CLI)
/// 5. insert the audit row
/// 6. update the column
///
/// Step 4 sits after step 3 deliberately: checking the admin count before the
/// no-op made `Ok(None)` unreachable from `grant-admin`, because the target
/// counts toward its own total the instant it is promoted.
///
/// The lock's job is to make the audit row **true**. Without it, two admins
/// promoting the same person concurrently both read `from_role = user`, both
/// write a row claiming `user → admin`, and one of those rows is a lie about a
/// change that did not happen. It is not protecting a last admin — there is no
/// last-admin rule, and zero admins is a legitimate state that `grant-admin`
/// exists to recover from.
///
/// The audit insert precedes the update and its failure fails the change: a
/// privilege that exists with no record of how it was granted is worse than a
/// privilege that failed to be granted.
///
/// Complexity: two primary-key lookups (`O(log U)`) on the HTTP path, or one
/// lookup plus an `O(U)` admin count on the bootstrap path, then two
/// single-row writes.
///
/// # Errors
///
/// [`RoleError::InvalidReason`] for a blank or over-long reason,
/// [`RoleError::NotAdmin`] when the actor is not an admin,
/// [`RoleError::AdminExists`] when a bootstrap runs on a platform that already
/// has one, [`RoleError::NotFound`] when the target does not exist.
pub async fn set_role(
    pool: &PgPool,
    actor: Actor,
    target_user_id: Uuid,
    to_role: PlatformRole,
    reason: &str,
) -> Result<Option<RoleChange>, RoleError> {
    let reason = check_reason(reason)?;

    // A transaction, not a pooled connection: `pg_advisory_xact_lock` releases
    // at the end of its transaction, and on an autocommit connection every
    // statement IS its own transaction — so the lock would be gone before the
    // first read. That exact bug shipped once in `usecase::parts::suggest`.
    let mut tx = pool.begin().await?;
    user_repo::lock_platform_role(&mut tx).await?;

    // Authorization stays first: an admin demoted between the extractor and
    // this lock is refused here, before anything else. The bootstrap precondition
    // is NOT checked here — see below.
    let actor_id = match actor {
        Actor::Admin(id) => {
            if user_repo::platform_role_of(&mut tx, id).await? != Some(PlatformRole::Admin) {
                return Err(RoleError::NotAdmin);
            }
            Some(id)
        }
        Actor::Bootstrap => None,
    };

    let from_role = user_repo::platform_role_of(&mut tx, target_user_id)
        .await?
        .ok_or(RoleError::NotFound)?;

    if from_role == to_role {
        // Nothing written. The transaction is dropped without a commit, which
        // rolls it back and releases the lock. Both entrances land here,
        // including `grant-admin <email>` re-run for an address that is already
        // an admin — a safe retry, not a failure.
        return Ok(None);
    }

    // The bootstrap precondition is checked only once a real change would
    // happen, and under the lock. Checking it in the actor match above made this
    // branch unreachable from bootstrap: the target contributes to its own
    // count, so the first successful grant made every later `grant-admin` for
    // that same address return `AdminExists` instead of the retry-safe no-op.
    // Counting and then inserting is still one transaction, so it is not
    // check-then-act — two operators running the command at once cannot both see
    // zero, because the lock serialises them.
    if matches!(actor, Actor::Bootstrap) && user_repo::admin_count(&mut tx).await? > 0 {
        return Err(RoleError::AdminExists);
    }

    let created_at = user_repo::insert_role_change(
        &mut tx,
        RoleChangeRow {
            id: Uuid::now_v7(),
            actor_id,
            target_user_id,
            from_role,
            to_role,
            reason: &reason,
        },
    )
    .await?;

    user_repo::set_platform_role(&mut tx, target_user_id, to_role).await?;
    tx.commit().await?;

    // Ids and roles only. Never the reason, never an email — the repository
    // rule is method, route, status, latency, request id, and a user id is not
    // a credential. The AM-361 fix pass had to remove a caller-supplied value
    // from a log line for exactly this reason.
    tracing::info!(
        %target_user_id,
        actor_id = ?actor_id,
        from_role = ?from_role,
        to_role = ?to_role,
        "platform role changed"
    );

    Ok(Some(RoleChange {
        target_user_id,
        from_role,
        to_role,
        created_at,
    }))
}

/// Trim the reason and refuse the two shapes an append-only column cannot
/// recover from: nothing at all, and more than anybody meant to type.
///
/// Here rather than at the HTTP boundary, because there are two entrances and
/// the CLI is not one of them. Messages are Bahasa Indonesia — they are
/// product text, and the repository rule puts product text in Indonesian.
fn check_reason(reason: &str) -> Result<String, RoleError> {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        return Err(RoleError::InvalidReason("Alasan wajib diisi.".to_owned()));
    }
    if trimmed.chars().count() > MAX_REASON {
        return Err(RoleError::InvalidReason(format!(
            "Alasan maksimal {MAX_REASON} karakter."
        )));
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_reason_is_refused() {
        for blank in ["", "   ", "\n\t "] {
            assert!(matches!(
                check_reason(blank),
                Err(RoleError::InvalidReason(_))
            ));
        }
    }

    #[test]
    fn a_reason_is_stored_trimmed() {
        assert_eq!(
            check_reason("  kurasi katalog  ").expect("accepted"),
            "kurasi katalog"
        );
    }

    #[test]
    fn the_length_bound_is_counted_in_characters_not_bytes() {
        // A thousand emoji is a thousand characters and four thousand bytes.
        // Counting bytes would refuse a reason written in a script this
        // platform's users actually type.
        assert!(check_reason(&"é".repeat(MAX_REASON)).is_ok());
        assert!(check_reason(&"é".repeat(MAX_REASON + 1)).is_err());
    }
}
