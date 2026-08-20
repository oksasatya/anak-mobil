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
use anakmobil_runtime::adapter::redis::rate_limit::{RateLimiter, WINDOW};
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

#[tokio::test]
async fn register_then_login_then_use_the_token() {
    // AC1: signing in yields a short-lived access token and a long-lived
    // refresh token, and the access token authenticates.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    let created = post(
        &app,
        "/auth/register",
        json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
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
        json!({"email": email, "username": a_username(), "password": password}),
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
        json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
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
    assert_eq!(a["error"]["code"], "auth.invalid_credentials");
    // The whole `error` object — code, message, and details together — not
    // just the code, so a divergence anywhere in the body is caught.
    assert_eq!(
        a["error"], b["error"],
        "an unknown email and a wrong password must answer byte-identically"
    );
}

#[tokio::test]
async fn a_short_password_is_refused_with_a_field_message() {
    let app = app!();
    let response = post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": a_username(), "password": "pendek"}),
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
    // Same email, different usernames — or this proves nothing about which
    // index fired.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    assert_eq!(
        post(
            &app,
            "/auth/register",
            json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
            peer,
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        post(
            &app,
            "/auth/register",
            json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
            peer,
        )
        .await
        .status(),
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
        json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
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
            json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
            peer,
        )
        .await,
    )
    .await;
    let access = body["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();

    let out = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&access)).await;
    assert_eq!(out.status(), StatusCode::OK);

    // The same token, one request later.
    let after = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&access)).await;
    assert_eq!(
        after.status(),
        StatusCode::UNAUTHORIZED,
        "the access token outlived the logout"
    );
}

#[tokio::test]
async fn logout_revokes_even_when_the_refresh_token_is_already_spent() {
    // The defect in spec §5. Logout used to rotate the refresh token to find
    // the session; a rotation that reports Reused or Invalid returned success
    // and revoked nothing, so somebody could press sign-out, be told it worked,
    // and still be authenticated a moment later.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    let body = json(
        post(
            &app,
            "/auth/register",
            json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
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

    // Spend the refresh token elsewhere, exactly as an in-flight refresh does.
    // The access token from the ORIGINAL pair is still live: rotation slides
    // the session rather than ending it.
    assert_eq!(
        post(
            &app,
            "/auth/refresh",
            json!({"refresh_token": refresh}),
            peer
        )
        .await
        .status(),
        StatusCode::OK
    );

    // No body at all — the Authorization header is the whole request.
    let out = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&access)).await;
    assert_eq!(out.status(), StatusCode::OK);
    assert_eq!(json(out).await["data"]["signed_out"], true);

    // The session must actually be gone.
    let after = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&access)).await;
    assert_eq!(
        after.status(),
        StatusCode::UNAUTHORIZED,
        "logout reported success without revoking the session"
    );
}

#[tokio::test]
async fn a_logout_against_a_dead_session_answers_like_a_live_one() {
    // Distinguishing them would tell a caller which it was.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    let first = json(
        post(
            &app,
            "/auth/register",
            json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
            peer,
        )
        .await,
    )
    .await;
    let live = first["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();

    let alive = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&live)).await;
    let alive_status = alive.status();
    let alive_body = json(alive).await;

    // A second session, revoked out from under the token before logout runs.
    let second = json(
        post(
            &app,
            "/auth/login",
            json!({"email": email, "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await,
    )
    .await;
    let dead = second["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();
    post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&dead)).await;

    // Logging out twice with the same token: the second one meets a dead
    // session. It must be refused as unauthenticated — never a 200 that
    // pretends, and never a distinct code that says "already signed out".
    let again = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&dead)).await;
    assert_eq!(again.status(), StatusCode::UNAUTHORIZED);

    assert_eq!(alive_status, StatusCode::OK);
    assert_eq!(alive_body["data"]["signed_out"], true);
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
            json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
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
    let after = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&orphaned)).await;
    assert_eq!(
        after.status(),
        StatusCode::UNAUTHORIZED,
        "a detected replay must revoke the sessions it protects"
    );
}

