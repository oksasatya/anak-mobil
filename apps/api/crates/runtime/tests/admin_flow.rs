//! The `Admin` extractor and the admin vehicle read, end to end.
//!
//! An extractor with no route cannot be tested through anything, so this file
//! proves both at once: that an ordinary account is refused, that a demoted
//! admin is refused on their very next admin request while their own ordinary
//! requests stay unaffected, and that the one admin read this ticket ships
//! carries no plate, no VIN, no purchase price, and no spend.

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
use uuid::Uuid;

// Copied from `tests/build_list_flow.rs`, not from `tests/part_merge_flow.rs`
// or `tests/session_store.rs` — both of the latter still return silently when
// their URL is unset, which is the bug PR #18 closed everywhere else.
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
        let Ok(pool) = anakmobil_runtime::adapter::postgres::connect(&database_url) else {
            panic!(
                "DATABASE_URL is set but unusable. The database is part of this \
                 suite, so a green board without it would prove nothing."
            );
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
        let Ok(redis) = anakmobil_runtime::adapter::redis::connect(&redis_url).await else {
            panic!(
                "REDIS_URL is set but unreachable. Sessions live in Redis, so \
                 every authenticated test below would be meaningless."
            );
        };
        let app = http::router(AppState {
            pool: pool.clone(),
            redis: redis.clone(),
            sessions: SessionStore::new(redis.clone()),
            limiter: RateLimiter::new(redis),
        });
        (app, pool)
    }};
}

fn a_peer() -> SocketAddr {
    let id = Uuid::now_v7().as_u128();
    SocketAddr::from(([10, (id >> 16) as u8, (id >> 8) as u8, id as u8], 443))
}

async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    bearer: Option<&str>,
) -> Response {
    let mut builder = Request::builder().method(method).uri(path);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let payload = body.map_or_else(Body::empty, |value| Body::from(value.to_string()));
    let mut request = builder.body(payload).expect("building the request");
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(a_peer()));

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

