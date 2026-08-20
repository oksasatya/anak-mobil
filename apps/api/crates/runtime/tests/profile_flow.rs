//! `GET /me` and `PATCH /me`, end to end.
//!
//! Drives the real router with `oneshot`, against a real Postgres and a real
//! Redis. Self-contained by this repository's convention for integration
//! suites — the `app!` macro and its helpers are copied verbatim from
//! `auth_flow.rs` rather than shared, so a fourth file never has to be kept in
//! sync with the first three.
//!
//! Needs `DATABASE_URL` and `REDIS_URL`. CI supplies both.

#![expect(
    clippy::expect_used,
    reason = "test helpers abort rather than propagate"
)]

use anakmobil_runtime::adapter::http;
use anakmobil_runtime::adapter::redis::rate_limit::RateLimiter;
use anakmobil_runtime::adapter::redis::session::SessionStore;
use anakmobil_runtime::platform::state::AppState;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};
use std::net::SocketAddr;
use tower::ServiceExt;

/// Build the router, or skip loudly.
macro_rules! app {
    () => {{
        let (Ok(database_url), Ok(redis_url)) =
            (std::env::var("DATABASE_URL"), std::env::var("REDIS_URL"))
        else {
            assert!(
                std::env::var("AM_SKIP_INTEGRATION").is_ok(),
                "DATABASE_URL and REDIS_URL are unset. Run `make be-test`, which loads .env. \
                 To skip the integration suites deliberately, set AM_SKIP_INTEGRATION=1."
            );
            eprintln!("SKIPPED: AM_SKIP_INTEGRATION is set");
            return;
        };

        let pool = match anakmobil_runtime::adapter::postgres::connect(&database_url) {
            Ok(pool) => pool,
            Err(err) => {
                eprintln!("SKIPPED: DATABASE_URL unusable: {err}");
                return;
            }
        };
        if anakmobil_runtime::adapter::postgres::migrate::run(&pool)
            .await
            .is_err()
        {
            panic!(
                "could not migrate the test database. Is Postgres running? \
                 `make db-up`. A suite that skips here reports green having \
                 executed nothing."
            );
        }
        let redis = match anakmobil_runtime::adapter::redis::connect(&redis_url).await {
            Ok(redis) => redis,
            Err(err) => {
                eprintln!("SKIPPED: REDIS_URL unreachable: {err}");
                return;
            }
        };

        http::router(AppState {
            pool,
            redis: redis.clone(),
            sessions: SessionStore::new(redis.clone()),
            limiter: RateLimiter::new(redis),
        })
    }};
}

/// A unique address per test run, so tests do not collide in a shared database.
fn an_email() -> String {
    format!("user-{}@example.com", uuid::Uuid::now_v7())
}

/// A unique, always-valid username per test run.
fn a_username() -> String {
    // Canonical by construction: lowercase hex, no dots, no edges.
    format!("u{}", &uuid::Uuid::now_v7().simple().to_string()[13..])
}

/// A unique client address, so one test's attempts do not rate-limit another's.
fn a_peer() -> SocketAddr {
    let id = uuid::Uuid::now_v7().as_u128();
    let octets = [10, (id >> 16) as u8, (id >> 8) as u8, id as u8];
    SocketAddr::from((octets, 443))
}

async fn post(app: &axum::Router, path: &str, body: Value, peer: SocketAddr) -> Response {
    post_with_auth(app, path, body, peer, None).await
}

async fn post_with_auth(
    app: &axum::Router,
    path: &str,
    body: Value,
    peer: SocketAddr,
    bearer: Option<&str>,
) -> Response {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let mut request = builder
        .body(Body::from(body.to_string()))
        .expect("building the request");
    // The router is built with connect-info in production; `oneshot` skips
    // that layer, so the extension is inserted here.
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(peer));

    app.clone()
        .oneshot(request)
        .await
        .expect("the router is infallible")
}

async fn json(response: Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("reading the body");
    serde_json::from_slice(&bytes).expect("the body is JSON")
}

async fn send_with_auth(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    bearer: &str,
) -> Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", format!("Bearer {bearer}"))
        .header("content-type", "application/json");
    if body.is_none() {
        builder = builder.header("content-length", "0");
    }
    let mut request = builder
        .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
        .expect("building the request");
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(a_peer()));
    app.clone()
        .oneshot(request)
        .await
        .expect("the router is infallible")
}

