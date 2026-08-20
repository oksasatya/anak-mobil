//! Sessions in Redis.
//!
//! # Why Redis and not a JWT
//!
//! AC2 requires a token to stop working the moment someone logs out. A
//! stateless JWT cannot do that — once signed it is valid until it expires,
//! and "logout" becomes a client-side gesture. So both tokens are opaque
//! random strings and every authenticated request costs one Redis round trip.
//!
//! # The key layout
//!
//! ```text
//! sess:{session_id}           -> user_id      TTL: refresh lifetime, slides on rotation
//! at:{digest(access)}         -> session_id   TTL: min(access lifetime, session TTL)
//! rt:{digest(refresh)}        HASH {sid, uid} TTL: session TTL
//! rt:used:{digest(refresh)}   HASH {sid, uid} TTL: session TTL   — the theft detector
//! user:{user_id}:sessions     SET of sid      no TTL
//! user:{user_id}:fenced       -> "1"          TTL: refresh lifetime
//! ```
//!
//! **`sess:` is the only authority.** Authenticating requires both the token
//! mapping *and* the session to exist, so revoking is a single `DEL` and the
//! orphaned token keys simply expire. Storing token hashes inside the session
//! so they could be deleted too would be more state to keep consistent for no
//! gain.
//!
//! **Redis stores digests, never tokens.** A dump, a stray `KEYS *`, or a
//! misconfigured replica yields values that cannot be presented as
//! credentials.
//!
//! **The user's session set carries no TTL.** `SADD` does not extend an
//! existing key's expiry, so a set created at first login would expire while
//! sessions added on day 89 were still alive — and revoke-all would then read
//! an empty set and silently miss them. It is removed when the last session
//! is revoked instead.
//!
//! # Atomicity
//!
//! Rotation and session creation are Lua scripts. Read-then-write across
//! several commands is not safe here: two refreshes arriving together could
//! both consume the same token and mint two live chains for one session, and
//! a refresh in flight during a logout could recreate the session it just
//! deleted. Redis runs a script to completion without interleaving, which is
//! exactly the guarantee those two need.
//!
//! The scripts compute some keys from values they read, which is safe on a
//! single Redis instance and would not be under Redis Cluster. This deployment
//! is a single instance; a move to Cluster would have to revisit it.

use std::time::Duration;

use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Script};
use uuid::Uuid;

use crate::shared::security::token_digest;

/// How long an access token lives.
///
/// One hour rather than fifteen minutes: revocation is already instant, so a
/// short access token is a second layer rather than the defence itself, and a
/// shorter one only makes the client refresh more often — which is the path
/// most likely to race.
pub const ACCESS_TTL: Duration = Duration::from_secs(60 * 60);

/// How long a refresh token, and therefore a session, lives.
pub const REFRESH_TTL: Duration = Duration::from_secs(60 * 60 * 24 * 90);

/// A newly minted pair. The raw tokens exist here and nowhere else — Redis
/// only ever sees their digests, and this struct is never logged.
#[derive(Debug, Clone)]
pub struct TokenPair {
    pub access: String,
    pub refresh: String,
    pub session_id: Uuid,
}

/// What happened when a refresh token was presented.
#[derive(Debug, PartialEq, Eq)]
pub enum Rotation {
    /// Exchanged. The old token is now invalid.
    Rotated(Box<TokenPair>),
    /// Already exchanged once. This is a stolen token being replayed, and the
    /// caller revokes every session the account has.
    Reused { user_id: Uuid },
    /// Never existed, or expired, or its session is gone.
    Invalid,
}

impl PartialEq for TokenPair {
    fn eq(&self, other: &Self) -> bool {
        self.session_id == other.session_id
    }
}

impl Eq for TokenPair {}

