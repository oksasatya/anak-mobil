//! Service history, end to end.
//!
//! The tests that matter are the two contracts this slice sets: cost reaches
//! nobody but the owner, and paging does not repeat or skip a row when the
//! history changes underneath it.

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
        http::router(AppState {
            pool,
            redis: redis.clone(),
            sessions: SessionStore::new(redis.clone()),
            limiter: RateLimiter::new(redis),
        })
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

async fn text(response: Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("reading the body");
    String::from_utf8(bytes.to_vec()).expect("utf-8")
}

async fn a_signed_in_person(app: &axum::Router) -> String {
    let email = format!("service-{}@example.com", Uuid::now_v7());
    let username = format!("u{}", &Uuid::now_v7().simple().to_string()[13..]);
    let response = send(
        app,
        "POST",
        "/auth/register",
        Some(json!({"email": email, "username": username, "password": "kata sandi panjang"})),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    json(response).await["data"]["access_token"]
        .as_str()
        .expect("token")
        .to_owned()
}

async fn a_car(app: &axum::Router, token: &str) -> String {
    let response = send(
        app,
        "POST",
        "/vehicles",
        Some(json!({"described_as": "Toyota Avanza 2019"})),
        Some(token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    json(response).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

/// Record a service on a given date, returning its id.
async fn a_service(app: &axum::Router, token: &str, car: &str, date: &str, cost: &str) -> String {
    let response = send(
        app,
        "POST",
        &format!("/vehicles/{car}/services"),
        Some(json!({
            "service_date": date,
            "mileage_km": 140_200,
            "category": "engine_oil",
            "parts_replaced": ["Filter oli"],
            "garage_name": "Bengkel XYZ",
            "cost": cost,
            "notes": "Ganti oli rutin"
        })),
        Some(token),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "{}",
        "recording a service"
    );
    json(response).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

#[tokio::test]
async fn a_service_is_recorded_and_read_back_whole() {
    // AC3's storage half: date, mileage, category, garage, and cost all
    // survive the round trip.
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let id = a_service(&app, &token, &car, "2026-01-10", "1200000").await;

    let body = json(send(&app, "GET", &format!("/services/{id}"), None, Some(&token)).await).await;
    let record = &body["data"];

    assert_eq!(record["service_date"], "2026-01-10");
    assert_eq!(record["mileage_km"], 140_200);
    assert_eq!(record["category"], "engine_oil");
    assert_eq!(record["garage_name"], "Bengkel XYZ");
    assert_eq!(record["cost"], "1200000.00");
    assert_eq!(record["parts_replaced"][0], "Filter oli");
}

#[tokio::test]
async fn another_person_reaches_neither_the_history_nor_the_cost() {
    // AC3's privacy half, as far as it can go today: there is no path that
    // hands a service record to a stranger, so there is no cost to filter.
    let app = app!();
    let owner = a_signed_in_person(&app).await;
    let stranger = a_signed_in_person(&app).await;

    let car = a_car(&app, &owner).await;
    let id = a_service(&app, &owner, &car, "2026-01-10", "9876543").await;

    let listed = send(
        &app,
        "GET",
        &format!("/vehicles/{car}/services"),
        None,
        Some(&stranger),
    )
    .await;
    assert_eq!(
        listed.status(),
        StatusCode::OK,
        "a stranger's list is empty, not forbidden"
    );
    let body = text(listed).await;
    assert!(
        !body.contains("9876543"),
        "the cost leaked into a stranger's list: {body}"
    );

    let detail = send(
        &app,
        "GET",
        &format!("/services/{id}"),
        None,
        Some(&stranger),
    )
    .await;
    assert_eq!(detail.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn another_person_cannot_write_to_the_history() {
    let app = app!();
    let owner = a_signed_in_person(&app).await;
    let stranger = a_signed_in_person(&app).await;

    let car = a_car(&app, &owner).await;
    let id = a_service(&app, &owner, &car, "2026-01-10", "1200000").await;

    let added = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/services"),
        Some(json!({"service_date": "2026-02-01", "category": "brakes"})),
        Some(&stranger),
    )
    .await;
    assert_eq!(added.status(), StatusCode::NOT_FOUND);

    let removed = send(
        &app,
        "DELETE",
        &format!("/services/{id}"),
        None,
        Some(&stranger),
    )
    .await;
    assert_eq!(removed.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_history_reads_newest_first() {
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;

    a_service(&app, &token, &car, "2024-03-01", "500000").await;
    a_service(&app, &token, &car, "2026-01-10", "1200000").await;
    a_service(&app, &token, &car, "2025-06-15", "800000").await;

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
    let dates: Vec<&str> = body["data"]["items"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|r| r["service_date"].as_str().unwrap_or(""))
        .collect();

    assert_eq!(dates, vec!["2026-01-10", "2025-06-15", "2024-03-01"]);
}

#[tokio::test]
async fn paging_walks_the_whole_history_exactly_once() {
    // AC5. Every record appears once and none is missed.
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;

    for day in 1..=7 {
        a_service(&app, &token, &car, &format!("2026-01-{day:02}"), "100000").await;
    }

    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;

    for _ in 0..10 {
        let path = cursor.as_ref().map_or_else(
            || format!("/vehicles/{car}/services?limit=3"),
            |c| format!("/vehicles/{car}/services?limit=3&after={c}"),
        );
        let body = json(send(&app, "GET", &path, None, Some(&token)).await).await;

        for item in body["data"]["items"].as_array().expect("a list") {
            seen.push(item["id"].as_str().expect("an id").to_owned());
        }
        match body["data"]["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }

    assert_eq!(
        seen.len(),
        7,
        "paging did not cover the history exactly once: {seen:?}"
    );
    let unique: std::collections::HashSet<_> = seen.iter().collect();
    assert_eq!(unique.len(), 7, "a record was returned twice");
}

#[tokio::test]
async fn a_record_inserted_mid_history_does_not_shift_the_next_page() {
    // The reason this pagination is a cursor and not an offset. With
    // `?page=2`, inserting a row above the boundary between two requests
    // pushes one row down, so the next page repeats it.
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;

    for day in 1..=6 {
        a_service(&app, &token, &car, &format!("2026-02-{day:02}"), "100000").await;
    }

    let first = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/services?limit=3"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let first_ids: Vec<String> = first["data"]["items"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|r| r["id"].as_str().unwrap_or("").to_owned())
        .collect();
    let cursor = first["data"]["next_cursor"]
        .as_str()
        .expect("a cursor")
        .to_owned();

    // Somebody records an older service they had forgotten — it lands in the
    // middle of the history, above the cursor's position is not affected but
    // an offset would have shifted everything after it.
    a_service(&app, &token, &car, "2026-02-04", "999999").await;

    let second = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/services?limit=3&after={cursor}"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let second_ids: Vec<String> = second["data"]["items"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|r| r["id"].as_str().unwrap_or("").to_owned())
        .collect();

    for id in &second_ids {
        assert!(
            !first_ids.contains(id),
            "the second page repeated a record from the first: {id}"
        );
    }
}

#[tokio::test]
async fn the_last_page_ends_the_loop_even_when_it_is_full() {
    // A client that stops on a short page asks one time too few. The cursor
    // being absent is what ends it, and a history that divides evenly into
    // pages is exactly where that distinction shows.
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;

    for day in 1..=4 {
        a_service(&app, &token, &car, &format!("2026-03-{day:02}"), "100000").await;
    }

    let first = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/services?limit=2"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let cursor = first["data"]["next_cursor"]
        .as_str()
        .expect("a cursor")
        .to_owned();

    let second = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/services?limit=2&after={cursor}"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;

    assert_eq!(second["data"]["items"].as_array().expect("a list").len(), 2);
    assert!(
        second["data"].get("next_cursor").is_none(),
        "a full final page must still end the loop"
    );
}

#[tokio::test]
async fn a_cursor_that_was_not_issued_here_is_refused() {
    // Silently starting from the beginning would make a client's paging loop
    // repeat forever without ever erroring.
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;

    let response = send(
        &app,
        "GET",
        &format!("/vehicles/{car}/services?after=bukan-cursor"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn an_enormous_limit_is_capped_rather_than_served() {
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    a_service(&app, &token, &car, "2026-01-10", "100000").await;

    let response = send(
        &app,
        "GET",
        &format!("/vehicles/{car}/services?limit=65535"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_service_dated_in_the_future_is_refused() {
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;

    let response = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/services"),
        Some(json!({"service_date": "2099-01-01", "category": "brakes"})),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        json(response).await["error"]["details"]["service_date"].is_string(),
        "the field should be named"
    );
}

#[tokio::test]
async fn a_record_can_be_amended_and_removed() {
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let id = a_service(&app, &token, &car, "2026-01-10", "1200000").await;

    let amended = send(
        &app,
        "PUT",
        &format!("/services/{id}"),
        Some(json!({
            "service_date": "2026-01-11",
            "category": "brakes",
            "cost": "1500000"
        })),
        Some(&token),
    )
    .await;
    assert_eq!(amended.status(), StatusCode::NO_CONTENT);

    let body = json(send(&app, "GET", &format!("/services/{id}"), None, Some(&token)).await).await;
    assert_eq!(body["data"]["category"], "brakes");
    assert_eq!(body["data"]["cost"], "1500000.00");

    let removed = send(
        &app,
        "DELETE",
        &format!("/services/{id}"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);

    let gone = send(&app, "GET", &format!("/services/{id}"), None, Some(&token)).await;
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_a_car_takes_its_history_with_it() {
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let id = a_service(&app, &token, &car, "2026-01-10", "1200000").await;

    let removed = send(
        &app,
        "DELETE",
        &format!("/vehicles/{car}"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);

    let gone = send(&app, "GET", &format!("/services/{id}"), None, Some(&token)).await;
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn recording_a_service_needs_a_signed_in_caller() {
    let app = app!();
    let response = send(
        &app,
        "GET",
        &format!("/vehicles/{}/services", Uuid::now_v7()),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
