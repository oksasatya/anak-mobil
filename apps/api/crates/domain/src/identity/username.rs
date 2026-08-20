//! The one place a username is decided.
//!
//! Pure: text in, a canonical name or a reason out. No async, no database, no
//! clock — so the rules can be exercised exhaustively at their boundaries
//! without a Postgres anywhere.
//!
//! `CITEXT` in the schema gives case-insensitive **uniqueness**. It is not the
//! validator, and the difference matters: uniqueness cannot tell `budi..s` from
//! `budi.s`, cannot refuse a leading dot, and cannot bound a length. Every path
//! that accepts a username — registration and the availability check — calls
//! this function first, so the column never sees a form this file would refuse.

/// The shortest name worth having. Two characters is a namespace nobody can
/// share fairly.
pub const MIN_LEN: usize = 3;

/// The longest. Thirty fits a profile header without truncation.
pub const MAX_LEN: usize = 30;

/// Names the platform keeps, in canonical form.
///
/// `/@username` is a real route, so a person holding `settings` or `login`
/// would sit on a path the product wants. The list is short on purpose: every
/// entry is a name somebody might otherwise reasonably want, and a long
/// speculative list is a land grab against our own users.
///
/// Never reported as its own outcome — see [`is_reserved`].
///
/// `"me"` is deliberately absent even though `GET /me` is a real route:
/// [`MIN_LEN`] already refuses any two-character name, so it can never be
/// typed as a username — and the invariant this list must hold (every
/// reserved name is itself canonical, see the test below) means listing it
/// anyway would be a self-contradiction, not extra protection.
pub const RESERVED: [&str; 12] = [
    "about",
    "admin",
    "anakmobil",
    "api",
    "edit",
    "help",
    "login",
    "new",
    "profile",
    "register",
    "settings",
    "support",
];

/// Why a username was refused.
///
/// Reserved is deliberately absent: an unavailable name is not a malformed one,
/// and answering them differently would tell a caller which is which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UsernameError {
    #[error("username is shorter than the minimum")]
    TooShort,
    #[error("username is longer than the maximum")]
    TooLong,
    #[error("username contains a character that is not permitted")]
    BadCharacter,
    #[error("username begins or ends with a dot or an underscore")]
    EdgePunctuation,
    #[error("username contains consecutive dots")]
    ConsecutiveDots,
}

/// Trim, lowercase, and check the shape. Returns the form that goes in the
/// column and in a URL.
///
/// Complexity: `O(n)` time over the input's characters, `O(n)` space for the
/// lowered copy, with `n` bounded by [`MAX_LEN`] on every accepted value.
///
/// # Errors
///
/// One [`UsernameError`] per call — the first rule broken, checked in the order
/// a person would notice: length, then alphabet, then the two placement rules.
pub fn canonicalise(raw: &str) -> Result<String, UsernameError> {
    let lowered = raw.trim().to_ascii_lowercase();

    // Characters, not bytes. A two-character name made of multi-byte glyphs
    // would otherwise clear a byte-counted minimum.
    let length = lowered.chars().count();
    if length < MIN_LEN {
        return Err(UsernameError::TooShort);
    }
    if length > MAX_LEN {
        return Err(UsernameError::TooLong);
    }

    if !lowered
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
    {
        return Err(UsernameError::BadCharacter);
    }

    // Byte indexing is safe from here: every character is ASCII, so a byte is a
    // character.
    let bytes = lowered.as_bytes();
    let edge = |b: u8| b == b'.' || b == b'_';
    if bytes.first().copied().is_some_and(edge) || bytes.last().copied().is_some_and(edge) {
        return Err(UsernameError::EdgePunctuation);
    }

    if bytes.windows(2).any(|pair| pair == b"..") {
        return Err(UsernameError::ConsecutiveDots);
    }

    Ok(lowered)
}

