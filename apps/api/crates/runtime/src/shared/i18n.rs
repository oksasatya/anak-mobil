//! Error codes and the messages they render as.
//!
//! A **code** is the stable contract a client switches on. A **message** is
//! human-facing text that may be rewritten any time without breaking anyone.
//! Keeping them separate is the whole point: rewording an error must never be
//! a breaking change, and a client must never parse prose to decide what
//! happened.
//!
//! Indonesian is the default rather than English. The repository rule is that
//! product-facing text is Bahasa Indonesia, and an error message is product
//! text — it is read by a person deciding what to do next.
//!
//! Codes are an enum, not strings. The `go-backend-boilerplate` this mirrors
//! declares them as constants and falls back to 500 for a code nobody mapped;
//! here the compiler refuses to build a `match` that forgets a variant, so
//! that fallback has nothing to catch.

use std::fmt;

tokio::task_local! {
    static CURRENT_LANG: Lang;
}

/// The language a response renders in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    /// Bahasa Indonesia — the default.
    #[default]
    Id,
    English,
}

impl Lang {
    /// Run `future` with this language visible to everything it awaits.
    ///
    /// Task-local for the same reason the request id is — see
    /// [`crate::shared::request_id`]. A handler can be handed the language as
    /// an argument, but `?` returns an error value with nowhere to carry it,
    /// and the error path is exactly where a translated message matters.
    pub async fn scope<F: Future>(self, future: F) -> F::Output {
        CURRENT_LANG.scope(self, future).await
    }

    /// The language of the request being handled.
    ///
    /// Falls back to the default outside a request, which is the right answer
    /// for a background job writing an operator-facing message.
    #[must_use]
    pub fn current() -> Self {
        CURRENT_LANG.try_with(|lang| *lang).unwrap_or_default()
    }

    /// Pick a language from an `Accept-Language` header value.
    ///
    /// Deliberately not a full RFC 4647 negotiation. Two languages do not
    /// justify a parser; this reads the first tag and falls back to the
    /// default, which is the correct answer for every header we cannot
    /// interpret.
    #[must_use]
    pub fn from_accept_language(header: Option<&str>) -> Self {
        let Some(header) = header else {
            return Self::default();
        };
        let first = header
            .split(',')
            .next()
            .unwrap_or_default()
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();

        if first.starts_with("en") {
            Self::English
        } else {
            Self::default()
        }
    }
}

/// Every error a client can be told about.
///
/// Cross-cutting codes are flat (`not_found`). Domain codes are namespaced by
/// their context (`auth.invalid_credentials`) and arrive with the feature that
/// raises them — adding one here before anything can return it is inventing a
/// contract nobody asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    NotFound,
    ValidationFailed,
    Unauthorized,
    Forbidden,
    Conflict,
    TooManyRequests,
    /// Today's allowance for suggesting new parts is spent.
    ///
    /// Namespaced and separate from [`Self::TooManyRequests`] because the
    /// waits are different by three orders of magnitude. The generic message
    /// says "wait a moment", which is true of a rate limiter measured in
    /// seconds and false of an allowance that refills over twenty-four hours —
    /// somebody told to wait a moment retries into the same wall for hours.
    PartsDailyLimit,
    Internal,
    ServiceUnavailable,
}