/// Registers a fresh person, returning their access token and their user id.
///
/// The id comes from the database rather than from the response, because
/// registration answers with tokens and says nothing about who the account is.
async fn a_person(app: &axum::Router, pool: &sqlx::PgPool) -> (String, Uuid) {
    let email = format!("admin-flow-{}@example.com", Uuid::now_v7());
    let username = format!("u{}", &Uuid::now_v7().simple().to_string()[13..]);
    let response = send(
        app,
        "POST",
        "/auth/register",
        Some(json!({ "email": email, "username": username, "password": "kata sandi panjang" })),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let token = json(response).await["data"]["access_token"]
        .as_str()
        .expect("an access token")
        .to_owned();

    let id = sqlx::query_scalar!("SELECT id FROM users WHERE email = $1::citext", email)
        .fetch_one(pool)
        .await
        .expect("the account we just registered");

    (token, id)
}

/// Promotes somebody directly in the database.
///
/// Deliberately SQL rather than the endpoint: this file's first tests exist
/// before `PATCH /admin/users/{id}/role` does, and a test that can only make
/// an admin by using the feature under test cannot fail independently of it.
async fn promote(pool: &sqlx::PgPool, id: Uuid) {
    sqlx::query!("UPDATE users SET platform_role = 'admin' WHERE id = $1", id)
        .execute(pool)
        .await
        .expect("promoting");
}

#[tokio::test]
async fn an_ordinary_account_is_refused_from_an_admin_route() {
    // 403 and not 404: AM-84 AC2 asks for a rejection the person can
    // understand, and hiding the route from an authenticated account buys
    // nothing.
    let (app, pool) = app!();
    let (token, _) = a_person(&app, &pool).await;
    let (_, target) = a_person(&app, &pool).await;

    let response = send(
        &app,
        "GET",
        &format!("/admin/users/{target}/vehicles"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_unauthenticated_caller_is_refused_before_any_role_is_read() {
    let (app, pool) = app!();
    let (_, target) = a_person(&app, &pool).await;

    let response = send(
        &app,
        "GET",
        &format!("/admin/users/{target}/vehicles"),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_demoted_admin_is_refused_on_their_very_next_admin_request() {
    // AC1, and the reason the role is not in the session: the token is never
    // reissued and never revoked, and the refusal still takes effect
    // immediately, because nothing cached the role.
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    let (_, target) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;

    let before = send(
        &app,
        "GET",
        &format!("/admin/users/{target}/vehicles"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(before.status(), StatusCode::OK);

    sqlx::query!(
        "UPDATE users SET platform_role = 'user' WHERE id = $1",
        admin_id
    )
    .execute(&pool)
    .await
    .expect("demoting");

    let after = send(
        &app,
        "GET",
        &format!("/admin/users/{target}/vehicles"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(after.status(), StatusCode::FORBIDDEN, "the role was cached");
}

#[tokio::test]
async fn a_demoted_admins_ordinary_requests_are_unaffected() {
    // The other half of AC1, and the half a literal reading gets wrong. A
    // demoted admin is still a user, and their garage still belongs to them.
    // Also the normal-path regression: adding the Admin extractor must not
    // change what an Authenticated route does.
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;

    sqlx::query!(
        "UPDATE users SET platform_role = 'user' WHERE id = $1",
        admin_id
    )
    .execute(&pool)
    .await
    .expect("demoting");

    let response = send(&app, "GET", "/vehicles", None, Some(&token)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn an_admin_reading_somebody_elses_cars_gets_no_plate_no_vin_and_no_price() {
    // AC4, and the reason the endpoint was pulled into this ticket at all.
    // The status assertion is not decoration: without an admin read endpoint
    // this test would receive a 404 and pass while asserting nothing, which
    // is the defect class this project's reviewers caught nine times in one
    // ticket.
    let (app, pool) = app!();
    let (admin_token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;
    let (owner_token, owner_id) = a_person(&app, &pool).await;

    // Private fields nest under `private`, per `tests/garage_flow.rs`'s
    // `a_car_with_a_plate` — the plan's draft body put them at the top level,
    // which `VehicleRequest` does not accept.
    let created = send(
        &app,
        "POST",
        "/vehicles",
        Some(json!({
            "described_as": "Avanza 2019",
            "private": {
                "plate": "B 1234 XYZ",
                "vin": "MHFXW42G8K0123456",
                "purchase_price": "185000000",
            },
        })),
        Some(&owner_token),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let response = send(
        &app,
        "GET",
        &format!("/admin/users/{owner_id}/vehicles"),
        None,
        Some(&admin_token),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "a 404 would make every assertion below vacuous"
    );

    let body = json(response).await;
    assert_eq!(
        body["data"].as_array().map(Vec::len),
        Some(1),
        "the admin did not see the car at all"
    );

    // Asserted on the whole serialised body, not field by field: a field-by-
    // field check passes on a response that renamed the field, and the values
    // are what must never travel.
    let text = body.to_string();
    for secret in ["B 1234 XYZ", "MHFXW42G8K0123456", "185000000"] {
        assert!(
            !text.contains(secret),
            "private vehicle data reached an admin: {secret} in {text}"
        );
    }
    assert!(
        body["data"][0].get("summary").is_none(),
        "the admin response carried a spend summary, which is service cost"
    );
}

#[tokio::test]
async fn an_admin_promotes_somebody_and_the_response_names_the_change() {
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;
    let (_, target) = a_person(&app, &pool).await;

    let response = send(
        &app,
        "PATCH",
        &format!("/admin/users/{target}/role"),
        Some(json!({ "role": "admin", "reason": "kurasi katalog" })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = json(response).await;
    assert_eq!(body["data"]["target_user_id"], target.to_string());
    assert_eq!(body["data"]["from_role"], "user");
    assert_eq!(body["data"]["to_role"], "admin");
    assert!(body["data"]["created_at"].is_string());

    // The response answers what changed, not who anybody is.
    let text = body.to_string();
    assert!(
        !text.contains("kurasi katalog"),
        "the reason reached the client"
    );
    assert!(
        !text.contains("@example.com"),
        "an email reached the client"
    );
}

#[tokio::test]
async fn repeating_a_successful_promotion_answers_204_so_a_retry_is_safe() {
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;
    let (_, target) = a_person(&app, &pool).await;

    let body = json!({ "role": "admin", "reason": "kurasi katalog" });

    let first = send(
        &app,
        "PATCH",
        &format!("/admin/users/{target}/role"),
        Some(body.clone()),
        Some(&token),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = send(
        &app,
        "PATCH",
        &format!("/admin/users/{target}/role"),
        Some(body),
        Some(&token),
    )
    .await;
    assert_eq!(
        second.status(),
        StatusCode::NO_CONTENT,
        "a retry must not become a 409 or a 500"
    );

    let rows = sqlx::query_scalar!(
        "SELECT count(*) FROM role_changes WHERE target_user_id = $1",
        target
    )
    .fetch_one(&pool)
    .await
    .expect("counting");
    assert_eq!(rows, Some(1), "the retry wrote a second audit row");
}

#[tokio::test]
async fn a_non_admin_gets_403_and_the_endpoint_is_not_a_user_id_oracle() {
    // What this pins is the outcome, not the layer: a non-admin gets 403, and
    // the same 403 whether the id is real or invented, so the endpoint reveals
    // nothing about who exists. The name deliberately does NOT claim "before any
    // lookup" — the `Admin` extractor refuses first, but `set_role` also
    // re-checks the actor under the lock, so a black-box status assertion cannot
    // tell which layer produced the 403. Both refuse; that is the point.
    let (app, pool) = app!();
    let (token, _) = a_person(&app, &pool).await;
    let (_, real) = a_person(&app, &pool).await;
    let invented = Uuid::now_v7();

    for target in [real, invented] {
        let response = send(
            &app,
            "PATCH",
            &format!("/admin/users/{target}/role"),
            Some(json!({ "role": "admin", "reason": "should be refused" })),
            Some(&token),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn an_unauthenticated_patch_is_401_on_the_role_route_itself() {
    // The contract table has a 401 row, and the sibling GET route has this test
    // — but 401 on PATCH must be pinned on THIS route, or a change that broke
    // PATCH's authentication independently would stay green behind the GET test.
    let (app, pool) = app!();
    let (_, target) = a_person(&app, &pool).await;

    let response = send(
        &app,
        "PATCH",
        &format!("/admin/users/{target}/role"),
        Some(json!({ "role": "admin", "reason": "no token" })),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_admin_naming_a_target_that_does_not_exist_gets_404() {
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;

    let response = send(
        &app,
        "PATCH",
        &format!("/admin/users/{}/role", Uuid::now_v7()),
        Some(json!({ "role": "admin", "reason": "nobody" })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_blank_reason_is_a_422_naming_the_field() {
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;
    let (_, target) = a_person(&app, &pool).await;

    let response = send(
        &app,
        "PATCH",
        &format!("/admin/users/{target}/role"),
        Some(json!({ "role": "admin", "reason": "   " })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body = json(response).await;
    assert_eq!(body["error"]["code"], "validation_failed");
    assert!(body["error"]["details"]["reason"].is_string());
}

#[tokio::test]
async fn an_admin_may_demote_themselves() {
    // Allowed explicitly. With no last-admin rule there is nothing it can
    // break, and stepping down is an ordinary thing for a person to do.
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;

    let response = send(
        &app,
        "PATCH",
        &format!("/admin/users/{admin_id}/role"),
        Some(json!({ "role": "user", "reason": "mundur" })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    // And the very next admin request is refused, because nothing cached it.
    let after = send(
        &app,
        "GET",
        &format!("/admin/users/{admin_id}/vehicles"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(after.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_user_with_role_history_cannot_be_deleted_by_the_foreign_key_not_the_trigger() {
    // Carried from Task 2's review: nothing in the suite pinned `ON DELETE
    // RESTRICT` on `role_changes` — both foreign keys were checked by hand
    // once, and Task 2's own migration is merged, so this is the only place
    // left to lock it in. If a later reader "harmonised" `role_changes` to
    // `SET NULL` (matching `part_merges`), the append-only trigger would
    // reject the referential UPDATE and take the whole parent DELETE down
    // with it — the exact defect class this project has already shipped
    // once. RESTRICT never writes to the child row, so the trigger never
    // fires at all: Postgres itself blocks the DELETE with 23503, not the
    // trigger's P0001.
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;
    let (_, target) = a_person(&app, &pool).await;

    let response = send(
        &app,
        "PATCH",
        &format!("/admin/users/{target}/role"),
        Some(json!({ "role": "admin", "reason": "kurasi katalog" })),
        Some(&token),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "setting up a role_changes row"
    );

    let result = sqlx::query!("DELETE FROM users WHERE id = $1", target)
        .execute(&pool)
        .await;

    let err = result.expect_err("a user referenced by role_changes must not be deletable");
    let sqlstate = err
        .as_database_error()
        .and_then(|db_err| db_err.code())
        .expect("a rejected DELETE carries a SQLSTATE");
    assert_eq!(
        sqlstate.as_ref(),
        "23503",
        "expected a foreign-key violation, not the append-only trigger's P0001 \
         (SET NULL would have made the trigger reject the referential UPDATE)"
    );
}
