//! The middleware that gives every request an identity and a log line.
//!
//! Three things happen here, and each closes a specific hole:
//!
//! 1. **An identifier is minted.** Always ours, never an inbound
//!    `X-Request-Id`. A caller-supplied value lands in a log line, and text
//!    from outside the system reaching a log is how log injection works.
//! 2. **A language is resolved** from `Accept-Language`, once, so the error
//!    path can translate without every handler threading it through.
//! 3. **One line is logged, from a fixed set of scalar fields.** Never the
//!    body, never a struct, and never the URI — see [`route_label`].

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::HeaderName;
use axum::http::header::ACCEPT_LANGUAGE;
use axum::middleware::Next;
use axum::response::Response;

use crate::shared::i18n::Lang;
use crate::shared::request_id::RequestId;

/// Echoed on every response so a proxy log and an application log can be
/// lined up without either one parsing a body.
pub const HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// What a request that matched no route is called in the log.
pub(super) const UNMATCHED: &str = "unmatched";

/// Probe traffic arrives every few seconds and says nothing when healthy.
/// Logged at debug so a normal log is about requests people made.
const PROBE_ROUTES: [&str; 2] = ["/healthz", "/readyz"];

/// What to call this request in the log.
///
/// The matched route *pattern* — `/vehicles/{id}` — never the URI. A URI
/// carries whatever the caller put in it: a password-reset token in a path
/// segment, a plate number in a query string. Logging the pattern keeps every
/// line groupable while making it structurally impossible to log a secret
/// that arrived in a URL.
///
/// A request that matched nothing has no pattern, and that is exactly the
/// case where the URI is most likely to hold something that should not be
/// written down — so it gets a constant instead.
#[must_use]
fn route_label(matched: Option<&str>) -> &str {
    matched.unwrap_or(UNMATCHED)
}

/// Wrap a request: identity in, one log line out.
pub async fn middleware(request: Request, next: Next) -> Response {
    let id = RequestId::generate();
    let lang = Lang::from_accept_language(
        request
            .headers()
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok()),
    );

    // Read before the request is consumed. `MatchedPath` is set by the router
    // during routing; its absence is what `route_label` treats as unmatched.
    let method = request.method().clone();
    let route = route_label(
        request
            .extensions()
            .get::<MatchedPath>()
            .map(MatchedPath::as_str),
    )
    .to_owned();

    let started = Instant::now();
    let mut response = id.clone().scope(lang.scope(next.run(request))).await;

    let status = response.status().as_u16();
    let latency_ms = started.elapsed().as_millis();
    let is_probe = PROBE_ROUTES.contains(&route.as_str());

    if is_probe {
        tracing::debug!(%method, %route, status, latency_ms, request_id = %id, "request");
    } else {
        tracing::info!(%method, %route, status, latency_ms, request_id = %id, "request");
    }

    if let Ok(value) = id.as_str().parse() {
        response.headers_mut().insert(HEADER, value);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_matched_route_is_labelled_by_its_pattern() {
        assert_eq!(route_label(Some("/vehicles/{id}")), "/vehicles/{id}");
    }

    #[test]
    fn an_unmatched_route_is_labelled_by_a_constant() {
        // The case that matters. A request to `/reset/<token>` matches
        // nothing, and the label must not be derived from what was asked for.
        assert_eq!(route_label(None), UNMATCHED);
    }

    #[test]
    fn the_unmatched_label_cannot_carry_caller_input() {
        // `route_label` has no access to the URI at all — it takes the matched
        // pattern or nothing. This asserts the shape of that contract, so a
        // later change that starts passing the URI in has to break it first.
        let label = route_label(None);
        assert!(
            !label.contains('/'),
            "the fallback must not look like a path"
        );
        assert_eq!(label, UNMATCHED);
    }
}
