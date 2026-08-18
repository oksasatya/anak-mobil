//! A fixed-window counter, sized for the login endpoint.
//!
//! # Why this ships with authentication rather than with the rate-limiting
//! story
//!
//! An unthrottled login endpoint is two problems, not one. Unlimited guesses
//! eventually break any reused or weak password, and — specific to argon2 —
//! every attempt costs 19 MiB and real CPU, so a few hundred concurrent
//! requests exhaust a small container without a single correct password. The
//! hashing that protects stored passwords is what makes the endpoint cheap to
//! attack.
//!
//! General API rate limiting still belongs to its own story. This is the
//! narrow version that keeps one endpoint from being an open door.
//!
//! # Two limits, not one
//!
//! Per-IP alone lets a botnet spread guesses across addresses. Per-account
//! alone lets one host hammer many accounts, and also hands an attacker a
//! denial-of-service against a specific person. Both together mean an attacker
//! must have many addresses *and* accept that each account absorbs only a few
//! attempts.
//!
//! The account key holds a digest of the email, not the email. Redis should
//! not accumulate a readable list of who has been trying to sign in.

use std::time::Duration;

use redis::Script;
use redis::aio::ConnectionManager;

use crate::shared::security::token_digest;

/// How long a window lasts.
///
/// Minutes, not seconds. A short window is one an attacker simply waits out,
/// which makes the limit a delay rather than a bound.
pub const WINDOW: Duration = Duration::from_secs(15 * 60);

/// Attempts allowed from one address per window.
pub const PER_IP: u32 = 20;

/// Attempts allowed against one account per window.
///
/// Deliberately lower than [`PER_IP`]: a person mistypes a password a handful
/// of times, and twenty is a script. Keep it below the address limit — raising
/// it above would make the account limit unreachable and silently pointless.
pub const PER_ACCOUNT: u32 = 10;

/// Increment and set the expiry together.
///
/// `INCR` followed by a separate `EXPIRE` leaves a window in which the process
/// dies after creating the key and before giving it a lifetime — and a counter
/// with no expiry locks that address or account out permanently.
const HIT: &str = r"
    local count = redis.call('INCR', KEYS[1])
    if count == 1 then
        redis.call('EXPIRE', KEYS[1], ARGV[1])
    end
    return count
";

/// Redis-backed attempt counting.
#[derive(Clone)]
pub struct RateLimiter {
    redis: ConnectionManager,
}

impl RateLimiter {
    #[must_use]
    pub const fn new(redis: ConnectionManager) -> Self {
        Self { redis }
    }

    /// Record an attempt. `true` means it is still within the allowance.
    ///
    /// Counted before the password is checked, so a wrong guess and a right
    /// one cost the same — otherwise the limit would only apply to attackers
    /// who fail, which is not a limit.
    ///
    /// # Errors
    ///
    /// Returns [`redis::RedisError`] when the store is unreachable. The caller
    /// decides whether to fail open or closed; on the login path it fails
    /// closed, because an unthrottled login is worse than a brief outage.
    pub async fn allow(&self, key: &str, limit: u32) -> Result<bool, redis::RedisError> {
        let mut conn = self.redis.clone();
        let count: u32 = Script::new(HIT)
            .key(key)
            .arg(WINDOW.as_secs())
            .invoke_async(&mut conn)
            .await?;
        Ok(count <= limit)
    }

    /// Check both limits for a login attempt.
    ///
    /// Both are recorded even when the first already failed, so an attacker
    /// cannot use one limit to shield the other from counting.
    ///
    /// # Errors
    ///
    /// Returns [`redis::RedisError`] when the store is unreachable.
    pub async fn allow_login(&self, ip: &str, email: &str) -> Result<bool, redis::RedisError> {
        let by_ip = self.allow(&format!("rl:login:ip:{ip}"), PER_IP).await?;
        let by_account = self
            .allow(
                &format!("rl:login:acct:{}", token_digest(email)),
                PER_ACCOUNT,
            )
            .await?;
        Ok(by_ip && by_account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_account_key_does_not_contain_the_email() {
        // Redis must not accumulate a readable list of who has been trying to
        // sign in — including addresses that have no account.
        let email = "budi@example.com";
        let key = format!("rl:login:acct:{}", token_digest(email));
        assert!(!key.contains(email), "{key}");
        assert!(!key.contains("budi"), "{key}");
    }
}
