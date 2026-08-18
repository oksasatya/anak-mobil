//! Parts, end to end.
//!
//! The contracts this slice sets: a part somebody types is usable at once,
//! lands in the queue, and is attributed to whoever typed it; the same part
//! typed twice is one row; two wheels that differ only in their numbers are
//! two parts, never one; an incomplete part says what it is missing instead
//! of being filled with a guess; and the daily allowance protects the
//! curation queue from being flooded by one account.

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
use sqlx::PgPool;
use std::net::SocketAddr;
use tower::ServiceExt;
use uuid::Uuid;

// Returns `(app, pool)` rather than the router alone — a plain `app!()` (the
// `service_summary_flow.rs` dialect) is enough when a test only ever talks to
// the router, but the attribution test below has to read `parts.suggested_by`
// directly, and `catalog_flow.rs` already established the pattern for that:
// hand the test its own pool alongside the router.
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

/// Registers a fresh person and returns their access token and email. The
/// email is how a test looks the person back up in `users` afterwards — the
/// session token is opaque by design (see `apps/api/CLAUDE.md`), so it
/// cannot be decoded into a user id.
async fn a_signed_in_person(app: &axum::Router) -> (String, String) {
    let email = format!("parts-{}@example.com", Uuid::now_v7());
    let response = send(
        app,
        "POST",
        "/auth/register",
        Some(json!({"email": email, "password": "kata sandi panjang"})),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let token = json(response).await["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();
    (token, email)
}

/// The id of the account with this email, read straight from `users` rather
/// than trusting anything the client claims.
async fn user_id(pool: &PgPool, email: &str) -> Uuid {
    sqlx::query_scalar!("SELECT id FROM users WHERE email = $1::citext", email)
        .fetch_one(pool)
        .await
        .expect("the registered user exists")
}

/// A wheel nobody else in this run will have typed.
fn a_wheel(product: &str) -> Value {
    json!({
        "category": "wheels",
        "brand": "Enkei",
        "product_name": product,
        "wheel_diameter_in": "18",
        "wheel_width_in": "8.5",
        "offset_et_mm": 40,
        "pcd_bolt_count": 5,
        "pcd_diameter_mm": "114.3",
        "center_bore_mm": "73.1"
    })
}

async fn add(app: &axum::Router, token: &str, body: Value) -> String {
    let response = send(app, "POST", "/parts", Some(body), Some(token)).await;
    assert_eq!(response.status(), StatusCode::CREATED, "adding a part");
    json(response).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

#[tokio::test]
async fn a_typed_part_is_usable_at_once_and_lands_in_the_queue() {
    // AC3, both halves. A NULL part_id would have satisfied neither: no queue
    // row, no suggester, no per-part completeness state. Checking only the
    // response the caller sees would miss the second half entirely — the row
    // itself has to be pending AND attributable to whoever typed it, or the
    // curation queue has a part nobody can ask about.
    let (app, pool) = app!();
    let (token, email) = a_signed_in_person(&app).await;
    let id = add(&app, &token, a_wheel(&format!("RPF1 {}", Uuid::now_v7()))).await;

    let body = json(send(&app, "GET", &format!("/parts/{id}"), None, Some(&token)).await).await;
    assert_eq!(body["data"]["status"], "pending");
    assert_eq!(body["data"]["is_complete"], true);

    let part_id: Uuid = id.parse().expect("a uuid");
    let caller_id = user_id(&pool, &email).await;
    let suggested_by: Option<Uuid> =
        sqlx::query_scalar!("SELECT suggested_by FROM parts WHERE id = $1", part_id)
            .fetch_one(&pool)
            .await
            .expect("the part row exists");
    assert_eq!(
        suggested_by,
        Some(caller_id),
        "the part is not attributed to whoever typed it"
    );
}

#[tokio::test]
async fn the_same_part_typed_twice_is_one_row() {
    // Otherwise the curation queue fills with copies of the same wheel, and
    // the curator's first job every morning is deleting them.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let wheel = a_wheel(&format!("RPF1 {}", Uuid::now_v7()));

    let first = add(&app, &token, wheel.clone()).await;
    let second = add(&app, &token, wheel).await;

    assert_eq!(first, second, "the same configuration produced two rows");
}

#[tokio::test]
async fn two_wheels_that_differ_only_in_width_are_two_parts() {
    // The decision this whole ticket turns on. Enkei RPF1 18x8.5 ET40 and
    // Enkei RPF1 18x9.5 ET45 share a brand and a product name and are
    // different wheels; collapsing them produces a confident wrong number.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let product = format!("RPF1 {}", Uuid::now_v7());

    let narrow = add(&app, &token, a_wheel(&product)).await;

    let mut wide = a_wheel(&product);
    wide["wheel_width_in"] = json!("9.5");
    wide["offset_et_mm"] = json!(45);
    let wide = add(&app, &token, wide).await;

    assert_ne!(narrow, wide, "two different wheels became one part");
}

#[tokio::test]
async fn an_incomplete_wheel_says_what_it_is_missing() {
    // AC2's second half: flagged, not guessed. A plausible invented offset
    // reads as evidence and is worse than a blank one.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;

    let mut wheel = a_wheel(&format!("RPF1 {}", Uuid::now_v7()));
    wheel["offset_et_mm"] = Value::Null;
    wheel["center_bore_mm"] = Value::Null;
    let id = add(&app, &token, wheel).await;

    let body = json(send(&app, "GET", &format!("/parts/{id}"), None, Some(&token)).await).await;
    assert_eq!(body["data"]["is_complete"], false);
    assert_eq!(
        body["data"]["missing_specs"],
        json!(["offset_et_mm", "center_bore_mm"])
    );
    assert_eq!(
        body["data"]["offset_et_mm"],
        Value::Null,
        "left blank rather than filled with a guess"
    );
}

#[tokio::test]
async fn a_pcd_survives_the_round_trip_exactly() {
    // 114.3 through a JSON float is 114.30000000000001, and a fitment engine
    // comparing that to a catalog 114.3 finds no match.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let id = add(&app, &token, a_wheel(&format!("RPF1 {}", Uuid::now_v7()))).await;

    let body = json(send(&app, "GET", &format!("/parts/{id}"), None, Some(&token)).await).await;
    assert_eq!(body["data"]["pcd_diameter_mm"], "114.3");
    assert_eq!(body["data"]["wheel_width_in"], "8.5");
}

#[tokio::test]
async fn an_impossible_spec_is_a_validation_failure_not_a_server_error() {
    // A constraint violation surfacing as a 500 breaks AC3's promise.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;

    let mut wheel = a_wheel("RPF1");
    wheel["pcd_bolt_count"] = json!(0);
    let response = send(&app, "POST", "/parts", Some(wheel), Some(&token)).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn an_impossible_tyre_spec_is_also_a_validation_failure() {
    // The mirror of the wheel case, on the category the platform's own
    // canonical typo lives on: a size typed whole into the aspect-ratio
    // field. Two categories are worth pinning here — a range check that only
    // ever runs against `wheels` in the tests can drift from the `tyres`
    // constraints without anything noticing.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;

    let wheel = json!({
        "category": "tyres",
        "brand": "Bridgestone",
        "product_name": format!("Potenza {}", Uuid::now_v7()),
        "tyre_width_mm": 225,
        "tyre_aspect_ratio": 900,
        "tyre_rim_diameter_in": "18"
    });
    let response = send(&app, "POST", "/parts", Some(wheel), Some(&token)).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn a_deep_dish_wheel_is_accepted() {
    // The mirror test, and the one that catches a bound set too tight. A
    // negative ET is exactly the wheel people fit.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;

    let mut wheel = a_wheel(&format!("Meister {}", Uuid::now_v7()));
    wheel["brand"] = json!("Work");
    wheel["offset_et_mm"] = json!(-25);
    let response = send(&app, "POST", "/parts", Some(wheel), Some(&token)).await;

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn a_part_with_no_specs_at_all_is_accepted_and_flagged_incomplete() {
    // AC2 flags an incomplete part; it never refuses one. Refusing would mean
    // a person who does not know their offset cannot record their wheels at
    // all — and a completeness check that quietly calls "no specs" complete
    // is the other failure mode, so both halves are asserted here.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;

    let mut wheel = a_wheel(&format!("Bare {}", Uuid::now_v7()));
    wheel["wheel_diameter_in"] = Value::Null;
    wheel["wheel_width_in"] = Value::Null;
    wheel["offset_et_mm"] = Value::Null;
    wheel["pcd_bolt_count"] = Value::Null;
    wheel["pcd_diameter_mm"] = Value::Null;
    wheel["center_bore_mm"] = Value::Null;
    let id = add(&app, &token, wheel).await;

    let body = json(send(&app, "GET", &format!("/parts/{id}"), None, Some(&token)).await).await;
    assert_eq!(body["data"]["is_complete"], false);
    assert_eq!(
        body["data"]["missing_specs"]
            .as_array()
            .expect("a list")
            .len(),
        6
    );
}

#[tokio::test]
async fn search_finds_a_part_by_its_product_name() {
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let product = format!("RPF1 {}", Uuid::now_v7());
    let id = add(&app, &token, a_wheel(&product)).await;

    // The space between "RPF1" and the UUID is not a valid URI character —
    // percent-encode it, or `Request::builder().uri(..)` refuses to parse
    // the string at all rather than exercising the search endpoint.
    let encoded_product = product.replace(' ', "%20");
    let body = json(
        send(
            &app,
            "GET",
            &format!("/parts?category=wheels&q={encoded_product}"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;

    let found = body["data"].as_array().expect("a list");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["id"], id);
}

#[tokio::test]
async fn the_daily_allowance_protects_the_curation_queue() {
    // The queue is read by a human, so flooding it denies service to
    // curation rather than to the server — the same reasoning as
    // `catalog_flow.rs`'s `one_person_cannot_flood_the_curation_queue`, and
    // the same number as `PARTS_PER_DAY` in `usecase/parts.rs`. Twenty
    // distinct wheels (varied by product name, so none of them collide with
    // each other and get folded into one row by `parts_identity`) must
    // succeed; the twenty-first must be refused with a status the client can
    // act on, not silently accepted or a 500.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let batch = Uuid::now_v7();

    for index in 0..20 {
        let response = send(
            &app,
            "POST",
            "/parts",
            Some(a_wheel(&format!("Allowance {batch} {index}"))),
            Some(&token),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "part {index} of today's allowance was refused early"
        );
    }

    let response = send(
        &app,
        "POST",
        "/parts",
        Some(a_wheel(&format!("Allowance {batch} 20"))),
        Some(&token),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "the twenty-first part today was accepted"
    );
}

#[tokio::test]
async fn parts_need_a_session() {
    let (app, _pool) = app!();
    let response = send(&app, "GET", "/parts", None, None).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_service_record_still_works_with_english_categories() {
    // The regression test for Task 1.1's rename. The normal path, unchanged.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;

    let car = json(
        send(
            &app,
            "POST",
            "/vehicles",
            Some(json!({ "described_as": "Toyota Avanza 2019" })),
            Some(&token),
        )
        .await,
    )
    .await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned();

    let response = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/services"),
        Some(json!({
            "service_date": "2026-01-10",
            "category": "engine_oil",
            "cost": "450000"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/services"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    assert_eq!(body["data"]["items"][0]["category"], "engine_oil");
}
