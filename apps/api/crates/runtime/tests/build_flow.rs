//! Builds and modifications, end to end.
//!
//! Slice 2's earlier tasks were verified by compilation and unit tests only.
//! This is the one place that proves the endpoints behave: a build keeps what
//! its owner put on it while a modification is attached to it, an unknown
//! part typed into a modification is usable at once and lands in the
//! curation queue the same way a typed part does through `POST /parts`, a
//! failed write leaves no orphan behind, removal is a timestamp rather than a
//! deletion, and a stranger gets the same 404 a nonexistent build would give.

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

// Same harness as `tests/parts_flow.rs`, with the email prefix changed to
// `build-`. Nothing here reads the pool directly — every check goes back
// through the router — so every test binds it as `_pool` rather than
// dropping the tuple return, which is what keeps this copy identical to the
// original rather than a slightly different macro.
macro_rules! app {
    () => {{
        let (Ok(database_url), Ok(redis_url)) =
            (std::env::var("DATABASE_URL"), std::env::var("REDIS_URL"))
        else {
            eprintln!("SKIPPED: set DATABASE_URL and REDIS_URL to run the build tests");
            return;
        };
        let Ok(pool) = anakmobil_runtime::adapter::postgres::connect(&database_url) else {
            eprintln!("SKIPPED: DATABASE_URL unusable");
            return;
        };
        if anakmobil_runtime::adapter::postgres::migrate::run(&pool)
            .await
            .is_err()
        {
            eprintln!("SKIPPED: could not migrate the test database");
            return;
        }
        let Ok(redis) = anakmobil_runtime::adapter::redis::connect(&redis_url).await else {
            eprintln!("SKIPPED: REDIS_URL unreachable");
            return;
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

/// Registers a fresh person and returns their access token and email.
async fn a_signed_in_person(app: &axum::Router) -> (String, String) {
    let email = format!("build-{}@example.com", Uuid::now_v7());
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

/// A described car, with no catalog match — enough to own a build.
async fn a_car(app: &axum::Router, token: &str) -> String {
    let response = send(
        app,
        "POST",
        "/vehicles",
        Some(json!({ "described_as": "Toyota Avanza 2019" })),
        Some(token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED, "adding a car");
    json(response).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

/// A wheel nobody else in this run will have typed — the same shape
/// `parts_flow.rs` uses, so the range checks are the ones already proven
/// there.
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

/// Add a part directly through `/parts`, and return its id — the `part_id`
/// half of a modification's choice.
async fn an_existing_part(app: &axum::Router, token: &str, product: &str) -> String {
    let response = send(app, "POST", "/parts", Some(a_wheel(product)), Some(token)).await;
    assert_eq!(response.status(), StatusCode::CREATED, "adding a part");
    json(response).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

/// Add a modification against an already-catalogued part, and return the
/// modification's own id — never the part's, which the endpoint does not
/// return.
async fn add_modification(app: &axum::Router, token: &str, car: &str, part_id: &str) -> String {
    let response = send(
        app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({ "part_id": part_id })),
        Some(token),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "adding a modification"
    );
    json(response).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

#[tokio::test]
async fn a_build_carries_its_modifications_notes_and_visibility() {
    // AC1, the whole of it — and the order is the point. Notes and visibility
    // are set FIRST, then a modification is added, then both are re-read.
    // This is the exact bug `build_repo::ensure_build` exists to prevent:
    // `add_modification` must call `ensure_build`, never `upsert_build`, or
    // attaching a part silently wipes the owner's notes and resets a public
    // build back to private. Adding the modification before setting notes
    // would never catch a regression that swapped the two calls.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let part = an_existing_part(&app, &token, &format!("RPF1 {}", Uuid::now_v7())).await;

    let saved = send(
        &app,
        "PUT",
        &format!("/vehicles/{car}/build"),
        Some(json!({
            "notes": "Daily driver, slowly turning into a track car",
            "visibility": "community"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(saved.status(), StatusCode::NO_CONTENT);

    let added = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({
            "part_id": part,
            "install_date": "2026-01-10",
            "mileage_km": 42_000,
            "cost": "1200000.75",
            "garage_name": "Bengkel ABC",
            "notes": "Ganti velg"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(added.status(), StatusCode::CREATED);
    let modification_id = json(added).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned();

    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/build"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    assert_eq!(
        body["data"]["notes"], "Daily driver, slowly turning into a track car",
        "adding a modification wiped the build's notes"
    );
    assert_eq!(
        body["data"]["visibility"], "community",
        "adding a modification reset the build's visibility"
    );

    let modifications = body["data"]["modifications"].as_array().expect("a list");
    assert_eq!(modifications.len(), 1);
    let modification = &modifications[0];
    assert_eq!(modification["id"], modification_id);
    assert_eq!(modification["part_id"], part);
    assert_eq!(modification["install_date"], "2026-01-10");
    assert_eq!(modification["mileage_km"], 42_000);
    assert_eq!(modification["cost"], "1200000.75");
    assert_eq!(modification["garage_name"], "Bengkel ABC");
    assert_eq!(modification["notes"], "Ganti velg");
    assert!(
        modification.get("removed_at").is_none(),
        "a fresh modification must not carry a removal time"
    );
}

#[tokio::test]
async fn an_unknown_part_typed_into_a_modification_is_usable_and_queued() {
    // AC3 end to end, which slice 1 could only half-prove: a part typed
    // straight into a modification is usable at once (the modification is
    // created) and it lands in the curation queue in the same breath (the
    // part it resolves to reads back as "pending"). The POST response only
    // carries the modification's own id, so the part_id is read back through
    // the build detail — checking only the immediate response would miss the
    // second half entirely.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;

    let added = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({ "part": a_wheel(&format!("RPF1 {}", Uuid::now_v7())) })),
        Some(&token),
    )
    .await;
    assert_eq!(added.status(), StatusCode::CREATED);

    // The build did not exist before this call — `ensure_build` created it on
    // demand.
    let build = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/build"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let part_id = build["data"]["modifications"][0]["part_id"]
        .as_str()
        .expect("a part id")
        .to_owned();

    let part = json(
        send(
            &app,
            "GET",
            &format!("/parts/{part_id}"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    assert_eq!(part["data"]["status"], "pending");
}

#[tokio::test]
async fn a_failed_modification_leaves_no_orphan_part_in_the_queue() {
    // The transaction boundary. Post a modification against somebody else's
    // car with an inline new part; the call is a 404, and a search for that
    // part's product name must return nothing. Without one transaction, the
    // part row survives the rolled-back modification and the curation queue
    // fills with parts for builds that do not exist.
    let (app, _pool) = app!();
    let (owner, _owner_email) = a_signed_in_person(&app).await;
    let (stranger, _stranger_email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &owner).await;
    let product = format!("Orphan {}", Uuid::now_v7());

    let response = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({ "part": a_wheel(&product) })),
        Some(&stranger),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let encoded_product = product.replace(' ', "%20");
    let found = json(
        send(
            &app,
            "GET",
            &format!("/parts?category=wheels&q={encoded_product}"),
            None,
            Some(&owner),
        )
        .await,
    )
    .await;
    assert_eq!(
        found["data"].as_array().expect("a list").len(),
        0,
        "a modification that failed still left a pending part behind"
    );
}

#[tokio::test]
async fn both_a_part_id_and_an_inline_part_is_refused() {
    // Picking one for the caller hides their bug until the wrong part is
    // attached to somebody's build.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let part = an_existing_part(&app, &token, &format!("RPF1 {}", Uuid::now_v7())).await;

    let response = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({
            "part_id": part,
            "part": a_wheel(&format!("RPF1 {}", Uuid::now_v7())),
        })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn a_removed_modification_is_still_there_with_a_removal_time() {
    // Removal sets removed_at rather than deleting: a removed modification is
    // still evidence the part fitted, and slice 3's second count depends on
    // the row surviving.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let part = an_existing_part(&app, &token, &format!("RPF1 {}", Uuid::now_v7())).await;
    let modification_id = add_modification(&app, &token, &car, &part).await;

    let removed = send(
        &app,
        "DELETE",
        &format!("/modifications/{modification_id}"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);

    let build = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/build"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let modification = &build["data"]["modifications"][0];
    assert_eq!(modification["id"], modification_id);
    assert!(
        modification["removed_at"].as_str().is_some(),
        "a removed modification must still be listed, with a removal time"
    );
}

#[tokio::test]
async fn removing_twice_is_not_an_error() {
    // A client retrying a request that already succeeded must not get a 404
    // for work that was done — and the removal time must not move on the
    // second call either, or a retry would silently change when the part
    // "came off".
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let part = an_existing_part(&app, &token, &format!("RPF1 {}", Uuid::now_v7())).await;
    let modification_id = add_modification(&app, &token, &car, &part).await;

    let first = send(
        &app,
        "DELETE",
        &format!("/modifications/{modification_id}"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(first.status(), StatusCode::NO_CONTENT);
    let removed_at_first = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/build"),
            None,
            Some(&token),
        )
        .await,
    )
    .await["data"]["modifications"][0]["removed_at"]
        .as_str()
        .expect("a removal time")
        .to_owned();

    let second = send(
        &app,
        "DELETE",
        &format!("/modifications/{modification_id}"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(
        second.status(),
        StatusCode::NO_CONTENT,
        "a retried removal must not 404"
    );

    let removed_at_second = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/build"),
            None,
            Some(&token),
        )
        .await,
    )
    .await["data"]["modifications"][0]["removed_at"]
        .as_str()
        .expect("a removal time")
        .to_owned();
    assert_eq!(
        removed_at_first, removed_at_second,
        "removing an already-removed modification moved its removal time"
    );
}

#[tokio::test]
async fn a_stranger_cannot_read_or_write_your_build() {
    // 404 for both reading and writing — the same answer as a build that does
    // not exist. A distinct 403 would tell a caller which ids are real.
    let (app, _pool) = app!();
    let (owner, _owner_email) = a_signed_in_person(&app).await;
    let (stranger, _stranger_email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &owner).await;
    let part = an_existing_part(&app, &owner, &format!("RPF1 {}", Uuid::now_v7())).await;
    let modification_id = add_modification(&app, &owner, &car, &part).await;

    let read = send(
        &app,
        "GET",
        &format!("/vehicles/{car}/build"),
        None,
        Some(&stranger),
    )
    .await;
    assert_eq!(read.status(), StatusCode::NOT_FOUND);

    let write = send(
        &app,
        "PUT",
        &format!("/vehicles/{car}/build"),
        Some(json!({ "notes": "not yours" })),
        Some(&stranger),
    )
    .await;
    assert_eq!(write.status(), StatusCode::NOT_FOUND);

    let add = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({ "part_id": part })),
        Some(&stranger),
    )
    .await;
    assert_eq!(add.status(), StatusCode::NOT_FOUND);

    let update = send(
        &app,
        "PUT",
        &format!("/modifications/{modification_id}"),
        Some(json!({ "part_id": part })),
        Some(&stranger),
    )
    .await;
    assert_eq!(update.status(), StatusCode::NOT_FOUND);

    let remove = send(
        &app,
        "DELETE",
        &format!("/modifications/{modification_id}"),
        None,
        Some(&stranger),
    )
    .await;
    assert_eq!(remove.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cost_visibility_defaults_to_private_and_round_trips() {
    // An absent field must mean private. A default of "community" would widen
    // who sees somebody's money without them asking.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;

    let body = json(
        send(&app, "GET", &format!("/vehicles/{car}"), None, Some(&token)).await,
    )
    .await;
    assert_eq!(body["data"]["cost_visibility"], "private");

    let updated = send(
        &app,
        "PUT",
        &format!("/vehicles/{car}"),
        Some(json!({
            "described_as": "Toyota Avanza 2019",
            "cost_visibility": "community"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::NO_CONTENT);

    let body = json(
        send(&app, "GET", &format!("/vehicles/{car}"), None, Some(&token)).await,
    )
    .await;
    assert_eq!(body["data"]["cost_visibility"], "community");
}

#[tokio::test]
async fn a_vehicle_still_lists_and_updates_as_it_did() {
    // The regression test for the vehicle_repo columns added in Task 2.3: the
    // garage list and the vehicle update are unchanged apart from the new
    // cost_visibility field.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;

    let listed = json(send(&app, "GET", "/vehicles", None, Some(&token)).await).await;
    let vehicles = listed["data"].as_array().expect("a list");
    assert!(
        vehicles.iter().any(|v| v["id"] == car),
        "the new car is missing from the garage list"
    );

    let updated = send(
        &app,
        "PUT",
        &format!("/vehicles/{car}"),
        Some(json!({
            "described_as": "Toyota Avanza 2019",
            "nickname": "Si Merah"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::NO_CONTENT);

    let body = json(
        send(&app, "GET", &format!("/vehicles/{car}"), None, Some(&token)).await,
    )
    .await;
    assert_eq!(body["data"]["nickname"], "Si Merah");
    assert_eq!(body["data"]["name"], "Si Merah");
}

#[tokio::test]
async fn an_ordinary_cost_is_accepted_and_round_trips() {
    // A suite that only asserts rejection passes on a guard that rejects
    // everything. A whole-number cost — the shape most people actually type
    // — must be accepted and come back exact.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let part = an_existing_part(&app, &token, &format!("RPF1 {}", Uuid::now_v7())).await;

    let response = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({ "part_id": part, "cost": "1200000" })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/build"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    assert_eq!(body["data"]["modifications"][0]["cost"], "1200000.00");
}

#[tokio::test]
async fn a_cost_with_too_many_decimals_is_refused_not_rounded() {
    // The defect this closes, proven through the router rather than only
    // against `ModificationRequest::check` directly: a bare parse let
    // "1200000.755" through, Postgres rounded it silently on insert, and the
    // idempotent re-read this endpoint relies on elsewhere would then
    // disagree with what the caller sent.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let part = an_existing_part(&app, &token, &format!("RPF1 {}", Uuid::now_v7())).await;

    let response = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({ "part_id": part, "cost": "1200000.755" })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn a_cost_past_the_columns_range_is_refused_not_a_500() {
    // `modifications.cost` is `NUMERIC(14, 2)`. A value the column cannot
    // hold at all must be a 422 caught before the database, not a 500 from a
    // `22003 numeric field overflow`.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let part = an_existing_part(&app, &token, &format!("RPF1 {}", Uuid::now_v7())).await;

    let response = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({ "part_id": part, "cost": "9999999999999999" })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn removed_at_cannot_be_set_by_the_client() {
    // `removed_at` is set server-side by `mark_modification_removed`, never
    // from a request body. `ModificationRequest` has no field for it, so a
    // caller who sends one must be silently ignored rather than trusted.
    let (app, _pool) = app!();
    let (token, _email) = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let part = an_existing_part(&app, &token, &format!("RPF1 {}", Uuid::now_v7())).await;

    let response = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({ "part_id": part, "removed_at": "2020-01-01T00:00:00Z" })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/build"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    assert!(
        body["data"]["modifications"][0].get("removed_at").is_none(),
        "a client-supplied removed_at reached the stored modification"
    );
}

#[tokio::test]
async fn build_endpoints_need_a_session() {
    let (app, _pool) = app!();
    let response = send(
        &app,
        "GET",
        &format!("/vehicles/{}/build", Uuid::now_v7()),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
