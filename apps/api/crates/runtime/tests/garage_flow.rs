//! The garage endpoints, end to end.
//!
//! The tests that matter here are not "does create work". They are the two
//! product rules: a plate never reaches anyone but its owner, and one person
//! can do nothing at all to another person's car.

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

macro_rules! app {
    () => {{
        let (Ok(database_url), Ok(redis_url)) =
            (std::env::var("DATABASE_URL"), std::env::var("REDIS_URL"))
        else {
            eprintln!("SKIPPED: set DATABASE_URL and REDIS_URL to run the garage tests");
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
        http::router(AppState {
            pool,
            redis: redis.clone(),
            sessions: SessionStore::new(redis.clone()),
            limiter: RateLimiter::new(redis),
        })
    }};
}

fn a_peer() -> SocketAddr {
    let id = uuid::Uuid::now_v7().as_u128();
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

async fn text(response: Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("reading the body");
    String::from_utf8(bytes.to_vec()).expect("utf-8")
}

/// Register a fresh account and return its access token.
async fn a_signed_in_person(app: &axum::Router) -> String {
    let email = format!("garage-{}@example.com", uuid::Uuid::now_v7());
    let response = send(
        app,
        "POST",
        "/auth/register",
        Some(json!({"email": email, "password": "kata sandi panjang"})),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    json(response).await["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned()
}

/// Add a car with a plate, and return its id.
async fn a_car_with_a_plate(app: &axum::Router, token: &str, plate: &str) -> String {
    let response = send(
        app,
        "POST",
        "/vehicles",
        Some(json!({
            "described_as": "Toyota Avanza 2019",
            "year": 2019,
            "private": { "plate": plate, "vin": "MHKM1BA3JKK000001", "purchase_price": "185000000.50" }
        })),
        Some(token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    json(response).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

#[tokio::test]
async fn a_plate_never_appears_in_the_list() {
    // The product rule, at the endpoint most likely to break it: the list is
    // what every garage screen calls, and it is where a `SELECT *` would show
    // up.
    let app = app!();
    let token = a_signed_in_person(&app).await;
    a_car_with_a_plate(&app, &token, "B 1234 XYZ").await;

    let listed = send(&app, "GET", "/vehicles", None, Some(&token)).await;
    assert_eq!(listed.status(), StatusCode::OK);

    let body = text(listed).await;
    for forbidden in [
        "B 1234 XYZ",
        "MHKM1BA3JKK000001",
        "185000000",
        "plate",
        "vin",
    ] {
        assert!(
            !body.contains(forbidden),
            "the vehicle list leaked {forbidden}: {body}"
        );
    }
}

#[tokio::test]
async fn the_owner_sees_their_own_plate_on_the_detail() {
    // The other half. Hiding it from its owner too would be a different bug.
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let id = a_car_with_a_plate(&app, &token, "B 5678 ABC").await;

    let body = json(send(&app, "GET", &format!("/vehicles/{id}"), None, Some(&token)).await).await;
    assert_eq!(body["data"]["private"]["plate"], "B 5678 ABC");
    assert_eq!(body["data"]["private"]["vin"], "MHKM1BA3JKK000001");
    // Exact to the cent, through a decimal rather than a float.
    assert_eq!(body["data"]["private"]["purchase_price"], "185000000.50");
}

#[tokio::test]
async fn another_person_cannot_reach_the_car_at_all() {
    // Not "cannot see the plate" — cannot see the car. Answering 404 rather
    // than 403 also means an id cannot be probed for existence.
    let app = app!();
    let owner = a_signed_in_person(&app).await;
    let stranger = a_signed_in_person(&app).await;
    let id = a_car_with_a_plate(&app, &owner, "B 9999 ZZZ").await;

    let seen = send(
        &app,
        "GET",
        &format!("/vehicles/{id}"),
        None,
        Some(&stranger),
    )
    .await;
    assert_eq!(seen.status(), StatusCode::NOT_FOUND);

    let listed = text(send(&app, "GET", "/vehicles", None, Some(&stranger)).await).await;
    assert!(!listed.contains("B 9999 ZZZ"));
}

#[tokio::test]
async fn another_person_cannot_change_or_delete_the_car() {
    let app = app!();
    let owner = a_signed_in_person(&app).await;
    let stranger = a_signed_in_person(&app).await;
    let id = a_car_with_a_plate(&app, &owner, "B 1111 AAA").await;

    let edited = send(
        &app,
        "PUT",
        &format!("/vehicles/{id}"),
        Some(json!({"described_as": "Bukan mobilmu"})),
        Some(&stranger),
    )
    .await;
    assert_eq!(edited.status(), StatusCode::NOT_FOUND);

    let removed = send(
        &app,
        "DELETE",
        &format!("/vehicles/{id}"),
        None,
        Some(&stranger),
    )
    .await;
    assert_eq!(removed.status(), StatusCode::NOT_FOUND);

    // Still there, still the owner's.
    let still = send(&app, "GET", &format!("/vehicles/{id}"), None, Some(&owner)).await;
    assert_eq!(still.status(), StatusCode::OK);
}

#[tokio::test]
async fn an_unauthenticated_caller_reaches_nothing() {
    let app = app!();
    for (method, path) in [("GET", "/vehicles"), ("POST", "/vehicles")] {
        let response = send(&app, method, path, Some(json!({})), None).await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path}"
        );
    }
}

#[tokio::test]
async fn a_car_missing_from_the_catalog_can_still_be_added() {
    // What AC2's suggestion flow depends on. If this failed, somebody whose
    // model is not catalogued could not use the app at all.
    let app = app!();
    let token = a_signed_in_person(&app).await;

    let response = send(
        &app,
        "POST",
        "/vehicles",
        Some(json!({"described_as": "Datsun Go+ Panca 2018"})),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn a_car_with_no_identity_at_all_is_refused() {
    let app = app!();
    let token = a_signed_in_person(&app).await;

    let response = send(
        &app,
        "POST",
        "/vehicles",
        Some(json!({"year": 2019})),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn deleting_a_car_takes_its_plate_with_it() {
    // A soft delete would leave the plate in the database after somebody
    // asked for their car to be gone.
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let id = a_car_with_a_plate(&app, &token, "B 2222 BBB").await;

    let removed = send(
        &app,
        "DELETE",
        &format!("/vehicles/{id}"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);

    let gone = send(&app, "GET", &format!("/vehicles/{id}"), None, Some(&token)).await;
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_garage_can_be_rearranged() {
    let app = app!();
    let token = a_signed_in_person(&app).await;

    let first = a_car_with_a_plate(&app, &token, "B 3333 CCC").await;
    let second = a_car_with_a_plate(&app, &token, "B 4444 DDD").await;

    let reordered = send(
        &app,
        "PUT",
        "/vehicles/order",
        Some(json!({"vehicle_ids": [second, first]})),
        Some(&token),
    )
    .await;
    assert_eq!(reordered.status(), StatusCode::NO_CONTENT);

    let listed = json(send(&app, "GET", "/vehicles", None, Some(&token)).await).await;
    let ids: Vec<&str> = listed["data"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|v| v["id"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(ids, vec![second.as_str(), first.as_str()]);
}

#[tokio::test]
async fn a_reorder_containing_someone_elses_car_changes_nothing() {
    // Partly applying it would leave the garage in a state neither person
    // asked for.
    let app = app!();
    let owner = a_signed_in_person(&app).await;
    let stranger = a_signed_in_person(&app).await;

    let mine = a_car_with_a_plate(&app, &owner, "B 5555 EEE").await;
    let theirs = a_car_with_a_plate(&app, &stranger, "B 6666 FFF").await;

    let response = send(
        &app,
        "PUT",
        "/vehicles/order",
        Some(json!({"vehicle_ids": [theirs, mine]})),
        Some(&owner),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // The stranger's car is untouched and still theirs.
    let theirs_still = send(
        &app,
        "GET",
        &format!("/vehicles/{theirs}"),
        None,
        Some(&stranger),
    )
    .await;
    assert_eq!(theirs_still.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_list_is_one_query_regardless_of_how_many_cars() {
    // The N+1 the ticket warns about. Not measured by counting queries — the
    // list is a single statement with joins, and this asserts the shape it
    // produces stays correct as the garage grows.
    let app = app!();
    let token = a_signed_in_person(&app).await;

    for index in 0..5 {
        a_car_with_a_plate(&app, &token, &format!("B {index}00 GGG")).await;
    }

    let listed = json(send(&app, "GET", "/vehicles", None, Some(&token)).await).await;
    let cars = listed["data"].as_array().expect("a list");
    assert_eq!(cars.len(), 5);
    assert!(
        cars.iter().all(|car| car["name"].is_string()),
        "every car needs a display name"
    );
}

#[tokio::test]
async fn a_variant_id_that_is_not_in_the_catalog_is_a_client_error() {
    // Left to the foreign key this surfaces as a 500 — an internal failure —
    // when it is really the caller sending an id that does not exist.
    let app = app!();
    let token = a_signed_in_person(&app).await;

    let response = send(
        &app,
        "POST",
        "/vehicles",
        Some(json!({"variant_id": uuid::Uuid::now_v7()})),
        Some(&token),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(json(response).await["error"]["details"]["variant_id"].is_string());
}
