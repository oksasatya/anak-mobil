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

/// Availability lookups allowed from one address per window.
///
/// Generous next to [`PER_IP`], and deliberately so: a register form checks a
/// name as somebody types it, debounced, and a person who cannot find a free
/// name legitimately tries a dozen. It is a bound on scraping the namespace,
/// not on using it.
pub const PER_IP_LOOKUP: u32 = 60;

/// Registrations allowed from one address per window.
///
/// Same order of magnitude as [`PER_IP`], not [`PER_IP_LOOKUP`]: a lookup is
/// one index probe, but `usecase::auth::register` hashes the password with
/// argon2 *before* the uniqueness check ever runs, so every registration
/// attempt — including one that will 409 — costs what a login attempt costs.
/// Set well above a real person's retry count (a typo, a taken email, a
/// taken username), and deliberately not lower: Indonesian mobile carriers
/// run heavy CGNAT, so a burst of unrelated people registering from one
/// shared address in the same window is ordinary traffic, not an attack — the
/// mistake ledger 84 named and this constant does not repeat.
pub const PER_IP_REGISTER: u32 = 20;

/// Increment, set the expiry, and report how long the window has left.
///
/// `INCR` followed by a separate `EXPIRE` leaves a window in which the process
/// dies after creating the key and before giving it a lifetime — and a counter
/// with no expiry locks that address or account out permanently. `TTL` rides
/// along in the same script so a caller cannot observe the count and the
/// remaining time from two different moments.
const HIT: &str = r"
    local count = redis.call('INCR', KEYS[1])
    if count == 1 then
        redis.call('EXPIRE', KEYS[1], ARGV[1])
    end
    return {count, redis.call('TTL', KEYS[1])}
";

/// One counted attempt.
///
/// Not `pub`: nothing outside this module ever holds one directly.
/// `allow_login` aggregates two into one [`LoginAttempt`] before anything
/// else sees a result, and `allow_lookup` collapses one to its `allowed`
/// bool. Keeping this private is what makes that aggregation a type boundary
/// rather than a convention a future handler could bypass by calling
/// [`RateLimiter::allow`] directly and publishing one limiter's own TTL —
/// precisely the oracle `refusal` exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Attempt {
    /// Whether it is still within the allowance.
    pub allowed: bool,
    /// Seconds until this counter's window resets.
    pub retry_after_seconds: u64,
}

/// What happened when a login attempt was counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginAttempt {
    Allowed,
    /// Refused, with one aggregate wait. It deliberately does not say which
    /// limiter refused and never reports attempts remaining — either would be
    /// the oracle that returning a number at all was challenged for being.
    Refused {
        retry_after_seconds: u64,
    },
}

/// Turn a Redis `TTL` reply into a wait a person can be shown.
///
/// `-1` means no expiry and `-2` means no key; neither is reachable directly
/// after the script above, and neither may become a panic or a zero-second
/// countdown that sends somebody back into the same wall.
fn seconds_from_ttl(ttl: i64) -> u64 {
    u64::try_from(ttl).map_or(WINDOW.as_secs(), |seconds| seconds.max(1))
}

