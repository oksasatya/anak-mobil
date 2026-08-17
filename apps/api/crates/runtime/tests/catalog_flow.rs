//! Catalog browsing and the suggestion queue, end to end.

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

macro_rules! setup {
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

async fn a_signed_in_person(app: &axum::Router) -> String {
    let email = format!("catalog-{}@example.com", Uuid::now_v7());
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
        .expect("token")
        .to_owned()
}

/// Seed one full chain — brand, model, generation, variant — with real
/// specifications, and return their ids.
async fn a_catalogued_car(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let suffix = Uuid::now_v7().simple().to_string();
    let (brand, model, generation, variant) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );

    sqlx::query!(
        "INSERT INTO brands (id, name, slug) VALUES ($1, $2, $3)",
        brand,
        format!("Toyota {suffix}"),
        format!("toyota-{suffix}")
    )
    .execute(pool)
    .await
    .expect("seeding a brand");

    sqlx::query!(
        "INSERT INTO vehicle_models (id, brand_id, name, slug) VALUES ($1, $2, $3, $4)",
        model,
        brand,
        "Avanza",
        "avanza"
    )
    .execute(pool)
    .await
    .expect("seeding a model");

    sqlx::query!(
        "INSERT INTO vehicle_generations (id, model_id, name, slug, code, year_start, year_end)
         VALUES ($1, $2, $3, $4, $5, $6, NULL)",
        generation,
        model,
        "W100",
        "w100",
        "W100",
        2021_i16
    )
    .execute(pool)
    .await
    .expect("seeding a generation");

    sqlx::query!(
        "INSERT INTO vehicle_variants
            (id, generation_id, name, slug, engine_code, displacement_cc,
             pcd_bolt_count, pcd_diameter_mm, center_bore_mm, oem_et_mm)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        variant,
        generation,
        "1.5 G CVT",
        "1-5-g-cvt",
        "2NR-VE",
        1496_i32,
        5_i16,
        sqlx::types::BigDecimal::from(1143) / sqlx::types::BigDecimal::from(10),
        sqlx::types::BigDecimal::from(671) / sqlx::types::BigDecimal::from(10),
        40_i16
    )
    .execute(pool)
    .await
    .expect("seeding a variant");

    (brand, model, generation, variant)
}

