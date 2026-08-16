//! Cost rollups and reminders, end to end.
//!
//! The interesting failures here are not "does it add up". They are the
//! two a batched summary gets wrong: one car's totals landing on another
//! car's row, and one person's spend landing in another person's garage.

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
use time::OffsetDateTime;
use tower::ServiceExt;

macro_rules! app {
    () => {{
        let (Ok(database_url), Ok(redis_url)) =
            (std::env::var("DATABASE_URL"), std::env::var("REDIS_URL"))
        else {
            eprintln!("SKIPPED: set DATABASE_URL and REDIS_URL to run the summary tests");
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

async fn a_signed_in_person(app: &axum::Router) -> String {
    let email = format!("summary-{}@example.com", uuid::Uuid::now_v7());
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

async fn a_car(app: &axum::Router, token: &str, described_as: &str) -> String {
    let response = send(
        app,
        "POST",
        "/vehicles",
        Some(json!({ "described_as": described_as })),
        Some(token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    json(response).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

/// `days` ago, as the API writes dates.
fn days_ago(days: i64) -> String {
    let date = OffsetDateTime::now_utc().date() - time::Duration::days(days);
    date.format(&time::format_description::well_known::Iso8601::DATE)
        .expect("formatting")
}

async fn a_service(
    app: &axum::Router,
    token: &str,
    vehicle: &str,
    category: &str,
    days: i64,
    mileage_km: Option<i32>,
    cost: Option<&str>,
) {
    let response = send(
        app,
        "POST",
        &format!("/vehicles/{vehicle}/services"),
        Some(json!({
            "service_date": days_ago(days),
            "category": category,
            "mileage_km": mileage_km,
            "cost": cost,
        })),
        Some(token),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "recording a service"
    );
}

#[tokio::test]
async fn a_summary_adds_up_and_keeps_its_scale() {
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token, "Toyota Avanza 2019").await;

    a_service(
        &app,
        &token,
        &car,
        "oli_mesin",
        30,
        Some(140_000),
        Some("450000.50"),
    )
    .await;
    a_service(
        &app,
        &token,
        &car,
        "rem",
        10,
        Some(141_000),
        Some("1200000"),
    )
    .await;

    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/summary"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let summary = &body["data"];

    assert_eq!(summary["service_count"], 2);
    assert_eq!(
        summary["total_cost"], "1650000.50",
        "exact, and two decimal places"
    );
    assert_eq!(summary["cost_last_year"], "1650000.50");
    assert_eq!(summary["odometer_km"], 141_000, "the highest reading seen");
    assert_eq!(summary["last_service_date"], days_ago(10));
}

#[tokio::test]
async fn spend_is_broken_down_by_kind_of_work() {
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token, "Honda Brio 2021").await;

    a_service(&app, &token, &car, "oli_mesin", 200, None, Some("400000")).await;
    a_service(&app, &token, &car, "oli_mesin", 20, None, Some("450000")).await;
    a_service(&app, &token, &car, "ac", 15, None, Some("300000")).await;

    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/summary"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let by_category = body["data"]["by_category"].as_array().expect("a breakdown");

    assert_eq!(by_category.len(), 2, "two kinds of work, not three records");
    assert_eq!(
        by_category[0]["category"], "oli_mesin",
        "the biggest spend first"
    );
    assert_eq!(by_category[0]["service_count"], 2);
    assert_eq!(by_category[0]["total_cost"], "850000.00");
    assert_eq!(by_category[1]["category"], "ac");
}

#[tokio::test]
async fn a_service_older_than_its_interval_is_overdue() {
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token, "Suzuki Ertiga 2018").await;

    // Oil, two years ago: six months is the interval, so this is well past.
    a_service(
        &app,
        &token,
        &car,
        "oli_mesin",
        730,
        Some(100_000),
        Some("400000"),
    )
    .await;

    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/summary"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let reminders = body["data"]["reminders"].as_array().expect("reminders");

    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0]["category"], "oli_mesin");
    assert_eq!(reminders[0]["urgency"], "overdue");
    assert!(
        reminders[0]["days_remaining"].as_i64().expect("a number") < 0,
        "the due date has passed"
    );
    assert_eq!(reminders[0]["due_at_km"], 105_000);
}

#[tokio::test]
async fn only_the_latest_service_of_a_kind_drives_its_reminder() {
    // The bug a naive query has: reminding from the first oil change ever
    // recorded rather than the most recent one.
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token, "Daihatsu Xenia 2020").await;

    a_service(
        &app,
        &token,
        &car,
        "oli_mesin",
        900,
        Some(100_000),
        Some("400000"),
    )
    .await;
    a_service(
        &app,
        &token,
        &car,
        "oli_mesin",
        5,
        Some(120_000),
        Some("450000"),
    )
    .await;

    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/summary"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let reminders = body["data"]["reminders"].as_array().expect("reminders");

    assert_eq!(
        reminders.len(),
        1,
        "one reminder per kind of work, not one per record"
    );
    assert_eq!(
        reminders[0]["urgency"], "later",
        "counted from five days ago"
    );
    assert_eq!(
        reminders[0]["due_at_km"], 125_000,
        "from the recent reading, not the old one"
    );
}

#[tokio::test]
async fn reactive_work_never_becomes_a_reminder() {
    // A dent does not recur on a schedule, and a reminder that pretends
    // it does is noise the owner learns to ignore — which costs the
    // reminders that do matter.
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token, "Mitsubishi Xpander 2022").await;

    a_service(&app, &token, &car, "body", 700, None, Some("2500000")).await;
    a_service(&app, &token, &car, "kelistrikan", 700, None, Some("800000")).await;

    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/summary"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;

    assert_eq!(
        body["data"]["reminders"]
            .as_array()
            .expect("an array")
            .len(),
        0
    );
    assert_eq!(
        body["data"]["total_cost"], "3300000.00",
        "still counted as spend"
    );
}

#[tokio::test]
async fn one_cars_totals_never_land_on_another_cars_row() {
    // This is what a batched summary gets wrong. Grouping the rollup by
    // the wrong key, or zipping two result sets that are not in the same
    // order, produces a list where every number is plausible and every
    // number is on the wrong car.
    let app = app!();
    let token = a_signed_in_person(&app).await;

    let avanza = a_car(&app, &token, "Toyota Avanza 2019").await;
    let brio = a_car(&app, &token, "Honda Brio 2021").await;
    let untouched = a_car(&app, &token, "Suzuki Jimny 2023").await;

    a_service(
        &app,
        &token,
        &avanza,
        "oli_mesin",
        20,
        Some(140_000),
        Some("450000"),
    )
    .await;
    a_service(
        &app,
        &token,
        &brio,
        "rem",
        900,
        Some(80_000),
        Some("1200000"),
    )
    .await;
    a_service(&app, &token, &brio, "ac", 10, None, Some("300000")).await;

    let body = json(send(&app, "GET", "/vehicles", None, Some(&token)).await).await;
    let rows = body["data"].as_array().expect("a list");
    assert_eq!(rows.len(), 3);

    let of = |id: &str| -> Value {
        rows.iter()
            .find(|row| row["id"] == id)
            .expect("the car is in the list")["summary"]
            .clone()
    };

    assert_eq!(of(&avanza)["service_count"], 1);
    assert_eq!(of(&avanza)["total_cost"], "450000.00");
    assert_eq!(of(&avanza)["overdue_count"], 0);

    assert_eq!(of(&brio)["service_count"], 2);
    assert_eq!(of(&brio)["total_cost"], "1500000.00");
    assert_eq!(
        of(&brio)["overdue_count"],
        1,
        "brakes, from two and a half years ago"
    );

    // A car with no history is a zeroed row, not a missing field. A
    // client that has to special-case an absent summary will get it
    // wrong on the one screen a new user actually sees.
    assert_eq!(of(&untouched)["service_count"], 0);
    assert_eq!(of(&untouched)["total_cost"], Value::Null);
    assert_eq!(of(&untouched)["overdue_count"], 0);
}

#[tokio::test]
async fn a_stranger_sees_nothing_of_your_spending() {
    let app = app!();
    let owner = a_signed_in_person(&app).await;
    let car = a_car(&app, &owner, "Toyota Fortuner 2023").await;
    a_service(
        &app,
        &owner,
        &car,
        "oli_mesin",
        20,
        Some(50_000),
        Some("900000"),
    )
    .await;

    let stranger = a_signed_in_person(&app).await;
    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/summary"),
            None,
            Some(&stranger),
        )
        .await,
    )
    .await;

    assert_eq!(body["data"]["service_count"], 0);
    assert_eq!(body["data"]["total_cost"], Value::Null);
    assert_eq!(
        body["data"]["by_category"]
            .as_array()
            .expect("an array")
            .len(),
        0
    );
    assert_eq!(
        body["data"]["reminders"]
            .as_array()
            .expect("an array")
            .len(),
        0
    );
}

#[tokio::test]
async fn a_summary_needs_a_session() {
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token, "Toyota Avanza 2019").await;

    let response = send(&app, "GET", &format!("/vehicles/{car}/summary"), None, None).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn spend_outside_the_last_year_counts_in_the_lifetime_total_only() {
    let app = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token, "Nissan Livina 2020").await;

    a_service(&app, &token, &car, "tune_up", 400, None, Some("2000000")).await;
    a_service(&app, &token, &car, "oli_mesin", 30, None, Some("450000")).await;

    let body = json(
        send(
            &app,
            "GET",
            &format!("/vehicles/{car}/summary"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;

    assert_eq!(body["data"]["total_cost"], "2450000.00", "everything, ever");
    assert_eq!(
        body["data"]["cost_last_year"], "450000.00",
        "the last twelve months only"
    );
}