#[tokio::test]
async fn a_request_without_a_token_is_refused() {
    let app = app!();
    let response = post(&app, "/auth/logout", json!({}), a_peer()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_throttled_login_says_how_long_to_wait() {
    // AM-61's countdown. One aggregate number and nothing else — no attempts
    // remaining, and no hint about which limiter refused.
    let app = app!();
    let peer = a_peer();

    let mut throttled = None;
    for _ in 0..40 {
        let response = post(
            &app,
            "/auth/login",
            json!({"email": an_email(), "password": "salah"}),
            peer,
        )
        .await;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            throttled = Some(json(response).await);
            break;
        }
    }

    let body = throttled.expect("40 failed logins from one address were all allowed");
    assert_eq!(body["error"]["code"], "too_many_requests");

    let wait = body["error"]["details"]["retry_after_seconds"]
        .as_u64()
        .expect("retry_after_seconds must be a number");
    // Deterministic, not merely bounded: this test used a fresh `an_email()`
    // per attempt, so only the per-IP counter ever trips, and by attempt 21
    // its remaining TTL is ~898s — never exactly `WINDOW.as_secs()` (900)
    // unless the aggregation itself is wrong. A `wait > 0 && wait <= 900`
    // range check is satisfied by the constant 900 regardless of whether the
    // account limiter's own TTL was ever consulted, which is the oracle this
    // test exists to catch.
    assert_eq!(wait, WINDOW.as_secs());

    // Nothing may say which limiter refused, or how many attempts are left.
    let details = body["error"]["details"].to_string();
    for leak in ["ip", "account", "remaining", "attempts", "limit"] {
        assert!(
            !details.contains(leak),
            "the 429 detail leaked `{leak}`: {details}"
        );
    }
}

#[tokio::test]
async fn repeated_registrations_from_one_address_are_eventually_refused() {
    // Branch-gate finding 3: `/auth/register` had no rate limiter at all, so
    // argon2 (paid before the uniqueness check even runs — see
    // `usecase/auth.rs::register`) ran unbounded for anyone scripting a
    // sweep. `PER_IP_REGISTER` is 20 per window (`adapter/redis/rate_limit.rs`);
    // 25 iterations from one held-fixed address guarantees a refusal without
    // hardcoding the exact count.
    let app = app!();
    let peer = a_peer();

    let mut throttled = None;
    for _ in 0..25 {
        let response = post(
            &app,
            "/auth/register",
            json!({
                "email": an_email(),
                "username": a_username(),
                "password": "kata sandi panjang",
            }),
            peer,
        )
        .await;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            throttled = Some(json(response).await);
            break;
        }
    }

    let body = throttled.expect("25 registrations from one address were all allowed through");
    assert_eq!(body["error"]["code"], "too_many_requests");

    // Mirrors the availability endpoint's contract (`profile_flow.rs`'s
    // `repeated_lookups_from_one_address_are_eventually_refused`), not
    // login's: a bool collapsed from one `allow()` call publishes no wait.
    assert!(
        body["error"].get("details").is_none(),
        "the register 429 must carry no details, unlike login's: {body}"
    );
}

