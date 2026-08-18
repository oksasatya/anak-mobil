//! Cursor pagination.
//!
//! ```jsonc
//! { "meta": { … }, "data": { "items": [ … ], "next_cursor": "…" | null } }
//! ```
//!
//! Pagination lives in `data`, not in `meta`. `meta` means "about this HTTP
//! exchange" — the request id, the time, the status — and putting a page
//! marker there gives it a second meaning.
//!
//! # Why a cursor and not a page number
//!
//! Offset pagination drifts. Between fetching page one and page two, a record
//! inserted above the boundary pushes one row down — so page two repeats it,
//! or, on a delete, skips one entirely. On a service timeline that is rare
//! enough to look like a ghost; on a feed it is constant. Offsets also get
//! slower with depth, because the database counts through every row it is
//! about to discard.
//!
//! The cost is real and worth stating: there is no "page 5 of 12", and no
//! total. A client that needs those needs a different endpoint, not a
//! different default.
//!
//! # What a cursor carries
//!
//! The sort key **and** a unique tiebreaker, never the id alone. Service
//! records sort by a user-supplied date, so somebody recording a 2019 service
//! today inserts a row in the middle of the history — an id-only cursor over
//! a time-ordered id would walk straight past it. Several services can also
//! share one date, which is what the tiebreaker settles.
//!
//! # Why it is not signed
//!
//! A tampered cursor can only move the caller through their own rows: every
//! query that accepts one is already scoped to the owner. There is nothing
//! for a signature to protect, and an unsigned opaque string is one less key
//! to rotate.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Serialize;
use time::Date;
use time::format_description::well_known::Iso8601;
use uuid::Uuid;

/// Rows per page when the caller does not say.
pub const DEFAULT_LIMIT: u16 = 20;

/// The most rows one request may ask for.
///
/// A ceiling rather than a suggestion: without it, `?limit=100000` is a way
/// to make the server assemble an arbitrarily large response on demand.
pub const MAX_LIMIT: u16 = 100;

/// A page of results.
#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// `None` on the last page. A client stops when this is null rather than
    /// when a page comes back short — a full final page is not the end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// A position in a date-ordered list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateCursor {
    pub date: Date,
    pub id: Uuid,
}

/// A cursor the caller sent could not be read.
#[derive(Debug, thiserror::Error)]
#[error("the cursor is not readable")]
pub struct CursorError;

impl DateCursor {
    /// Encode as an opaque string.
    ///
    /// Opaque so its shape is not a contract. A client that parses it will
    /// break the day the sort changes, and this way there is nothing to parse.
    #[must_use]
    pub fn encode(&self) -> String {
        let date = self.date.format(&Iso8601::DATE).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(format!("{date}|{}", self.id))
    }

    /// Read one back.
    ///
    /// # Errors
    ///
    /// [`CursorError`] when the string is not one this server produced.
    /// Rejected rather than ignored: silently starting from the beginning
    /// would make a client's paging loop repeat forever without ever failing.
    pub fn decode(raw: &str) -> Result<Self, CursorError> {
        let bytes = URL_SAFE_NO_PAD.decode(raw).map_err(|_| CursorError)?;
        let text = String::from_utf8(bytes).map_err(|_| CursorError)?;
        let (date, id) = text.split_once('|').ok_or(CursorError)?;

        Ok(Self {
            date: Date::parse(date, &Iso8601::DATE).map_err(|_| CursorError)?,
            id: Uuid::parse_str(id).map_err(|_| CursorError)?,
        })
    }
}

/// Clamp a requested page size into what the server will serve.
#[must_use]
pub fn clamp_limit(requested: Option<u16>) -> u16 {
    requested.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn a_cursor() -> DateCursor {
        DateCursor {
            date: Date::from_calendar_date(2026, Month::August, 15).expect("a real date"),
            id: Uuid::now_v7(),
        }
    }

    #[test]
    fn a_cursor_survives_a_round_trip() {
        let cursor = a_cursor();
        assert_eq!(
            DateCursor::decode(&cursor.encode()).expect("decoding"),
            cursor
        );
    }

    #[test]
    fn a_cursor_is_opaque() {
        // Not a contract. A client that reads a date out of it would break the
        // day the sort changes.
        let cursor = a_cursor();
        let encoded = cursor.encode();
        assert!(!encoded.contains("2026"), "{encoded}");
        assert!(!encoded.contains(&cursor.id.to_string()), "{encoded}");
    }

    #[test]
    fn a_cursor_survives_a_url_and_a_header() {
        let encoded = a_cursor().encode();
        assert!(
            encoded
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "needs escaping somewhere: {encoded}"
        );
    }

    #[test]
    fn nonsense_is_rejected_rather_than_ignored() {
        // The failure this prevents: silently starting from the beginning
        // makes a client's paging loop repeat forever without ever erroring.
        for raw in [
            "",
            "not-base64!!",
            "aGVsbG8",
            "MjAyNi0wOC0xNXxub3QtYS11dWlk",
        ] {
            assert!(DateCursor::decode(raw).is_err(), "accepted {raw:?}");
        }
    }

    #[test]
    fn a_missing_limit_gets_the_default() {
        assert_eq!(clamp_limit(None), DEFAULT_LIMIT);
    }

    #[test]
    fn an_enormous_limit_is_capped() {
        // Otherwise a caller can ask the server to assemble an
        // arbitrarily large response on demand.
        assert_eq!(clamp_limit(Some(u16::MAX)), MAX_LIMIT);
    }

    #[test]
    fn a_limit_of_zero_still_returns_something() {
        // A page of nothing, repeated, is a loop that never advances.
        assert_eq!(clamp_limit(Some(0)), 1);
    }
}