impl ErrorCode {
    /// The wire form — this string is the contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::ValidationFailed => "validation_failed",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::Conflict => "conflict",
            Self::TooManyRequests => "too_many_requests",
            Self::PartsDailyLimit => "parts.daily_limit",
            Self::Internal => "internal_error",
            Self::ServiceUnavailable => "service_unavailable",
        }
    }

    /// The message a client sees.
    ///
    /// Never the underlying cause. A 5xx message says something went wrong on
    /// our side and stops there — SQL, hostnames, connection strings, and
    /// tokens live in the log, which is not a place a client can read.
    #[must_use]
    pub const fn message(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Self::NotFound, Lang::Id) => "Data yang diminta tidak ditemukan.",
            (Self::NotFound, Lang::English) => "The requested resource was not found.",

            (Self::ValidationFailed, Lang::Id) => "Ada isian yang belum benar.",
            (Self::ValidationFailed, Lang::English) => "Some fields are not valid.",

            (Self::Unauthorized, Lang::Id) => "Kamu perlu masuk dulu.",
            (Self::Unauthorized, Lang::English) => "You need to sign in first.",

            (Self::Forbidden, Lang::Id) => "Kamu tidak punya akses ke sini.",
            (Self::Forbidden, Lang::English) => "You do not have access to this.",

            (Self::Conflict, Lang::Id) => "Data ini sudah berubah. Muat ulang, lalu coba lagi.",
            (Self::Conflict, Lang::English) => "This has changed. Reload and try again.",

            (Self::PartsDailyLimit, Lang::Id) => {
                "Jatah part baru untuk hari ini sudah habis. Coba lagi besok."
            }
            (Self::PartsDailyLimit, Lang::English) => {
                "Today's allowance for new parts is spent. Try again tomorrow."
            }
            (Self::TooManyRequests, Lang::Id) => "Terlalu banyak permintaan. Tunggu sebentar.",
            (Self::TooManyRequests, Lang::English) => "Too many requests. Wait a moment.",

            (Self::Internal, Lang::Id) => "Ada yang salah di sisi kami. Coba lagi sebentar lagi.",
            (Self::Internal, Lang::English) => {
                "Something went wrong on our side. Try again shortly."
            }

            (Self::ServiceUnavailable, Lang::Id) => {
                "Layanan sedang tidak tersedia. Coba lagi sebentar lagi."
            }
            (Self::ServiceUnavailable, Lang::English) => {
                "The service is unavailable. Try again shortly."
            }
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every code, both languages. A missing arm is a compile error, but an
    /// arm that renders an empty string is not — and an empty error message
    /// reaches a user as a blank dialog.
    #[test]
    fn every_code_renders_in_both_languages() {
        let codes = [
            ErrorCode::NotFound,
            ErrorCode::ValidationFailed,
            ErrorCode::Unauthorized,
            ErrorCode::Forbidden,
            ErrorCode::Conflict,
            ErrorCode::TooManyRequests,
            ErrorCode::Internal,
            ErrorCode::ServiceUnavailable,
        ];

        for code in codes {
            assert!(!code.as_str().is_empty(), "{code} has no wire form");
            for lang in [Lang::Id, Lang::English] {
                assert!(
                    !code.message(lang).trim().is_empty(),
                    "{code} has no message in {lang:?}"
                );
            }
        }
    }

    #[test]
    fn wire_forms_are_unique() {
        // Two codes sharing a string would make the contract ambiguous while
        // still compiling.
        let codes = [
            ErrorCode::NotFound,
            ErrorCode::ValidationFailed,
            ErrorCode::Unauthorized,
            ErrorCode::Forbidden,
            ErrorCode::Conflict,
            ErrorCode::TooManyRequests,
            ErrorCode::Internal,
            ErrorCode::ServiceUnavailable,
        ];
        let mut seen = std::collections::HashSet::new();
        for code in codes {
            assert!(seen.insert(code.as_str()), "duplicate wire form: {code}");
        }
    }

    #[test]
    fn indonesian_is_the_default() {
        assert_eq!(Lang::default(), Lang::Id);
        assert_eq!(Lang::from_accept_language(None), Lang::Id);
        assert_eq!(Lang::from_accept_language(Some("id-ID,id;q=0.9")), Lang::Id);
    }

    #[test]
    fn english_is_selected_when_asked_for() {
        assert_eq!(Lang::from_accept_language(Some("en")), Lang::English);
        assert_eq!(
            Lang::from_accept_language(Some("en-GB,en;q=0.8")),
            Lang::English
        );
        assert_eq!(Lang::from_accept_language(Some("  EN-US  ")), Lang::English);
    }

    #[tokio::test]
    async fn the_scoped_language_is_visible_to_everything_inside() {
        assert_eq!(
            Lang::current(),
            Lang::Id,
            "the default applies outside a request"
        );

        Lang::English
            .scope(async {
                assert_eq!(Lang::current(), Lang::English);
                tokio::task::yield_now().await;
                assert_eq!(Lang::current(), Lang::English, "still set after an await");
            })
            .await;
    }

    #[test]
    fn unknown_and_malformed_headers_fall_back_rather_than_fail() {
        for header in ["", ",", ";q=0.9", "de-DE", "zh-Hans,zh;q=0.9"] {
            assert_eq!(
                Lang::from_accept_language(Some(header)),
                Lang::Id,
                "header {header:?} should fall back to the default"
            );
        }
    }
}