/// Register and return the access token, so each test starts from a real account.
async fn an_account(app: &axum::Router) -> String {
    let body = json(
        post(
            app,
            "/auth/register",
            json!({"email": an_email(), "username": a_username(), "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await,
    )
    .await;
    body["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned()
}

#[tokio::test]
async fn me_reports_a_fresh_account_as_onboarding_incomplete() {
    // The state a person is in the instant after registering: a username,
    // because register demanded one, and nothing else yet.
    //
    // Registered with a deliberately non-canonical form — uppercased and
    // padded — so this also pins that `GET /me` answers the CANONICAL
    // username, never the form that was submitted. `citext` alone would let
    // `BUDI…` be stored and still authenticate; only the canonicaliser
    // guarantees the column holds the lowercase form this asserts against.
    let app = app!();
    let username = a_username();
    let registered = json(
        post(
            &app,
            "/auth/register",
            json!({
                "email": an_email(),
                "username": format!("  {}  ", username.to_uppercase()),
                "password": "kata sandi panjang",
            }),
            a_peer(),
        )
        .await,
    )
    .await;
    let token = registered["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();

    let response = send_with_auth(&app, "GET", "/me", None, &token).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = json(response).await;
    assert!(body["data"]["id"].is_string());
    assert!(body["data"]["email"].is_string());
    assert_eq!(
        body["data"]["username"], username,
        "GET /me must answer the canonical username, not the submitted form: {body}"
    );
    assert!(
        body["data"]["display_name"].is_null(),
        "a fresh account has no display name: {body}"
    );
    assert_eq!(
        body["data"]["has_vehicles"], false,
        "a fresh account has no car"
    );
}

#[tokio::test]
async fn adding_a_vehicle_flips_has_vehicles_to_true() {
    // `has_vehicles` is the SOLE input to the onboarding gate: it was
    // previously exercised only at `false`, so replacing the repository's
    // `EXISTS(...)` subselect with a constant `FALSE` left every suite green
    // while trapping every user who owns a car inside the first-vehicle
    // wizard forever.
    let app = app!();
    let token = an_account(&app).await;

    let created = send_with_auth(
        &app,
        "POST",
        "/vehicles",
        Some(json!({"described_as": "Toyota Avanza 2019"})),
        &token,
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let body = json(send_with_auth(&app, "GET", "/me", None, &token).await).await;
    assert_eq!(
        body["data"]["has_vehicles"], true,
        "an account that owns a car must report has_vehicles: true: {body}"
    );
}

#[tokio::test]
async fn me_never_returns_the_password_hash() {
    let app = app!();
    let token = an_account(&app).await;

    let response = send_with_auth(&app, "GET", "/me", None, &token).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = json(response).await;
    // Exact key set, not substring absence: a substring blocklist catches an
    // argon2 hash and nothing else — not a bcrypt hash, not a session token,
    // not `platform_role`. This is the contract Plans B, C and D actually
    // need pinned.
    let keys: std::collections::BTreeSet<&str> = body["data"]
        .as_object()
        .expect("data is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        std::collections::BTreeSet::from([
            "id",
            "email",
            "username",
            "display_name",
            "has_vehicles"
        ]),
        "GET /me must answer exactly this shape: {body}"
    );
}

#[tokio::test]
async fn me_answers_the_callers_own_row_not_anyone_elses() {
    // Neither existing assertion (`id.is_string()`, `email.is_string()`) is
    // compared to who actually registered. This is the assertion that would
    // survive a future refactor adding a `?user_id=` parameter "for admin
    // convenience" — the kind of change that looks harmless in review.
    let app = app!();
    let (token_a, token_b) = (an_account(&app).await, an_account(&app).await);

    let a = json(send_with_auth(&app, "GET", "/me", None, &token_a).await).await;
    let b = json(send_with_auth(&app, "GET", "/me", None, &token_b).await).await;

    assert_ne!(
        a["data"]["id"], b["data"]["id"],
        "A's token must answer A's row, not B's"
    );
}

#[tokio::test]
async fn me_refuses_a_caller_without_a_token() {
    let app = app!();
    let request = Request::builder()
        .method("GET")
        .uri("/me")
        .body(Body::empty())
        .expect("building the request");
    let response = app.oneshot(request).await.expect("infallible");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn setting_a_display_name_moves_the_account_past_the_profile_step() {
    let app = app!();
    let token = an_account(&app).await;

    let response = send_with_auth(
        &app,
        "PATCH",
        "/me",
        Some(json!({"display_name": "  Budi Santoso  "})),
        &token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = json(response).await;
    assert_eq!(
        body["data"]["display_name"], "Budi Santoso",
        "surrounding whitespace should be trimmed, not stored"
    );

    // And it is the same shape GET answers with, so one client parser covers both.
    let fetched = json(send_with_auth(&app, "GET", "/me", None, &token).await).await;
    assert_eq!(fetched["data"], body["data"]);
}

#[tokio::test]
async fn an_empty_patch_changes_nothing_and_still_answers_the_profile() {
    // A PATCH with no keys is not an error. It is how a client asks for the
    // current state without caring which fields it might have sent.
    let app = app!();
    let token = an_account(&app).await;

    send_with_auth(
        &app,
        "PATCH",
        "/me",
        Some(json!({"display_name": "Budi"})),
        &token,
    )
    .await;

    let response = send_with_auth(&app, "PATCH", "/me", Some(json!({})), &token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["data"]["display_name"], "Budi");
}

#[tokio::test]
async fn a_blank_or_oversized_display_name_is_refused_under_its_field() {
    let app = app!();
    let token = an_account(&app).await;

    for bad in ["", "   ", &"a".repeat(61)] {
        let response = send_with_auth(
            &app,
            "PATCH",
            "/me",
            Some(json!({"display_name": bad})),
            &token,
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "`{bad}` should be refused"
        );
        let body = json(response).await;
        assert!(
            body["error"]["details"]["display_name"].is_string(),
            "{body}"
        );
    }
}

/// A lookup from a caller-chosen address, so repeated calls can be counted
/// against the same limiter key.
async fn availability_from(app: &axum::Router, username: &str, peer: SocketAddr) -> Response {
    let mut request = Request::builder()
        .method("GET")
        .uri(format!("/usernames/{username}/availability"))
        .body(Body::empty())
        .expect("building the request");
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(peer));
    app.clone().oneshot(request).await.expect("infallible")
}

async fn availability(app: &axum::Router, username: &str) -> Response {
    availability_from(app, username, a_peer()).await
}

#[tokio::test]
async fn a_free_username_is_available_without_a_session() {
    // Public on purpose: the register screen needs this before a session
    // exists. Every other non-probe route in this router is authenticated.
    let app = app!();
    let response = availability(&app, &a_username()).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["data"]["available"], true);
}

#[tokio::test]
async fn a_taken_username_is_unavailable() {
    let app = app!();
    let username = a_username();

    post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    assert_eq!(
        json(availability(&app, &username).await).await["data"]["available"],
        false
    );
}

#[tokio::test]
async fn availability_ignores_case_the_way_the_column_does() {
    let app = app!();
    let username = a_username();

    post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    assert_eq!(
        json(availability(&app, &username.to_uppercase()).await).await["data"]["available"],
        false,
        "`BUDI` and `budi` are one name"
    );
}

/// A response's headers, minus the two that are expected to differ on every
/// call by design.
///
/// `x-request-id` is obviously per-request. `content-length` is the subtler one,
/// and it made this comparison flaky: the envelope carries an RFC3339
/// `meta.timestamp`, whose fractional seconds are serialised with trailing zeros
/// trimmed — so the same body is 143 or 142 bytes depending on the microsecond it
/// was produced in. Measured directly against a running server: eight identical
/// requests returned `143 143 143 143 143 143 143` and one `142`.
///
/// Comparing it therefore fails against *correct* code roughly one call in ten,
/// which is worse than not comparing it at all — a test that cries wolf gets
/// ignored, and this one guards the endpoint's central safety property.
///
/// Nothing is lost by excluding it. The property being defended is that a
/// reserved name and a taken name are indistinguishable, and both answer
/// `available: false` — identical bodies, which the caller asserts separately and
/// exactly. This helper exists for the *other* half: that no future handler adds
/// a distinguishing header such as `x-reserved: true`. That still holds.
fn headers_without_request_id(response: &Response) -> std::collections::BTreeMap<String, String> {
    response
        .headers()
        .iter()
        .filter(|(name, _)| *name != "x-request-id" && *name != "content-length")
        .map(|(name, value)| {
            (
                name.to_string(),
                value.to_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_reserved_name_answers_byte_for_byte_like_a_taken_one() {
    // The guard rail the whole endpoint rests on. If these ever differ, the
    // endpoint becomes a way to enumerate the reserved list — and, worse, a
    // precedent for answering "unavailable" in two distinguishable ways.
    let app = app!();
    let taken = a_username();

    post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": taken, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    let on_taken = availability(&app, &taken).await;
    let on_reserved = availability(&app, "admin").await;
    assert_eq!(on_taken.status(), on_reserved.status());

    // Headers too, not only the body: a future handler adding e.g.
    // `x-reserved: true` would slip past a body-only comparison.
    assert_eq!(
        headers_without_request_id(&on_taken),
        headers_without_request_id(&on_reserved),
        "reserved and taken must send identical headers"
    );

    let (a, b) = (json(on_taken).await, json(on_reserved).await);
    assert_eq!(a["data"], b["data"], "reserved and taken must be identical");
}

#[tokio::test]
async fn every_reserved_name_answers_unavailable() {
    let app = app!();
    // Iterate the constant, not a copied list: a copy silently disagrees with
    // the source the moment either changes.
    for name in anakmobil_domain::identity::username::RESERVED {
        assert_eq!(
            json(availability(&app, name).await).await["data"]["available"],
            false,
            "{name} is reserved and must be unavailable"
        );
    }
}

#[tokio::test]
async fn a_malformed_username_is_a_validation_failure_not_an_availability_answer() {
    // Shape is public knowledge — the client mirrors the regex for instant
    // feedback — so refusing a malformed name leaks nothing and is more useful
    // than answering `false`.
    let app = app!();

    for bad in ["ab", ".budi", "budi.", "budi..s", "budi-santoso"] {
        let response = availability(&app, bad).await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "`{bad}` should be refused"
        );
        assert!(json(response).await["error"]["details"]["username"].is_string());
    }
}

#[tokio::test]
async fn availability_never_mentions_an_email_or_an_account() {
    // The endpoint answers one boolean about one name. Anything else here
    // would correlate the username namespace with account state. An exact key
    // set catches any extra field, not only the five a substring blocklist
    // happened to name — a handler leaking a sixth field nobody thought to
    // list would still pass a substring check.
    let app = app!();
    let username = a_username();

    post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    let response = availability(&app, &username).await;
    // Without this, a 404 (the route does not exist yet) is also an object
    // with no `email`/`data` keys — the same vacant-pass trap Task 6's red
    // phase hit.
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;

    let top: std::collections::BTreeSet<&str> = body
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top,
        std::collections::BTreeSet::from(["meta", "data"]),
        "{body}"
    );

    let data: std::collections::BTreeSet<&str> = body["data"]
        .as_object()
        .expect("data is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        data,
        std::collections::BTreeSet::from(["available"]),
        "{body}"
    );
}

#[tokio::test]
async fn repeated_lookups_from_one_address_are_eventually_refused() {
    // The only security control on this endpoint, and until this test it was
    // dead code: every other test above calls `availability()`, whose helper
    // mints a fresh address per request, so the per-IP counter this handler
    // relies on never accumulated. `PER_IP_LOOKUP` is 60 per window
    // (`adapter/redis/rate_limit.rs`); 70 iterations from one held-fixed
    // address guarantees a refusal without hardcoding the exact count.
    let app = app!();
    let peer = a_peer();

    let mut throttled = None;
    for _ in 0..70 {
        let response = availability_from(&app, &a_username(), peer).await;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            throttled = Some(json(response).await);
            break;
        }
    }

    let body = throttled.expect("70 lookups from one address were all allowed through");
    assert_eq!(body["error"]["code"], "too_many_requests");

    // The inverse of login's contract (`auth_flow.rs`'s
    // `a_throttled_login_says_how_long_to_wait`): login publishes a wait in
    // `details.retry_after_seconds`, this endpoint publishes no `details` at
    // all. That asymmetry is the contract, and nothing pinned it before this.
    assert!(
        body["error"].get("details").is_none(),
        "the lookup 429 must carry no details, unlike login's: {body}"
    );
}
