//! The one place a failure becomes an HTTP response.
//!
//! Handlers return `Result<ApiResponse<T>, ApiError>` and propagate with `?`.
//! Nothing else builds an error body, chooses a status, or decides what a
//! client is told — which is what makes the rules below enforceable rather
//! than merely documented.
//!
//! Two rules the boilerplate this mirrors states, and one the compiler
//! upgrades from a convention into a guarantee:
//!
//! - **A 5xx never shows its cause to the client.** The cause goes to the log
//!   with the request id; the client gets a generic message. A SQL error, a
//!   hostname, or a connection string in a response body is an information
//!   leak, and the only way to be sure it never happens is for the response
//!   path to have no access to the cause text at all.
//! - **A 4xx is expected.** It is logged at info, never reported as a fault,
//!   and never given a stack trace. A validation failure is the system working.
//! - **Every code maps to exactly one status, in one match.** In Go an
//!   unmapped code falls back to 500; here the match is exhaustive over an
//!   enum, so a new code that nobody mapped does not compile. There is no
//!   fallback because there is no gap for one to cover.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::shared::i18n::{ErrorCode, Lang};
use crate::shared::request_id::RequestId;
use crate::shared::response::{ErrorBody, error_response};

/// A failure on its way to a client.
#[derive(Debug)]
pub struct ApiError {
    code: ErrorCode,
    /// Field-level detail for a validation failure. Reaches the client, so it
    /// carries only what the client supplied — never a cause.
    details: Option<serde_json::Value>,
    /// The underlying cause. Logged, never serialised. Not `pub`, and there
    /// is no accessor, so no future handler can route it into a body.
    source: Option<anyhow::Error>,
}

impl ApiError {
    const fn bare(code: ErrorCode) -> Self {
        Self {
            code,
            details: None,
            source: None,
        }
    }

    #[must_use]
    pub const fn not_found() -> Self {
        Self::bare(ErrorCode::NotFound)
    }

    #[must_use]
    pub const fn unauthorized() -> Self {
        Self::bare(ErrorCode::Unauthorized)
    }

    #[must_use]
    pub const fn forbidden() -> Self {
        Self::bare(ErrorCode::Forbidden)
    }

    #[must_use]
    pub const fn conflict() -> Self {
        Self::bare(ErrorCode::Conflict)
    }

    #[must_use]
    pub const fn too_many_requests() -> Self {
        Self::bare(ErrorCode::TooManyRequests)
    }

    #[must_use]
    pub const fn service_unavailable() -> Self {
        Self::bare(ErrorCode::ServiceUnavailable)
    }

    /// A validation failure, with per-field detail the client can act on.
    #[must_use]
    pub fn validation(details: serde_json::Value) -> Self {
        Self {
            code: ErrorCode::ValidationFailed,
            details: Some(details),
            source: None,
        }
    }

    /// An unexpected failure. The cause is kept for the log and never leaves
    /// this process.
    #[must_use]
    pub fn internal(source: impl Into<anyhow::Error>) -> Self {
        Self {
            code: ErrorCode::Internal,
            details: None,
            source: Some(source.into()),
        }
    }

    /// The code this failure reports. Useful in tests, which assert on the
    /// code rather than on a message that may be reworded.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }
}