/// Is this canonical name one the platform keeps?
///
/// Separate from [`canonicalise`] so that a reserved name can be reported in
/// exactly the way a taken name is. Twelve entries scanned linearly: `O(1)`
/// for a fixed list, and a `HashSet` here would cost more to build than the
/// scan costs to run.
#[must_use]
pub fn is_reserved(canonical: &str) -> bool {
    RESERVED.contains(&canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_name_survives_unchanged() {
        assert_eq!(canonicalise("budi").as_deref(), Ok("budi"));
        assert_eq!(canonicalise("budi_santoso").as_deref(), Ok("budi_santoso"));
        assert_eq!(canonicalise("budi.s").as_deref(), Ok("budi.s"));
        assert_eq!(canonicalise("b3ngkel99").as_deref(), Ok("b3ngkel99"));
    }

    #[test]
    fn case_and_surrounding_space_are_normalised_away() {
        // Somebody typing on a phone gets a capital first letter for free, and
        // a paste carries whitespace. Neither is a reason to refuse a name.
        assert_eq!(canonicalise("  Budi  ").as_deref(), Ok("budi"));
        assert_eq!(canonicalise("BUDI").as_deref(), Ok("budi"));
    }

    #[test]
    fn the_length_boundaries_are_exact() {
        assert_eq!(canonicalise("ab"), Err(UsernameError::TooShort));
        assert!(canonicalise("abc").is_ok());
        assert!(canonicalise(&"a".repeat(MAX_LEN)).is_ok());
        assert_eq!(
            canonicalise(&"a".repeat(MAX_LEN + 1)),
            Err(UsernameError::TooLong)
        );
    }

    #[test]
    fn length_is_counted_in_characters_not_bytes() {
        // "aé" is two characters and three bytes. Counting bytes would let a
        // two-character name through the minimum.
        assert_eq!(canonicalise("aé"), Err(UsernameError::TooShort));
    }

    #[test]
    fn only_lowercase_letters_digits_dot_and_underscore_are_permitted() {
        for bad in [
            "budi-santoso",
            "budi santoso",
            "budi@id",
            "budi!",
            "büdi",
            "budi😀x",
        ] {
            assert_eq!(
                canonicalise(bad),
                Err(UsernameError::BadCharacter),
                "{bad} should be refused"
            );
        }
    }

    #[test]
    fn a_name_may_not_begin_or_end_with_punctuation() {
        for bad in [".budi", "budi.", "_budi", "budi_"] {
            assert_eq!(
                canonicalise(bad),
                Err(UsernameError::EdgePunctuation),
                "{bad} should be refused"
            );
        }
    }

    #[test]
    fn dots_may_not_run_together() {
        // "budi..s" and "budi.s" would read as the same person at a glance,
        // which is the whole reason to refuse it.
        assert_eq!(canonicalise("budi..s"), Err(UsernameError::ConsecutiveDots));
        assert_eq!(canonicalise("a...b"), Err(UsernameError::ConsecutiveDots));
        assert!(canonicalise("budi.s.t").is_ok());
    }

    #[test]
    fn underscores_may_run_together() {
        // Deliberate asymmetry: `__` is visually distinct from `_`, and the
        // confusion the dot rule prevents does not arise.
        assert!(canonicalise("budi__s").is_ok());
    }

    #[test]
    fn a_reserved_name_is_well_formed_but_unavailable() {
        // The distinction that keeps the availability endpoint from leaking.
        // A reserved name must pass canonicalisation so that it can be reported
        // as unavailable in exactly the way a taken name is; making it a
        // validation error would answer 422 where a taken name answers 200
        // {available:false}, and the difference is the oracle.
        assert_eq!(canonicalise("Admin").as_deref(), Ok("admin"));
        assert!(is_reserved("admin"));
        assert!(is_reserved("anakmobil"));
        assert!(!is_reserved("budi"));
    }

    #[test]
    fn every_reserved_name_is_itself_a_valid_username() {
        // A reserved entry that could never be typed anyway is dead weight, and
        // one that is not canonical would never match the value being checked.
        for name in RESERVED {
            assert_eq!(
                canonicalise(name).as_deref(),
                Ok(name),
                "{name} is reserved but not canonical"
            );
        }
    }
}