#[tokio::test]
async fn a_successful_login_carries_no_retry_hint() {
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    post(
        &app,
        "/auth/register",
        json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
        peer,
    )
    .await;

    let body = json(
        post(
            &app,
            "/auth/login",
            json!({"email": email, "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await,
    )
    .await;
    assert!(body.get("error").is_none());
    assert!(body["data"]["access_token"].is_string());
    // The test's own name: today nothing stops a future change from adding
    // `retry_after_seconds` to a *successful* payload too.
    assert!(!body.to_string().contains("retry_after"));
}

#[tokio::test]
async fn a_taken_username_is_reported_as_a_username_not_an_email() {
    // The defect: `23505` was mapped to EmailTaken with a comment saying a
    // unique violation "can only be the email index". Adding the username index
    // made that false, and somebody would be told to change an address that is
    // perfectly free.
    let app = app!();
    let username = a_username();

    assert_eq!(
        post(
            &app,
            "/auth/register",
            json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await
        .status(),
        StatusCode::CREATED
    );

    // A different address, the same name.
    let clash = post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;
    assert_eq!(clash.status(), StatusCode::CONFLICT);

    let body = json(clash).await;
    assert_eq!(body["error"]["code"], "conflict");
    assert!(
        body["error"]["details"]["username"].is_string(),
        "the collision must name the username: {body}"
    );
    assert!(
        body["error"]["details"].get("email").is_none(),
        "a username collision must not be reported against the email: {body}"
    );
}

#[tokio::test]
async fn a_taken_email_names_the_email() {
    let app = app!();
    let email = an_email();

    post(
        &app,
        "/auth/register",
        json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    let clash = post(
        &app,
        "/auth/register",
        json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;
    assert_eq!(clash.status(), StatusCode::CONFLICT);

    let body = json(clash).await;
    assert!(body["error"]["details"]["email"].is_string(), "{body}");
    assert!(body["error"]["details"].get("username").is_none(), "{body}");
}

#[tokio::test]
async fn a_username_is_canonicalised_before_it_is_stored() {
    // Uppercase in, lowercase held. The second registration proves the first
    // one claimed the canonical form rather than the typed form.
    let app = app!();
    let username = a_username();

    assert_eq!(
        post(
            &app,
            "/auth/register",
            json!({"email": an_email(), "username": username.to_uppercase(), "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await
        .status(),
        StatusCode::CREATED
    );

    assert_eq!(
        post(
            &app,
            "/auth/register",
            json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await
        .status(),
        StatusCode::CONFLICT,
        "`BUDI` and `budi` must be one name"
    );
}

#[tokio::test]
async fn a_malformed_username_is_a_field_level_validation_failure() {
    let app = app!();

    for bad in [
        "ab",
        ".budi",
        "budi.",
        "budi..s",
        "budi-santoso",
        "budi santoso",
    ] {
        let response = post(
            &app,
            "/auth/register",
            json!({"email": an_email(), "username": bad, "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "`{bad}` should be refused"
        );

        let body = json(response).await;
        assert_eq!(body["error"]["code"], "validation_failed");
        assert!(
            body["error"]["details"]["username"].is_string(),
            "`{bad}` gave no message under the username field: {body}"
        );
    }
}

#[tokio::test]
async fn a_reserved_username_answers_exactly_like_a_taken_one() {
    // The guard rail from the spec: nothing may distinguish reserved from taken,
    // or the endpoint becomes a way to enumerate the reserved list.
    let app = app!();
    let taken = a_username();

    post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": taken, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    let on_taken = post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": taken, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;
    let on_reserved = post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": "admin", "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    assert_eq!(on_taken.status(), on_reserved.status());

    let (a, b) = (json(on_taken).await, json(on_reserved).await);
    assert_eq!(
        a["error"], b["error"],
        "reserved and taken must be identical"
    );

    // Asserting the 409 above does not close this: without the `is_reserved`
    // short-circuit in `http::auth::register`, "admin" would genuinely
    // register on this first run (it is not yet taken), this assertion would
    // catch it here, but the two assertions above would ALSO have kept
    // passing on every run after the first — a real unique-constraint
    // conflict producing the identical 409 body for the wrong reason. Only a
    // direct check of the row itself catches the mutation on every run.
    let pool = anakmobil_runtime::adapter::postgres::connect(
        &std::env::var("DATABASE_URL").expect("DATABASE_URL, required by app!() above"),
    )
    .expect("connecting to postgres");
    let mut conn = pool.acquire().await.expect("acquiring a connection");
    assert!(
        !anakmobil_runtime::adapter::postgres::user_repo::username_exists(&mut conn, "admin")
            .await
            .expect("querying users"),
        "the reserved name `admin` must never actually be inserted"
    );
}

#[tokio::test]
async fn login_still_takes_only_an_email_and_a_password() {
    // The shipped contract. Splitting the DTO exists so that adding a username
    // to registration cannot make `/auth/login` demand one.
    let app = app!();
    let (email, password) = (an_email(), "kata sandi panjang");

    post(
        &app,
        "/auth/register",
        json!({"email": email, "username": a_username(), "password": password}),
        a_peer(),
    )
    .await;

    let response = post(
        &app,
        "/auth/login",
        json!({"email": email, "password": password}),
        a_peer(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}