/// The single code-to-status mapping.
///
/// Exhaustive by construction: adding a variant to [`ErrorCode`] makes this
/// match non-exhaustive, and the build fails until someone decides what the
/// new code means over HTTP.
#[must_use]
pub const fn status_for(code: ErrorCode) -> StatusCode {
    match code {
        ErrorCode::NotFound => StatusCode::NOT_FOUND,
        ErrorCode::ValidationFailed => StatusCode::UNPROCESSABLE_ENTITY,
        ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        ErrorCode::Forbidden => StatusCode::FORBIDDEN,
        ErrorCode::Conflict => StatusCode::CONFLICT,
        ErrorCode::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
        ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        ErrorCode::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// Anything that reaches a handler as `anyhow` becomes a masked 500.
///
/// This is what lets a use case propagate with `?` without every layer
/// deciding how its failure looks over HTTP.
impl From<anyhow::Error> for ApiError {
    fn from(source: anyhow::Error) -> Self {
        Self::internal(source)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Read once, used for the status line, for `meta.status`, and for the
        // log-level decision below.
        let status = status_for(self.code);
        let code = self.code.as_str();

        if status.is_server_error() {
            // `{:#}` renders the whole context chain, which is the only place
            // the cause is ever written.
            let cause = self
                .source
                .as_ref()
                .map_or_else(|| "unspecified".to_owned(), |err| format!("{err:#}"));
            tracing::error!(
                error.code = code,
                error.cause = %cause,
                request_id = RequestId::current().map(|id| id.as_str().to_owned()),
                "request failed"
            );
        } else {
            // Expected. One line, no cause, no fault report.
            tracing::info!(
                error.code = code,
                request_id = RequestId::current().map(|id| id.as_str().to_owned()),
                "request rejected"
            );
        }

        error_response(
            status,
            ErrorBody {
                code,
                message: self.code.message(Lang::current()),
                details: self.details,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use serde_json::{Value, json};

    async fn body_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("reading a test response body");
        serde_json::from_slice(&bytes).expect("response body is JSON")
    }

    #[test]
    fn every_code_maps_to_a_sensible_status() {
        let cases = [
            (ErrorCode::NotFound, 404),
            (ErrorCode::ValidationFailed, 422),
            (ErrorCode::Unauthorized, 401),
            (ErrorCode::Forbidden, 403),
            (ErrorCode::Conflict, 409),
            (ErrorCode::TooManyRequests, 429),
            (ErrorCode::Internal, 500),
            (ErrorCode::ServiceUnavailable, 503),
        ];

        for (code, expected) in cases {
            assert_eq!(
                status_for(code).as_u16(),
                expected,
                "wrong status for {code}"
            );
        }
    }

    #[tokio::test]
    async fn an_error_body_carries_the_code_and_a_message() {
        let response = ApiError::not_found().into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = body_json(response).await;
        assert_eq!(body["error"]["code"], "not_found");
        assert!(body["error"]["message"].is_string());
        assert!(
            body.get("data").is_none(),
            "an error must not carry a data key"
        );
        assert_eq!(body["meta"]["status"], 404);
    }

    #[tokio::test]
    async fn a_five_hundred_never_shows_its_cause() {
        // The load-bearing test of this module. The cause is deliberately
        // recognisable so that any path leaking it into the body fails here.
        let secret = "postgres://admin:sup3rs3cret@db.internal:5432/anakmobil";
        let response =
            ApiError::internal(anyhow::anyhow!("connect failed: {secret}")).into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("reading a test response body");
        let text = String::from_utf8(bytes.to_vec()).expect("body is utf-8");

        assert!(
            !text.contains("sup3rs3cret"),
            "the body leaked a credential: {text}"
        );
        assert!(
            !text.contains("db.internal"),
            "the body leaked a hostname: {text}"
        );
        assert!(
            !text.contains("connect failed"),
            "the body leaked the cause: {text}"
        );
        assert!(
            text.contains("internal_error"),
            "the body must still name the code"
        );
    }

    #[tokio::test]
    async fn validation_detail_reaches_the_client() {
        let response =
            ApiError::validation(json!({ "plat": "format tidak dikenali" })).into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body = body_json(response).await;
        assert_eq!(body["error"]["code"], "validation_failed");
        assert_eq!(body["error"]["details"]["plat"], "format tidak dikenali");
    }

    #[tokio::test]
    async fn details_are_omitted_when_there_are_none() {
        let body = body_json(ApiError::forbidden().into_response()).await;
        assert!(body["error"].get("details").is_none());
    }

    #[tokio::test]
    async fn the_message_follows_the_requested_language() {
        let indonesian = body_json(ApiError::not_found().into_response()).await;
        let english = Lang::English
            .scope(async { body_json(ApiError::not_found().into_response()).await })
            .await;

        assert_ne!(
            indonesian["error"]["message"], english["error"]["message"],
            "the language scope did not reach the error body"
        );
        // The code is the contract and does not move with the language.
        assert_eq!(indonesian["error"]["code"], english["error"]["code"]);
    }

    #[tokio::test]
    async fn an_anyhow_error_becomes_a_masked_five_hundred() {
        let err: ApiError = anyhow::anyhow!("something in a use case").into();
        assert_eq!(err.code(), ErrorCode::Internal);

        let body = body_json(err.into_response()).await;
        assert_eq!(body["error"]["code"], "internal_error");
        assert!(
            !body.to_string().contains("something in a use case"),
            "the cause reached the client"
        );
    }
}
