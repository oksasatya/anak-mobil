//! Password hashing and token minting.
//!
//! Two different jobs that look similar and must not be confused:
//!
//! **Passwords are low-entropy**, so verification is deliberately slow. A
//! human-chosen password sits in a space an attacker can search, and argon2id
//! exists to make each guess cost memory and time.
//!
//! **Tokens are 256 bits from the operating system's CSPRNG**, so they cannot
//! be searched at all. Hashing them with argon2 would add ~20 ms to every
//! authenticated request and buy nothing. They get SHA-256, which is fast and
//! sufficient — its job is to make a stolen copy of Redis useless, not to
//! resist guessing.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

/// 32 bytes. Guessing one is not a threat model.
const TOKEN_BYTES: usize = 32;

/// 16 bytes, the argon2 recommendation. A salt is not secret; it only has to
/// be different for every hash.
const SALT_BYTES: usize = 16;

/// OWASP's argon2id floor: 19 MiB of memory, two passes, one lane.
///
/// The memory cost is the point — it is what stops an attacker from running
/// millions of guesses in parallel on a GPU. It is also the operational cost:
/// each concurrent verification holds 19 MiB, so ten simultaneous logins want
/// ~190 MiB. That is a real number on a small container, and it is the reason
/// the login endpoint is rate limited rather than left open.
const MEMORY_KIB: u32 = 19_456;
const ITERATIONS: u32 = 2;
const LANES: u32 = 1;

/// A hash of a password could not be produced or read.
#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("could not hash the password")]
    Hash,
    #[error("the stored hash is not a valid PHC string")]
    Malformed,
}

fn argon2() -> Result<Argon2<'static>, PasswordError> {
    let params =
        Params::new(MEMORY_KIB, ITERATIONS, LANES, None).map_err(|_| PasswordError::Hash)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Hash a password for storage.
///
/// Returns a PHC string carrying the algorithm, its parameters, and a random
/// salt — so raising the parameters later does not invalidate existing
/// passwords.
///
/// # Errors
///
/// Returns [`PasswordError::Hash`] when hashing fails.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    // The salt comes from the same source as the tokens rather than from a
    // userspace PRNG, so there is exactly one place in this file that entropy
    // enters and one thing to audit.
    let mut salt_bytes = [0_u8; SALT_BYTES];
    getrandom::fill(&mut salt_bytes).map_err(|_| PasswordError::Hash)?;
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|_| PasswordError::Hash)?;

    argon2()?
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordError::Hash)
}

/// Check a password against a stored hash.
///
/// Wrong password and malformed hash are different outcomes here, but the
/// caller must render them identically to a client — see the login use case.
///
/// # Errors
///
/// Returns [`PasswordError::Malformed`] when the stored hash cannot be parsed.
/// A wrong password is `Ok(false)`, not an error.
pub fn verify_password(password: &str, stored: &str) -> Result<bool, PasswordError> {
    let parsed = PasswordHash::new(stored).map_err(|_| PasswordError::Malformed)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// A hash to verify against when no user was found.
///
/// Login must take the same time whether an email is registered or not.
/// Returning early on a missing user turns the endpoint into an account
/// enumeration oracle: an attacker learns which addresses have accounts by
/// timing the response, without ever guessing a password.
///
/// Generated once per process rather than being a constant, so the value is
/// not a fixed target and no test can accidentally assert against it.
#[must_use]
pub fn decoy_hash() -> &'static str {
    use std::sync::OnceLock;
    static DECOY: OnceLock<String> = OnceLock::new();
    DECOY.get_or_init(|| {
        hash_password("a password nobody has, used only to burn the same time")
            .unwrap_or_else(|_| String::new())
    })
}

/// A fresh bearer token.
///
/// 256 bits straight from the operating system's CSPRNG — no userspace PRNG
/// with state that could be seeded badly or forked. base64url so it survives
/// a header, a URL, and a JSON string without escaping.
///
/// # Errors
///
/// Returns an error when the operating system cannot supply entropy, which is
/// not a condition to paper over: without it, every token would be guessable.
pub fn generate_token() -> Result<String, getrandom::Error> {
    let mut bytes = [0_u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// The key a token is stored under.
///
/// Redis never sees a token, only this. Anyone who reads the database — a
/// dump, a `KEYS *`, a misconfigured replica — gets values that cannot be
/// presented as credentials.
///
/// SHA-256 and not argon2: the input already has 256 bits of entropy, so
/// there is nothing to slow an attacker down about, and this runs on every
/// authenticated request.
#[must_use]
pub fn token_digest(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_verifies_against_its_own_hash() {
        let hash = hash_password("kata sandi yang benar").expect("hashing");
        assert!(verify_password("kata sandi yang benar", &hash).expect("verifying"));
    }

    #[test]
    fn a_wrong_password_does_not_verify() {
        let hash = hash_password("kata sandi yang benar").expect("hashing");
        assert!(!verify_password("kata sandi yang salah", &hash).expect("verifying"));
    }

    #[test]
    fn the_same_password_hashes_differently_every_time() {
        // A random salt per hash. Without it, two users with the same password
        // would have identical hashes, and one cracked hash would reveal every
        // account sharing that password.
        let first = hash_password("sama").expect("hashing");
        let second = hash_password("sama").expect("hashing");
        assert_ne!(first, second);
    }

    #[test]
    fn the_hash_carries_its_parameters() {
        // What makes raising the cost later a non-breaking change.
        let hash = hash_password("apa saja").expect("hashing");
        assert!(hash.starts_with("$argon2id$"), "{hash}");
        assert!(hash.contains(&format!("m={MEMORY_KIB}")), "{hash}");
        assert!(hash.contains(&format!("t={ITERATIONS}")), "{hash}");
    }

    #[test]
    fn a_malformed_stored_hash_is_reported_rather_than_accepted() {
        // Not `Ok(false)`. A corrupted row is an operational problem, and
        // treating it as a wrong password would hide it forever.
        let err = verify_password("apa saja", "bukan phc string").expect_err("should reject");
        assert!(matches!(err, PasswordError::Malformed), "{err:?}");
    }

    #[test]
    fn the_decoy_hash_is_usable_and_matches_nothing() {
        let decoy = decoy_hash();
        assert!(
            !decoy.is_empty(),
            "the decoy must be a real hash to burn real time"
        );
        assert!(!verify_password("", decoy).expect("verifying"));
        assert!(!verify_password("password", decoy).expect("verifying"));
    }

    #[test]
    fn tokens_are_unique_and_long_enough() {
        let first = generate_token().expect("entropy");
        let second = generate_token().expect("entropy");
        assert_ne!(first, second);
        // 32 bytes base64url without padding.
        assert_eq!(first.len(), 43, "{first}");
    }

    #[test]
    fn tokens_survive_a_url_and_a_header() {
        for _ in 0..32 {
            let token = generate_token().expect("entropy");
            assert!(
                token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "token needs escaping somewhere: {token}"
            );
        }
    }

    #[test]
    fn a_digest_is_stable_and_reveals_nothing() {
        let token = generate_token().expect("entropy");
        assert_eq!(token_digest(&token), token_digest(&token));
        assert!(
            !token_digest(&token).contains(&token),
            "the digest must not contain the token"
        );
        assert_ne!(token_digest(&token), token);
    }

    #[test]
    fn different_tokens_digest_differently() {
        let first = generate_token().expect("entropy");
        let second = generate_token().expect("entropy");
        assert_ne!(token_digest(&first), token_digest(&second));
    }
}
