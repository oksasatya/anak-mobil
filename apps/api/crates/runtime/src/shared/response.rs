//! The response envelope every API endpoint answers in.
//!
//! ```jsonc
//! { "meta": { "request_id", "timestamp", "status" }, "data": { ... } }
//! { "meta": { "request_id", "timestamp", "status" }, "error": { "code", "message", "details"? } }
//! ```
//!
//! One shape for success and failure means a client writes one parser, and a
//! person pasting a response into a support thread carries the request id and
//! the status with it.
//!
//! Three properties are deliberate, each carried over from the Go boilerplate
//! this mirrors:
//!
//! 1. **`meta` is declared first.** Serde serialises in declaration order.
//!    Key order means nothing to a parser — this is for the human who wants
//!    the request id before scrolling past the payload.
//! 2. **`meta.status` repeats the HTTP status inside the body**, so a pasted
//!    body carries it without its headers. It is computed once, from the same
//!    value that sets the status line. Deriving it twice is how a body starts
//!    disagreeing with the response it sits in.
//! 3. **`204 No Content` writes no envelope at all.** No body means no status
//!    field that could contradict anything.
//!
//! The probe endpoints are the one deliberate exception: they answer flat
//! JSON, because a load balancer is not an API client and reading `meta` to
//! find a status it already has in the status line serves nobody.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::shared::request_id::RequestId;

/// The envelope header.
#[derive(Debug, Serialize)]
pub struct Meta {
    /// Absent outside a request — see [`RequestId::current`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// RFC 3339, UTC.
    pub timestamp: String,
    /// The same status the response line carries.
    pub status: u16,
}

impl Meta {
    fn for_status(status: StatusCode) -> Self {
        Self {
            request_id: RequestId::current().map(|id| id.as_str().to_owned()),
            timestamp: now_rfc3339(),
            status: status.as_u16(),
        }
    }
}

/// The `error` half of the envelope.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    /// The stable contract a client switches on.
    pub code: &'static str,
    /// Human-facing text. May be reworded at any time; never parse it.
    pub message: &'static str,
    /// Field-level detail for a validation failure. Never a cause chain — an
    /// internal error's detail belongs in the log, not in a response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// The wire shape. Private: it is only ever built by this module, which is
/// what keeps `meta` correct.
#[derive(Serialize)]
struct Wire<T> {
    meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBody>,
}

/// A successful response, carrying its status so the body and the status line
/// cannot drift apart.
#[derive(Debug)]
pub struct ApiResponse<T> {
    status: StatusCode,
    data: T,
}

impl<T> ApiResponse<T> {
    /// `200 OK`.
    #[must_use]
    pub const fn ok(data: T) -> Self {
        Self {
            status: StatusCode::OK,
            data,
        }
    }

    /// `201 Created`.
    #[must_use]
    pub const fn created(data: T) -> Self {
        Self {
            status: StatusCode::CREATED,
            data,
        }
    }

    /// Any other success status.
    #[must_use]
    pub const fn with_status(status: StatusCode, data: T) -> Self {
        Self { status, data }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        // `self.status` is read once and used for both, so property 2 holds
        // by construction rather than by remembering.
        let wire = Wire {
            meta: Meta::for_status(self.status),
            data: Some(self.data),
            error: None,
        };
        (self.status, Json(wire)).into_response()
    }
}

/// `204 No Content` — no body, and therefore no envelope.
#[derive(Debug, Clone, Copy)]
pub struct NoContent;

impl IntoResponse for NoContent {
    fn into_response(self) -> Response {
        StatusCode::NO_CONTENT.into_response()
    }
}

/// Build the error half of the envelope.
///
/// Used by the single `IntoResponse` implementation in [`crate::shared::errors`];
/// nothing else constructs an error body.
pub(crate) fn error_response(status: StatusCode, error: ErrorBody) -> Response {
    let wire = Wire::<()> {
        meta: Meta::for_status(status),
        data: None,
        error: Some(error),
    };
    (status, Json(wire)).into_response()
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        // Rfc3339 cannot fail for a valid OffsetDateTime, and `now_utc` only
        // produces valid ones. An empty string keeps the response well-formed
        // rather than turning a formatting quirk into a 500.
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use serde_json::Value;

    async fn body_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("reading a test response body");
        serde_json::from_slice(&bytes).expect("response body is JSON")
    }

    #[tokio::test]
    async fn success_carries_meta_then_data() {
        let response = ApiResponse::ok(serde_json::json!({ "id": 7 })).into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = body_json(response).await;
        assert_eq!(body["data"]["id"], 7);
        assert!(
            body.get("error").is_none(),
            "a success must not carry an error key"
        );
        assert_eq!(body["meta"]["status"], 200);
        assert!(body["meta"]["timestamp"].is_string());
    }

    #[tokio::test]
    async fn meta_is_the_first_key() {
        // Property 1. Serde writes declaration order, and the test exists so
        // that reordering the struct fails here rather than quietly degrading
        // every pasted response.
        let response = ApiResponse::ok(serde_json::json!({ "id": 1 })).into_response();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("reading a test response body");
        let text = String::from_utf8(bytes.to_vec()).expect("body is utf-8");
        assert!(text.starts_with(r#"{"meta":"#), "body starts with: {text}");
    }

    #[tokio::test]
    async fn meta_status_matches_the_status_line() {
        // Property 2, across every status a helper can produce.
        let cases = [
            (ApiResponse::ok(1).into_response(), StatusCode::OK),
            (ApiResponse::created(1).into_response(), StatusCode::CREATED),
            (
                ApiResponse::with_status(StatusCode::ACCEPTED, 1).into_response(),
                StatusCode::ACCEPTED,
            ),
        ];

        for (response, expected) in cases {
            assert_eq!(response.status(), expected);
            let body = body_json(response).await;
            assert_eq!(
                body["meta"]["status"],
                expected.as_u16(),
                "meta.status disagreed with the status line"
            );
        }
    }

    #[tokio::test]
    async fn no_content_writes_no_body() {
        // Property 3.
        let response = NoContent.into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("reading a test response body");
        assert!(bytes.is_empty(), "204 must not carry a body");
    }

    #[tokio::test]
    async fn the_request_id_reaches_the_envelope() {
        let id = RequestId::generate();
        let expected = id.as_str().to_owned();

        let body = id
            .scope(async {
                let response = ApiResponse::ok(serde_json::json!({})).into_response();
                body_json(response).await
            })
            .await;

        assert_eq!(body["meta"]["request_id"], expected);
    }

    #[tokio::test]
    async fn the_request_id_is_omitted_rather_than_faked_outside_a_request() {
        let body = body_json(ApiResponse::ok(serde_json::json!({})).into_response()).await;
        assert!(
            body["meta"].get("request_id").is_none(),
            "an absent id must be absent, not an empty string"
        );
    }
}