/// Errors from the session store.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session store unavailable")]
    Redis(#[from] redis::RedisError),
    #[error("could not mint a token")]
    Entropy(#[from] getrandom::Error),
    #[error("the account cannot start new sessions")]
    Fenced,
}

fn session_key(session_id: Uuid) -> String {
    format!("sess:{session_id}")
}

fn access_key(token: &str) -> String {
    format!("at:{}", token_digest(token))
}

fn refresh_key(token: &str) -> String {
    format!("rt:{}", token_digest(token))
}

fn used_key(token: &str) -> String {
    format!("rt:used:{}", token_digest(token))
}

fn sessions_key(user_id: Uuid) -> String {
    format!("user:{user_id}:sessions")
}

fn fence_key(user_id: Uuid) -> String {
    format!("user:{user_id}:fenced")
}

/// Create a session, refusing if the account has been fenced.
///
/// The fence check and the writes are one script, so an account deletion
/// cannot slip between them and leave a session behind.
const CREATE: &str = r"
    if redis.call('EXISTS', KEYS[1]) == 1 then
        return 'fenced'
    end
    redis.call('SET', KEYS[2], ARGV[1], 'EX', ARGV[3])
    redis.call('SADD', KEYS[3], ARGV[2])
    redis.call('SET', KEYS[4], ARGV[2], 'EX', ARGV[4])
    redis.call('HSET', KEYS[5], 'sid', ARGV[2], 'uid', ARGV[1])
    redis.call('EXPIRE', KEYS[5], ARGV[3])
    return 'created'
";

/// Exchange a refresh token.
///
/// Consuming the old token, recording it as used, and writing the new pair
/// happen together or not at all.
const ROTATE: &str = r"
    local found = redis.call('HMGET', KEYS[1], 'sid', 'uid')
    if found[1] then
        local sess = 'sess:' .. found[1]
        if redis.call('EXISTS', sess) == 0 then
            -- Logged out while this request was in flight. Refusing here is
            -- what stops a refresh from resurrecting a revoked session.
            return {'invalid'}
        end
        redis.call('DEL', KEYS[1])
        redis.call('HSET', KEYS[2], 'sid', found[1], 'uid', found[2])
        redis.call('EXPIRE', KEYS[2], ARGV[1])
        redis.call('HSET', KEYS[3], 'sid', found[1], 'uid', found[2])
        redis.call('EXPIRE', KEYS[3], ARGV[1])
        redis.call('SET', KEYS[4], found[1], 'EX', ARGV[2])
        redis.call('EXPIRE', sess, ARGV[1])
        return {'rotated', found[1], found[2]}
    end
    local replayed = redis.call('HMGET', KEYS[2], 'sid', 'uid')
    if replayed[2] then
        return {'reused', replayed[1], replayed[2]}
    end
    return {'invalid'}
";

/// Who a live access token belongs to, and which session it came from.
///
/// The session id costs nothing to return: [`SessionStore::authenticate`] has
/// always had to read `at:{digest} -> session_id` before it could read
/// `sess:{session_id} -> user_id`, and used to throw the first hop away. Logout
/// then had to rediscover the session by rotating a refresh token, which is how
/// it came to report success while revoking nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

/// Redis-backed session storage.
#[derive(Clone)]
pub struct SessionStore {
    redis: ConnectionManager,
}

impl SessionStore {
    #[must_use]
    pub const fn new(redis: ConnectionManager) -> Self {
        Self { redis }
    }

    /// Start a session and mint its first token pair.
    ///
    /// # Errors
    ///
    /// [`SessionError::Fenced`] when the account has been deleted,
    /// [`SessionError::Entropy`] when the operating system cannot supply
    /// randomness, [`SessionError::Redis`] otherwise.
    pub async fn create(&self, user_id: Uuid) -> Result<TokenPair, SessionError> {
        let session_id = Uuid::now_v7();
        let access = crate::shared::security::generate_token()?;
        let refresh = crate::shared::security::generate_token()?;

        let mut conn = self.redis.clone();
        let outcome: String = Script::new(CREATE)
            .key(fence_key(user_id))
            .key(session_key(session_id))
            .key(sessions_key(user_id))
            .key(access_key(&access))
            .key(refresh_key(&refresh))
            .arg(user_id.to_string())
            .arg(session_id.to_string())
            .arg(REFRESH_TTL.as_secs())
            .arg(ACCESS_TTL.as_secs())
            .invoke_async(&mut conn)
            .await?;

        if outcome == "fenced" {
            return Err(SessionError::Fenced);
        }

        Ok(TokenPair {
            access,
            refresh,
            session_id,
        })
    }

    /// Resolve an access token to the account it belongs to, and the session
    /// it came from.
    ///
    /// Both the token mapping and the session must exist. That second check is
    /// what makes logout instant.
    ///
    /// # Errors
    ///
    /// [`SessionError::Redis`] when the store is unreachable.
    pub async fn authenticate(&self, access: &str) -> Result<Option<Resolved>, SessionError> {
        let mut conn = self.redis.clone();

        let Some(raw_session): Option<String> = conn.get(access_key(access)).await? else {
            return Ok(None);
        };
        let Ok(session_id) = Uuid::parse_str(&raw_session) else {
            return Ok(None);
        };
        let Some(raw_user): Option<String> = conn.get(session_key(session_id)).await? else {
            return Ok(None);
        };
        let Ok(user_id) = Uuid::parse_str(&raw_user) else {
            return Ok(None);
        };

        Ok(Some(Resolved {
            user_id,
            session_id,
        }))
    }

    /// Exchange a refresh token for a new pair.
    ///
    /// # Errors
    ///
    /// [`SessionError::Entropy`] or [`SessionError::Redis`].
    pub async fn rotate(&self, refresh: &str) -> Result<Rotation, SessionError> {
        let new_access = crate::shared::security::generate_token()?;
        let new_refresh = crate::shared::security::generate_token()?;

        let mut conn = self.redis.clone();
        let outcome: Vec<String> = Script::new(ROTATE)
            .key(refresh_key(refresh))
            .key(used_key(refresh))
            .key(refresh_key(&new_refresh))
            .key(access_key(&new_access))
            .arg(REFRESH_TTL.as_secs())
            .arg(ACCESS_TTL.as_secs())
            .invoke_async(&mut conn)
            .await?;

        match outcome.first().map(String::as_str) {
            Some("rotated") => {
                let session_id = outcome
                    .get(1)
                    .and_then(|id| Uuid::parse_str(id).ok())
                    .ok_or_else(|| corrupt("rotate returned an unreadable session id"))?;
                Ok(Rotation::Rotated(Box::new(TokenPair {
                    access: new_access,
                    refresh: new_refresh,
                    session_id,
                })))
            }
            Some("reused") => {
                let user_id = outcome
                    .get(2)
                    .and_then(|id| Uuid::parse_str(id).ok())
                    .ok_or_else(|| corrupt("reuse returned an unreadable user id"))?;
                Ok(Rotation::Reused { user_id })
            }
            _ => Ok(Rotation::Invalid),
        }
    }

    /// End one session. Its tokens stop working immediately.
    ///
    /// # Errors
    ///
    /// [`SessionError::Redis`] when the store is unreachable.
    pub async fn revoke(&self, user_id: Uuid, session_id: Uuid) -> Result<(), SessionError> {
        let mut conn = self.redis.clone();
        let _: () = conn.del(session_key(session_id)).await?;
        let _: () = conn
            .srem(sessions_key(user_id), session_id.to_string())
            .await?;
        Ok(())
    }

    /// End every session an account has.
    ///
    /// `fence` stops new sessions from being created, which account deletion
    /// needs and reuse detection must not use — after a stolen token is
    /// detected the owner still has to be able to log in again.
    ///
    /// # Errors
    ///
    /// [`SessionError::Redis`] when the store is unreachable.
    pub async fn revoke_all(&self, user_id: Uuid, fence: bool) -> Result<usize, SessionError> {
        let mut conn = self.redis.clone();

        // The fence goes up first. Setting it afterwards would leave a window
        // in which a login lands between reading the set and finishing.
        if fence {
            let _: () = conn
                .set_ex(fence_key(user_id), "1", REFRESH_TTL.as_secs())
                .await?;
        }

        let ids: Vec<String> = conn.smembers(sessions_key(user_id)).await?;
        for id in &ids {
            let _: () = conn.del(format!("sess:{id}")).await?;
        }
        let _: () = conn.del(sessions_key(user_id)).await?;

        Ok(ids.len())
    }
}

fn corrupt(message: &'static str) -> redis::RedisError {
    redis::RedisError::from((redis::ErrorKind::UnexpectedReturnType, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_derived_from_the_digest_and_never_the_token() {
        // The property the whole layout rests on: a Redis key must not be
        // presentable as a credential.
        let token = "a-token-that-must-not-appear-in-any-key";

        for key in [access_key(token), refresh_key(token), used_key(token)] {
            assert!(!key.contains(token), "the raw token leaked into {key}");
        }
    }

    #[test]
    fn the_three_token_namespaces_do_not_collide() {
        // `at:`, `rt:`, and `rt:used:` must stay distinct even for the same
        // token, or a refresh token would authenticate as an access token.
        let token = "same";
        let keys = [access_key(token), refresh_key(token), used_key(token)];
        let unique: std::collections::HashSet<_> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len(), "{keys:?}");
    }

    #[test]
    fn an_access_token_lives_no_longer_than_its_session() {
        // A token mapping that outlived its session would authenticate
        // against a session that no longer exists.
        assert!(ACCESS_TTL <= REFRESH_TTL);
    }

    #[test]
    fn the_lua_scripts_are_syntactically_plausible() {
        // Not a substitute for running them — the integration test does that.
        // This catches an unbalanced block introduced while editing.
        for script in [CREATE, ROTATE] {
            assert_eq!(
                script.matches("if ").count(),
                script.matches("end").count(),
                "unbalanced if/end in:\n{script}"
            );
        }
    }
}
