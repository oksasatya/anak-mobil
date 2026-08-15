//! The session store against a real Redis.
//!
//! The unit tests in `session.rs` check key derivation and cannot touch the
//! Lua, and the Lua is where the interesting failures live: two refreshes
//! racing for one token, a refresh arriving during a logout, a revoke that
//! misses a session. None of that can be asserted against a fake.
//!
//! Needs `REDIS_URL`. CI always supplies one, so these always run there.
//! Locally they skip loudly rather than silently, because a test that quietly
//! does nothing is worse than one that is missing.

use anakmobil_runtime::adapter::redis::session::{Rotation, SessionStore};
use uuid::Uuid;

/// Connect, or skip with a visible reason.
macro_rules! store {
    () => {{
        let Ok(url) = std::env::var("REDIS_URL") else {
            eprintln!("SKIPPED: set REDIS_URL to run the session store tests");
            return;
        };
        match anakmobil_runtime::adapter::redis::connect(&url).await {
            Ok(redis) => SessionStore::new(redis),
            Err(err) => {
                eprintln!("SKIPPED: REDIS_URL is set but unreachable: {err}");
                return;
            }
        }
    }};
}

#[tokio::test]
async fn a_new_session_authenticates() {
    let store = store!();
    let user = Uuid::now_v7();

    let pair = store.create(user).await.expect("creating a session");
    assert_eq!(
        store
            .authenticate(&pair.access)
            .await
            .expect("authenticating"),
        Some(user)
    );
}

#[tokio::test]
async fn an_unknown_token_authenticates_as_nobody() {
    let store = store!();
    assert_eq!(
        store
            .authenticate("not-a-token")
            .await
            .expect("authenticating"),
        None
    );
}

#[tokio::test]
async fn logout_is_immediate() {
    // AC2. The token is not expired; it is revoked, and the difference is the
    // whole reason sessions live in Redis rather than in a JWT.
    let store = store!();
    let user = Uuid::now_v7();

    let pair = store.create(user).await.expect("creating a session");
    assert!(
        store
            .authenticate(&pair.access)
            .await
            .expect("before")
            .is_some()
    );

    store.revoke(user, pair.session_id).await.expect("revoking");
    assert_eq!(store.authenticate(&pair.access).await.expect("after"), None);
}

#[tokio::test]
async fn rotation_invalidates_the_old_refresh() {
    // AC3, first half.
    let store = store!();
    let user = Uuid::now_v7();

    let first = store.create(user).await.expect("creating a session");
    let Rotation::Rotated(second) = store.rotate(&first.refresh).await.expect("rotating") else {
        panic!("the first exchange should rotate");
    };

    assert_eq!(
        second.session_id, first.session_id,
        "rotation stays in the same session"
    );
    assert_ne!(second.access, first.access);
    assert_eq!(
        store.authenticate(&second.access).await.expect("new token"),
        Some(user)
    );
}

#[tokio::test]
async fn replaying_an_exchanged_refresh_is_detected() {
    // AC3, second half. The replay is the theft signal.
    let store = store!();
    let user = Uuid::now_v7();

    let first = store.create(user).await.expect("creating a session");
    let _ = store.rotate(&first.refresh).await.expect("rotating");

    assert_eq!(
        store.rotate(&first.refresh).await.expect("replaying"),
        Rotation::Reused { user_id: user },
        "a second use of an exchanged token must be reported as reuse, not as invalid"
    );
}

#[tokio::test]
async fn two_refreshes_racing_for_one_token_produce_exactly_one_chain() {
    // The finding that forced the Lua script. Read-then-write across separate
    // commands lets both callers consume the same token and mint two live
    // chains for one session — two independent logins from one theft.
    let store = store!();
    let user = Uuid::now_v7();
    let pair = store.create(user).await.expect("creating a session");

    let (a, b) = tokio::join!(store.rotate(&pair.refresh), store.rotate(&pair.refresh));
    let outcomes = [a.expect("first"), b.expect("second")];

    let rotated = outcomes
        .iter()
        .filter(|o| matches!(o, Rotation::Rotated(_)))
        .count();
    assert_eq!(
        rotated, 1,
        "exactly one caller may consume the token: {outcomes:?}"
    );

    let other = outcomes
        .iter()
        .filter(|o| !matches!(o, Rotation::Rotated(_)))
        .count();
    assert_eq!(other, 1, "the loser must not also rotate: {outcomes:?}");
}

#[tokio::test]
async fn a_refresh_cannot_resurrect_a_revoked_session() {
    // Logout deletes the session but not the refresh key, which expires on its
    // own. Without the session check inside the script, this exchange would
    // succeed and hand back working tokens after logout.
    let store = store!();
    let user = Uuid::now_v7();

    let pair = store.create(user).await.expect("creating a session");
    store.revoke(user, pair.session_id).await.expect("revoking");

    assert_eq!(
        store.rotate(&pair.refresh).await.expect("rotating"),
        Rotation::Invalid,
        "a revoked session must not be refreshable"
    );
}

#[tokio::test]
async fn revoke_all_ends_every_device() {
    // AC5, and the response to detected theft.
    let store = store!();
    let user = Uuid::now_v7();

    let phone = store.create(user).await.expect("phone");
    let tablet = store.create(user).await.expect("tablet");
    let laptop = store.create(user).await.expect("laptop");

    let revoked = store.revoke_all(user, false).await.expect("revoking all");
    assert_eq!(revoked, 3);

    for pair in [&phone, &tablet, &laptop] {
        assert_eq!(store.authenticate(&pair.access).await.expect("after"), None);
    }
}

#[tokio::test]
async fn revoke_all_reaches_a_session_added_much_later() {
    // The TTL trap: `SADD` does not extend an existing key's expiry, so a
    // session set carrying its own TTL would expire while later sessions were
    // still alive, and revoke-all would read an empty set and report success
    // while leaving them working.
    let store = store!();
    let user = Uuid::now_v7();

    let early = store.create(user).await.expect("early session");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let late = store.create(user).await.expect("late session");

    assert_eq!(
        store.revoke_all(user, false).await.expect("revoking all"),
        2
    );
    assert_eq!(
        store.authenticate(&early.access).await.expect("early"),
        None
    );
    assert_eq!(store.authenticate(&late.access).await.expect("late"), None);
}

#[tokio::test]
async fn a_fenced_account_cannot_start_a_new_session() {
    // AC5's other half. Revoking existing sessions is not enough if a login
    // racing the deletion can create a fresh one.
    let store = store!();
    let user = Uuid::now_v7();

    store.create(user).await.expect("a session before deletion");
    store
        .revoke_all(user, true)
        .await
        .expect("deleting the account");

    assert!(
        store.create(user).await.is_err(),
        "a deleted account must not log in again"
    );
}

#[tokio::test]
async fn detected_theft_does_not_lock_the_owner_out() {
    // Reuse detection revokes sessions but must NOT fence: the legitimate
    // owner has to be able to log in again with their password. Fencing here
    // would turn a stolen token into a permanent denial of service against
    // the victim.
    let store = store!();
    let user = Uuid::now_v7();

    store.create(user).await.expect("a session");
    store
        .revoke_all(user, false)
        .await
        .expect("responding to theft");

    assert!(
        store.create(user).await.is_ok(),
        "the owner must be able to log in again"
    );
}