#[tokio::test]
async fn the_catalog_walks_from_brand_to_variant() {
    let (app, pool) = setup!();
    let token = a_signed_in_person(&app).await;
    let (brand, model, generation, variant) = a_catalogued_car(&pool).await;

    let brands = json(send(&app, "GET", "/catalog/brands", None, Some(&token)).await).await;
    assert!(
        brands["data"]
            .as_array()
            .expect("a list")
            .iter()
            .any(|b| b["id"] == brand.to_string()),
        "the seeded brand is missing from the list"
    );

    let models = json(
        send(
            &app,
            "GET",
            &format!("/catalog/brands/{brand}/models"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    assert_eq!(models["data"][0]["id"], model.to_string());

    let generations = json(
        send(
            &app,
            "GET",
            &format!("/catalog/models/{model}/generations"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    assert_eq!(generations["data"][0]["id"], generation.to_string());

    let variants = json(
        send(
            &app,
            "GET",
            &format!("/catalog/generations/{generation}/variants"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    assert_eq!(variants["data"][0]["id"], variant.to_string());
}

#[tokio::test]
async fn a_generation_still_in_production_shows_an_open_range() {
    let (app, pool) = setup!();
    let token = a_signed_in_person(&app).await;
    let (_, model, _, _) = a_catalogued_car(&pool).await;

    let body = json(
        send(
            &app,
            "GET",
            &format!("/catalog/models/{model}/generations"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;

    assert_eq!(body["data"][0]["years"], "2021–");
    assert_eq!(body["data"][0]["code"], "W100");
}

#[tokio::test]
async fn a_variant_carries_its_fitment_numbers() {
    // AC4 of AM-353 pays off here: the specifications arrive as numbers a
    // client can compare, plus one preformatted string so three clients do not
    // each invent their own way of writing a bolt pattern.
    let (app, pool) = setup!();
    let token = a_signed_in_person(&app).await;
    let (_, _, generation, _) = a_catalogued_car(&pool).await;

    let body = json(
        send(
            &app,
            "GET",
            &format!("/catalog/generations/{generation}/variants"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let variant = &body["data"][0];

    assert_eq!(variant["pcd_bolt_count"], 5);
    assert_eq!(variant["pcd_diameter_mm"], "114.3");
    assert_eq!(variant["center_bore_mm"], "67.1");
    assert_eq!(variant["oem_et_mm"], 40);
    assert_eq!(variant["displacement_cc"], 1496);
    assert_eq!(variant["engine_code"], "2NR-VE");
    // Written the way a person writes it, not "5x114.30".
    assert_eq!(variant["pcd"], "5x114.3");
}

#[tokio::test]
async fn an_unknown_parent_yields_an_empty_level_rather_than_an_error() {
    // The client is walking a tree it was just handed. An empty level is a
    // real answer, and it treats "no models" and "no brand" the same way.
    let (app, _pool) = setup!();
    let token = a_signed_in_person(&app).await;

    let body = json(
        send(
            &app,
            "GET",
            &format!("/catalog/brands/{}/models", Uuid::now_v7()),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    assert_eq!(body["data"].as_array().expect("a list").len(), 0);
}

#[tokio::test]
async fn the_catalog_needs_a_signed_in_caller() {
    let (app, _pool) = setup!();
    let response = send(&app, "GET", "/catalog/brands", None, None).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_missing_model_can_be_reported() {
    // AC2. Without this, somebody whose car is not catalogued has no way to
    // say so, and the catalog can only ever grow by guesswork.
    let (app, _pool) = setup!();
    let token = a_signed_in_person(&app).await;

    let response = send(
        &app,
        "POST",
        "/catalog/suggestions",
        Some(json!({"brand": "Datsun", "model": "Go+ Panca", "year": 2018})),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(json(response).await["data"]["id"].is_string());
}

#[tokio::test]
async fn a_suggestion_needs_a_brand_and_a_model() {
    let (app, _pool) = setup!();
    let token = a_signed_in_person(&app).await;

    let response = send(
        &app,
        "POST",
        "/catalog/suggestions",
        Some(json!({"brand": "  ", "model": "Go+ Panca"})),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(json(response).await["error"]["details"]["brand"].is_string());
}

#[tokio::test]
async fn the_same_gap_reported_twice_is_kept_twice() {
    // Deliberate. Forty reports of one missing model is the clearest demand
    // signal curation will ever get, and merging them throws it away.
    let (app, pool) = setup!();
    let token = a_signed_in_person(&app).await;
    let model = format!("Go+ Panca {}", Uuid::now_v7());

    for _ in 0..2 {
        let response = send(
            &app,
            "POST",
            "/catalog/suggestions",
            Some(json!({"brand": "Datsun", "model": model})),
            Some(&token),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let filed = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM catalog_suggestions WHERE model = $1",
        model
    )
    .fetch_one(&pool)
    .await
    .expect("counting");
    assert_eq!(filed, Some(2));
}

#[tokio::test]
async fn one_person_cannot_flood_the_curation_queue() {
    // The queue is read by a human, so flooding it denies service to curation
    // rather than to the server.
    let (app, _pool) = setup!();
    let token = a_signed_in_person(&app).await;

    let mut throttled = false;
    for index in 0..15 {
        let response = send(
            &app,
            "POST",
            "/catalog/suggestions",
            Some(json!({"brand": "Merek", "model": format!("Model {index}")})),
            Some(&token),
        )
        .await;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            throttled = true;
            break;
        }
    }

    assert!(
        throttled,
        "fifteen suggestions from one account were all accepted"
    );
}

#[tokio::test]
async fn a_suggestion_survives_the_account_that_filed_it() {
    // What was reported is catalog work, not personal data. Losing it because
    // somebody later closed their account would throw away the only record
    // that the gap exists.
    let (app, pool) = setup!();
    let token = a_signed_in_person(&app).await;
    let model = format!("Panca {}", Uuid::now_v7());

    send(
        &app,
        "POST",
        "/catalog/suggestions",
        Some(json!({"brand": "Datsun", "model": model})),
        Some(&token),
    )
    .await;

    let owner: Uuid = sqlx::query_scalar!(
        "SELECT suggested_by FROM catalog_suggestions WHERE model = $1",
        model
    )
    .fetch_one(&pool)
    .await
    .expect("reading the suggestion")
    .expect("a suggester");

    sqlx::query!("DELETE FROM users WHERE id = $1", owner)
        .execute(&pool)
        .await
        .expect("deleting the account");

    let still_there: Option<Uuid> = sqlx::query_scalar!(
        "SELECT suggested_by FROM catalog_suggestions WHERE model = $1",
        model
    )
    .fetch_one(&pool)
    .await
    .expect("the suggestion should still exist");

    assert!(
        still_there.is_none(),
        "the link to the deleted account must be cleared"
    );
}
