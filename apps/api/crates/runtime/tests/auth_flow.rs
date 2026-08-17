//! The authentication endpoints, end to end.
//!
//! Drives the real router with `oneshot`, against a real Postgres and a real
//! Redis. Nothing here is faked: the argon2 verification runs, the Lua scripts
//! run, and the assertions are about what a client actually receives.
//!
//! Needs `DATABASE_URL` and `REDIS_URL`. CI supplies both.

// `clippy.toml` exempts `#[test]` functions from the no-expect rule, but not
// the helpers beside them, and a helper that cannot build a request has
// nothing useful to return — aborting the test is the correct outcome. Scoped
// to this file rather than relaxed workspace-wide.
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
            // A missing database used to `return`, and cargo reported the test
            // as PASSING — it captures stderr for passing tests, so the SKIPPED
            // line never reached anyone. Measured: thirteen tests "passing" in
            // 0.00s having executed nothing, and it caught somebody halfway
            // through verifying a sabotage, which is when it does most damage.
            //
            // Failing loudly is the default now. Somebody who genuinely wants to
            // run the unit tests without a database opts out on purpose:
            //
            //     AM_SKIP_INTEGRATION=1 cargo test
            //
            // `make be-test` loads .env, so the normal path never sees this.
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

#[tokio::test]
async fn register_then_login_then_use_the_token() {
    // AC1: signing in yields a short-lived access token and a long-lived
    // refresh token, and the access token authenticates.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    let created = post(
        &app,
        "/auth/register",
        json!({"email": email, "password": "kata sandi panjang"}),
        peer,
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let body = json(created).await;
    assert_eq!(body["data"]["token_type"], "Bearer");
    assert_eq!(body["data"]["expires_in"], 3600);
    assert!(body["data"]["access_token"].is_string());
    assert!(body["data"]["refresh_token"].is_string());

    let signed_in = post(
        &app,
        "/auth/login",
        json!({"email": email, "password": "kata sandi panjang"}),
        peer,
    )
    .await;
    assert_eq!(signed_in.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_password_is_never_echoed_back() {
    // AC4. The most direct way this could go wrong is a debug field on a DTO.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());
    let password = "sandi-rahasia-yang-panjang";

    let response = post(
        &app,
        "/auth/register",
        json!({"email": email, "password": password}),
        peer,
    )
    .await;
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let text = String::from_utf8(bytes.to_vec()).expect("utf-8");

    assert!(
        !text.contains(password),
        "the response echoed the password: {text}"
    );
}

#[tokio::test]
async fn an_unknown_email_and_a_wrong_password_are_indistinguishable() {
    // The enumeration defence. If these differ in status or code, an attacker
    // learns which addresses have accounts without guessing a password.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    post(
        &app,
        "/auth/register",
        json!({"email": email, "password": "kata sandi panjang"}),
        peer,
    )
    .await;

    let wrong_password = post(
        &app,
        "/auth/login",
        json!({"email": email, "password": "salah sekali"}),
        a_peer(),
    )
    .await;
    let unknown_email = post(
        &app,
        "/auth/login",
        json!({"email": an_email(), "password": "salah sekali"}),
        a_peer(),
    )
    .await;

    assert_eq!(wrong_password.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_email.status(), StatusCode::UNAUTHORIZED);

    let (a, b) = (json(wrong_password).await, json(unknown_email).await);
    assert_eq!(a["error"]["code"], b["error"]["code"]);
    assert_eq!(a["error"]["message"], b["error"]["message"]);
}

#[tokio::test]
async fn a_short_password_is_refused_with_a_field_message() {
    let app = app!();
    let response = post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "password": "pendek"}),
        a_peer(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = json(response).await;
    assert_eq!(body["error"]["code"], "validation_failed");
    assert!(body["error"]["details"]["password"].is_string());
}

#[tokio::test]
async fn a_taken_email_is_refused() {
    let app = app!();
    let (email, peer) = (an_email(), a_peer());
    let body = json!({"email": email, "password": "kata sandi panjang"});

    assert_eq!(
        post(&app, "/auth/register", body.clone(), peer)
            .await
            .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        post(&app, "/auth/register", body, peer).await.status(),
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn email_matching_ignores_case() {
    // CITEXT, so `Budi@…` and `budi@…` are one account rather than two.
    let app = app!();
    let email = an_email();
    let shouted = email.to_uppercase();

    post(
        &app,
        "/auth/register",
        json!({"email": email, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    let response = post(
        &app,
        "/auth/login",
        json!({"email": shouted, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn logout_stops_the_next_request() {
    // AC2, from the client's side rather than the store's.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    let body = json(
        post(
            &app,
            "/auth/register",
            json!({"email": email, "password": "kata sandi panjang"}),
            peer,
        )
        .await,
    )
    .await;
    let access = body["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();
    let refresh = body["data"]["refresh_token"]
        .as_str()
        .expect("refresh token")
        .to_owned();

    let out = post_with_auth(
        &app,
        "/auth/logout",
        json!({"refresh_token": refresh}),
        peer,
        Some(&access),
    )
    .await;
    assert_eq!(out.status(), StatusCode::OK);

    // The same token, one request later.
    let after = post_with_auth(
        &app,
        "/auth/logout",
        json!({"refresh_token": "anything"}),
        peer,
        Some(&access),
    )
    .await;
    assert_eq!(
        after.status(),
        StatusCode::UNAUTHORIZED,
        "the access token outlived the logout"
    );
}

#[tokio::test]
async fn refreshing_rotates_and_a_replay_is_refused() {
    // AC3, through the endpoint.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    let body = json(
        post(
            &app,
            "/auth/register",
            json!({"email": email, "password": "kata sandi panjang"}),
            peer,
        )
        .await,
    )
    .await;
    let first_refresh = body["data"]["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_owned();

    let rotated = post(
        &app,
        "/auth/refresh",
        json!({"refresh_token": first_refresh}),
        peer,
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::OK);
    let second = json(rotated).await;
    assert_ne!(
        second["data"]["refresh_token"],
        body["data"]["refresh_token"]
    );

    // Replaying the first token: refused, and every session revoked.
    let replayed = post(
        &app,
        "/auth/refresh",
        json!({"refresh_token": first_refresh}),
        peer,
    )
    .await;
    assert_eq!(replayed.status(), StatusCode::UNAUTHORIZED);

    let orphaned = second["data"]["access_token"]
        .as_str()
        .expect("access")
        .to_owned();
    let after = post_with_auth(
        &app,
        "/auth/logout",
        json!({"refresh_token": "x"}),
        peer,
        Some(&orphaned),
    )
    .await;
    assert_eq!(
        after.status(),
        StatusCode::UNAUTHORIZED,
        "a detected replay must revoke the sessions it protects"
    );
}

#[tokio::test]
async fn a_request_without_a_token_is_refused() {
    let app = app!();
    let response = post(
        &app,
        "/auth/logout",
        json!({"refresh_token": "x"}),
        a_peer(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn repeated_failures_from_one_address_are_throttled() {
    // The reason rate limiting was pulled into this ticket: without it, this
    // loop is a password-guessing machine and an argon2 CPU sink.
    let app = app!();
    let peer = a_peer();

    let mut throttled = false;
    for _ in 0..40 {
        let response = post(
            &app,
            "/auth/login",
            json!({"email": an_email(), "password": "salah"}),
            peer,
        )
        .await;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            throttled = true;
            break;
        }
    }

    assert!(
        throttled,
        "40 failed logins from one address were all allowed through"
    );
}