/// The wait to report, given both counters.
///
/// `None` when the attempt is allowed. Otherwise the **larger** of the two
/// remaining windows, whichever limiter actually refused: the two windows start
/// at different moments, so reporting the refusing one's own wait would
/// distinguish a per-address refusal from a per-account one.
fn refusal(by_ip: &Attempt, by_account: &Attempt) -> Option<u64> {
    if by_ip.allowed && by_account.allowed {
        return None;
    }
    Some(
        by_ip
            .retry_after_seconds
            .max(by_account.retry_after_seconds),
    )
}

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

    /// Record an attempt.
    ///
    /// Counted before the password is checked, so a wrong guess and a right one
    /// cost the same — otherwise the limit would only apply to attackers who
    /// fail, which is not a limit.
    ///
    /// # Errors
    ///
    /// Returns [`redis::RedisError`] when the store is unreachable. The caller
    /// decides whether to fail open or closed; on the login path it fails
    /// closed, because an unthrottled login is worse than a brief outage.
    async fn allow(&self, key: &str, limit: u32) -> Result<Attempt, redis::RedisError> {
        let mut conn = self.redis.clone();
        let (count, ttl): (u32, i64) = Script::new(HIT)
            .key(key)
            .arg(WINDOW.as_secs())
            .invoke_async(&mut conn)
            .await?;

        Ok(Attempt {
            allowed: count <= limit,
            retry_after_seconds: seconds_from_ttl(ttl),
        })
    }

    /// Check both limits for a login attempt.
    ///
    /// Both are recorded even when the first already failed, so an attacker
    /// cannot use one limit to shield the other from counting.
    ///
    /// # Errors
    ///
    /// Returns [`redis::RedisError`] when the store is unreachable.
    pub async fn allow_login(
        &self,
        ip: &str,
        email: &str,
    ) -> Result<LoginAttempt, redis::RedisError> {
        let by_ip = self.allow(&format!("rl:login:ip:{ip}"), PER_IP).await?;
        let by_account = self
            .allow(
                &format!("rl:login:acct:{}", token_digest(email)),
                PER_ACCOUNT,
            )
            .await?;

        Ok(
            refusal(&by_ip, &by_account).map_or(LoginAttempt::Allowed, |retry_after_seconds| {
                LoginAttempt::Refused {
                    retry_after_seconds,
                }
            }),
        )
    }

    /// Count an unauthenticated lookup against the calling address.
    ///
    /// Per-IP only. There is no second key to add: the thing being looked up is
    /// a public name, so counting per-name would throttle a popular name for
    /// everybody rather than throttling whoever is scraping.
    ///
    /// # Errors
    ///
    /// Returns [`redis::RedisError`] when the store is unreachable.
    pub async fn allow_lookup(&self, ip: &str) -> Result<bool, redis::RedisError> {
        Ok(self
            .allow(&format!("rl:lookup:ip:{ip}"), PER_IP_LOOKUP)
            .await?
            .allowed)
    }

    /// Count a registration attempt against the calling address.
    ///
    /// Per-IP only, the same shape as [`Self::allow_lookup`]: there is no
    /// account to key a second counter on, because the account this call is
    /// trying to create does not exist yet.
    ///
    /// # Errors
    ///
    /// Returns [`redis::RedisError`] when the store is unreachable.
    pub async fn allow_register(&self, ip: &str) -> Result<bool, redis::RedisError> {
        Ok(self
            .allow(&format!("rl:register:ip:{ip}"), PER_IP_REGISTER)
            .await?
            .allowed)
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

    #[test]
    fn an_allowed_attempt_reports_no_refusal() {
        let ok = Attempt {
            allowed: true,
            retry_after_seconds: 900,
        };
        assert_eq!(refusal(&ok, &ok), None);
    }

    #[test]
    fn a_refusal_reports_the_longer_of_the_two_waits() {
        // The two windows started at different moments, so their remaining
        // times differ. Reporting the refusing limiter's own wait would tell
        // the caller which limiter refused; the larger of the two does not.
        let ip = Attempt {
            allowed: false,
            retry_after_seconds: 42,
        };
        let account = Attempt {
            allowed: false,
            retry_after_seconds: 611,
        };
        assert_eq!(refusal(&ip, &account), Some(611));
        assert_eq!(refusal(&account, &ip), Some(611));
    }

    #[test]
    fn one_limiter_refusing_still_reports_the_longer_wait_of_the_two() {
        // Both counters were incremented, so both have a live window. The
        // answer must not depend on which one said no.
        let allowed_long = Attempt {
            allowed: true,
            retry_after_seconds: 800,
        };
        let refused_short = Attempt {
            allowed: false,
            retry_after_seconds: 30,
        };
        assert_eq!(refusal(&allowed_long, &refused_short), Some(800));
        assert_eq!(refusal(&refused_short, &allowed_long), Some(800));
    }

    #[test]
    fn a_nonsense_ttl_never_becomes_a_zero_second_countdown() {
        // Redis answers -1 for no expiry and -2 for no key. Telling somebody to
        // wait zero seconds sends them straight back into the same wall.
        assert_eq!(seconds_from_ttl(-2), WINDOW.as_secs());
        assert_eq!(seconds_from_ttl(-1), WINDOW.as_secs());
        assert_eq!(seconds_from_ttl(0), 1);
        assert_eq!(seconds_from_ttl(37), 37);
    }
}
