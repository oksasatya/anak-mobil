//! The batched build list, end to end.
//!
//! `usecase::builds::page` fetches every build's modifications and photos in
//! three queries total rather than one per build — and a batch is exactly
//! where grouping by the wrong key, leaking the has-more probe row, or
//! filtering the wrong side of a `LEFT JOIN` goes unnoticed until it is wrong
//! on a real page. This file proves the failures that shape actually has:
//! one build's parts landing on another's row, a private build leaking
//! through the list or through a stranger's view of its own cost, the
//! has-more probe row repeating a build across pages, a cursor that lets a
//! stranger learn a private build exists, and the two evidence counts on
//! `parts` disagreeing with who is looking and which car they are compared
//! against.
//!
//! The query count itself is not asserted here. Counting queries needs
//! `pg_stat_statements`, which needs `shared_preload_libraries` and a server
//! restart, which a GitHub Actions service container cannot give it — such a
//! test would pass locally and silently skip in CI, which is worse than not
//! having it. What is tested instead is the failure batching actually
//! produces, exactly as `apps/api/CLAUDE.md` describes for the service
//! rollups.

#![expect(
    clippy::expect_used,
    reason = "test helpers abort rather than propagate"
)]

use anakmobil_runtime::adapter::http;
use anakmobil_runtime::adapter::postgres::build_repo;
use anakmobil_runtime::adapter::redis::rate_limit::RateLimiter;
use anakmobil_runtime::adapter::redis::session::SessionStore;
use anakmobil_runtime::platform::state::AppState;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::HashSet;
use std::net::SocketAddr;
use tower::ServiceExt;
use uuid::Uuid;

// Same harness as `tests/build_flow.rs`, copied rather than shared — each
// integration test file is its own binary, so there is nothing to import
// from.
macro_rules! app {
    () => {{
        let (Ok(database_url), Ok(redis_url)) =
            (std::env::var("DATABASE_URL"), std::env::var("REDIS_URL"))
        else {
            eprintln!("SKIPPED: set DATABASE_URL and REDIS_URL to run the build list tests");
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

/// Registers a fresh person and returns their access token.
async fn a_signed_in_person(app: &axum::Router) -> String {
    let email = format!("build-list-{}@example.com", Uuid::now_v7());
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

/// A described car, with no catalog match.
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

/// A car attached to a real catalog variant, rather than free text — what the
/// variant-filtered evidence-count tests need to have something to filter by.
async fn a_car_with_variant(app: &axum::Router, token: &str, variant_id: Uuid) -> String {
    let response = send(
        app,
        "POST",
        "/vehicles",
        Some(json!({ "variant_id": variant_id })),
        Some(token),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "adding a catalogued car"
    );
    json(response).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
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

/// Add a part directly through `/parts`, and return its id.
async fn an_existing_part(app: &axum::Router, token: &str, product: &str) -> String {
    let response = send(app, "POST", "/parts", Some(a_wheel(product)), Some(token)).await;
    assert_eq!(response.status(), StatusCode::CREATED, "adding a part");
    json(response).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

/// Add a modification against an already-catalogued part, and return the
/// modification's own id.
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

/// A private build with no modifications — enough to occupy a row in
/// `GET /builds` without the cost of adding a part. Returns the build's own
/// id, read back through the owner-only detail endpoint.
async fn a_build(app: &axum::Router, token: &str) -> String {
    let car = a_car(app, token).await;
    let saved = send(
        app,
        "PUT",
        &format!("/vehicles/{car}/build"),
        Some(json!({})),
        Some(token),
    )
    .await;
    assert_eq!(saved.status(), StatusCode::NO_CONTENT);
    json(
        send(
            app,
            "GET",
            &format!("/vehicles/{car}/build"),
            None,
            Some(token),
        )
        .await,
    )
    .await["data"]["id"]
        .as_str()
        .expect("an id")
        .to_owned()
}

/// Every build this caller can see, walking every page rather than trusting
/// page one. The suite runs concurrently against one shared database, and a
/// handful of community-visible builds from unrelated tests can land ahead
/// of a build this test just made — this walks until `next_cursor` is
/// exhausted instead of assuming a fresh build sits on the first page.
async fn all_visible_builds(app: &axum::Router, token: &str) -> Vec<Value> {
    let mut items = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let path = after.as_ref().map_or_else(
            || "/builds".to_owned(),
            |cursor| format!("/builds?after={cursor}"),
        );
        let body = json(send(app, "GET", &path, None, Some(token)).await).await;
        items.extend(body["data"]["items"].as_array().expect("a list").clone());
        after = body["data"]["next_cursor"].as_str().map(ToOwned::to_owned);
        if after.is_none() {
            break;
        }
    }
    items
}

/// Seed one full catalog chain — brand, model, generation, variant — with a
/// unique slug per call, and return only the variant id: the piece these
/// tests read back. Mirrors `catalog_flow.rs`'s `a_catalogued_car`, trimmed
/// to what this file actually needs.
async fn a_catalog_variant(pool: &PgPool) -> Uuid {
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
        format!("Brand {suffix}"),
        format!("brand-{suffix}"),
    )
    .execute(pool)
    .await
    .expect("seeding a brand");

    sqlx::query!(
        "INSERT INTO vehicle_models (id, brand_id, name, slug) VALUES ($1, $2, $3, $4)",
        model,
        brand,
        format!("Model {suffix}"),
        format!("model-{suffix}"),
    )
    .execute(pool)
    .await
    .expect("seeding a model");

    sqlx::query!(
        "INSERT INTO vehicle_generations (id, model_id, name, slug, year_start)
         VALUES ($1, $2, $3, $4, $5)",
        generation,
        model,
        format!("Generation {suffix}"),
        format!("gen-{suffix}"),
        2020_i16,
    )
    .execute(pool)
    .await
    .expect("seeding a generation");

    sqlx::query!(
        "INSERT INTO vehicle_variants (id, generation_id, name, slug)
         VALUES ($1, $2, $3, $4)",
        variant,
        generation,
        format!("Variant {suffix}"),
        format!("variant-{suffix}"),
    )
    .execute(pool)
    .await
    .expect("seeding a variant");

    variant
}

#[tokio::test]
async fn one_builds_modifications_never_land_on_another_builds_row() {
    // This is what a batched list gets wrong. Grouping by the wrong key, or
    // zipping two result sets that are not in the same order, produces a
    // list where every build is plausible and every part is on the wrong
    // car. Three builds: one with two modifications, one with one, one with
    // none.
    let (app, _pool) = app!();
    let token = a_signed_in_person(&app).await;

    let car_two = a_car(&app, &token).await;
    let part_a = an_existing_part(&app, &token, &format!("Two-A {}", Uuid::now_v7())).await;
    let part_b = an_existing_part(&app, &token, &format!("Two-B {}", Uuid::now_v7())).await;
    let mod_a = add_modification(&app, &token, &car_two, &part_a).await;
    let mod_b = add_modification(&app, &token, &car_two, &part_b).await;

    let car_one = a_car(&app, &token).await;
    let part_c = an_existing_part(&app, &token, &format!("One-C {}", Uuid::now_v7())).await;
    let mod_c = add_modification(&app, &token, &car_one, &part_c).await;

    let car_zero = a_car(&app, &token).await;
    let saved = send(
        &app,
        "PUT",
        &format!("/vehicles/{car_zero}/build"),
        Some(json!({})),
        Some(&token),
    )
    .await;
    assert_eq!(saved.status(), StatusCode::NO_CONTENT);

    let items = all_visible_builds(&app, &token).await;
    let find = |vehicle_id: &str| -> Value {
        items
            .iter()
            .find(|item| item["vehicle_id"] == vehicle_id)
            .unwrap_or_else(|| panic!("build for vehicle {vehicle_id} is missing from the list"))
            .clone()
    };

    let mods_two: HashSet<String> = find(&car_two)["modifications"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|m| m["id"].as_str().expect("an id").to_owned())
        .collect();
    assert_eq!(
        mods_two,
        HashSet::from([mod_a, mod_b]),
        "car_two's build carried the wrong modifications"
    );

    let mods_one: Vec<String> = find(&car_one)["modifications"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|m| m["id"].as_str().expect("an id").to_owned())
        .collect();
    assert_eq!(
        mods_one,
        vec![mod_c],
        "car_one's build carried the wrong modification"
    );

    assert_eq!(
        find(&car_zero)["modifications"]
            .as_array()
            .expect("a list")
            .len(),
        0,
        "car_zero's build must have no modifications"
    );
}

#[tokio::test]
async fn a_private_build_is_absent_from_a_strangers_list() {
    // Absent, not present-with-fields-blanked. A blanked row still tells a
    // stranger the build exists.
    let (app, _pool) = app!();
    let owner = a_signed_in_person(&app).await;
    let stranger = a_signed_in_person(&app).await;
    let car = a_car(&app, &owner).await;
    let part = an_existing_part(&app, &owner, &format!("Hidden {}", Uuid::now_v7())).await;
    add_modification(&app, &owner, &car, &part).await;

    let items = all_visible_builds(&app, &stranger).await;
    assert!(
        !items.iter().any(|item| item["vehicle_id"] == car),
        "a stranger's list carried a private build"
    );
}

#[tokio::test]
async fn a_stranger_sees_no_modification_costs_when_the_car_keeps_them_private() {
    // AM-360 AC3's other half, now configured rather than true by
    // construction. The build is visible; the money is not.
    let (app, _pool) = app!();
    let owner = a_signed_in_person(&app).await;
    let stranger = a_signed_in_person(&app).await;
    // cost_visibility is left at its default (private) — a_car sets no
    // opinion about it, and neither does this test.
    let car = a_car(&app, &owner).await;
    let part = an_existing_part(&app, &owner, &format!("Priced {}", Uuid::now_v7())).await;

    let saved = send(
        &app,
        "PUT",
        &format!("/vehicles/{car}/build"),
        Some(json!({ "visibility": "community" })),
        Some(&owner),
    )
    .await;
    assert_eq!(saved.status(), StatusCode::NO_CONTENT);

    let added = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({ "part_id": part, "cost": "1500000" })),
        Some(&owner),
    )
    .await;
    assert_eq!(added.status(), StatusCode::CREATED);

    // The owner check must come first: a private cost_visibility must never
    // hide a person's own numbers from themselves.
    let owner_items = all_visible_builds(&app, &owner).await;
    let owner_build = owner_items
        .iter()
        .find(|item| item["vehicle_id"] == car)
        .expect("the owner's own build is missing from their list");
    assert_eq!(
        owner_build["modifications"][0]["cost"], "1500000.00",
        "cost_visibility hid an owner's own numbers from themselves"
    );

    let stranger_items = all_visible_builds(&app, &stranger).await;
    let stranger_build = stranger_items
        .iter()
        .find(|item| item["vehicle_id"] == car)
        .expect("the community build is missing from a stranger's list");
    assert!(
        stranger_build["modifications"][0]["cost"].is_null(),
        "a stranger saw a cost the car's owner kept private"
    );
}

#[tokio::test]
async fn both_counts_are_zero_for_a_part_nobody_has_fitted() {
    // A row of zeros, not a missing row. The empty catalog is the state this
    // platform launches in, and a client special-casing an absent count will
    // get it wrong on the one screen a new user actually sees. Checked on
    // both endpoints that carry the counts.
    let (app, _pool) = app!();
    let token = a_signed_in_person(&app).await;
    let product = format!("Unfitted {}", Uuid::now_v7());
    let part = an_existing_part(&app, &token, &product).await;

    let detail = json(send(&app, "GET", &format!("/parts/{part}"), None, Some(&token)).await).await;
    assert_eq!(detail["data"]["active_build_count"], 0);
    assert_eq!(detail["data"]["ever_installed_build_count"], 0);

    let encoded = product.replace(' ', "%20");
    let searched = json(
        send(
            &app,
            "GET",
            &format!("/parts?category=wheels&q={encoded}"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let found = searched["data"]
        .as_array()
        .expect("a list")
        .iter()
        .find(|p| p["id"] == part)
        .expect("the seeded part is missing from search");
    assert_eq!(found["active_build_count"], 0);
    assert_eq!(found["ever_installed_build_count"], 0);
}

#[tokio::test]
async fn a_removed_part_still_counts_as_ever_installed() {
    // The whole reason removal sets removed_at instead of deleting the row.
    // active_build_count drops to 0; ever_installed_build_count stays 1.
    let (app, _pool) = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let part = an_existing_part(&app, &token, &format!("Taken off {}", Uuid::now_v7())).await;
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

    let detail = json(send(&app, "GET", &format!("/parts/{part}"), None, Some(&token)).await).await;
    assert_eq!(detail["data"]["active_build_count"], 0);
    assert_eq!(detail["data"]["ever_installed_build_count"], 1);
}

#[tokio::test]
async fn one_build_fitting_a_part_twice_counts_once() {
    // COUNT(DISTINCT v.id), not COUNT(*). Somebody who replaced their wheels
    // with the same model is one piece of evidence, not two.
    let (app, _pool) = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await;
    let part = an_existing_part(&app, &token, &format!("Twice {}", Uuid::now_v7())).await;
    add_modification(&app, &token, &car, &part).await;
    add_modification(&app, &token, &car, &part).await;

    let detail = json(send(&app, "GET", &format!("/parts/{part}"), None, Some(&token)).await).await;
    assert_eq!(detail["data"]["active_build_count"], 1);
    assert_eq!(detail["data"]["ever_installed_build_count"], 1);
}

#[tokio::test]
async fn evidence_from_a_different_car_does_not_count_towards_this_variant() {
    // An Avanza build is not evidence about a Civic. Two vehicles on two
    // catalog variants, both fitting the same wheel; the count filtered by
    // one variant returns 1, not 2.
    //
    // No HTTP endpoint exposes the variant filter yet — nothing in this
    // slice ships a fitment-check surface — so this calls `evidence_counts`
    // directly, the way it will be called once that surface exists.
    let (app, pool) = app!();
    let token = a_signed_in_person(&app).await;
    let variant_a = a_catalog_variant(&pool).await;
    let variant_b = a_catalog_variant(&pool).await;
    let car_a = a_car_with_variant(&app, &token, variant_a).await;
    let car_b = a_car_with_variant(&app, &token, variant_b).await;

    // Community so the raw repo call below, with an arbitrary viewer, still
    // sees both — visibility is not what this test is about.
    for car in [&car_a, &car_b] {
        let saved = send(
            &app,
            "PUT",
            &format!("/vehicles/{car}/build"),
            Some(json!({ "visibility": "community" })),
            Some(&token),
        )
        .await;
        assert_eq!(saved.status(), StatusCode::NO_CONTENT);
    }

    let part = an_existing_part(&app, &token, &format!("Fitment {}", Uuid::now_v7())).await;
    let part_id = Uuid::parse_str(&part).expect("a uuid");
    add_modification(&app, &token, &car_a, &part).await;
    add_modification(&app, &token, &car_b, &part).await;

    let mut conn = pool.acquire().await.expect("acquiring a connection");
    let viewer = Uuid::now_v7();

    let for_a = build_repo::evidence_counts(&mut conn, &[part_id], viewer, Some(variant_a))
        .await
        .expect("counting")
        .into_iter()
        .next()
        .expect("a row for the part");
    assert_eq!(
        for_a.active_build_count, 1,
        "the count filtered by variant_a leaked evidence from variant_b's car"
    );

    let for_b = build_repo::evidence_counts(&mut conn, &[part_id], viewer, Some(variant_b))
        .await
        .expect("counting")
        .into_iter()
        .next()
        .expect("a row for the part");
    assert_eq!(
        for_b.active_build_count, 1,
        "the count filtered by variant_b leaked evidence from variant_a's car"
    );
}

#[tokio::test]
async fn a_car_with_no_catalog_variant_counts_towards_no_variant() {
    // variant_id is nullable because free-text cars exist. A NULL variant
    // must match no filter — matching every filter would make an
    // unidentified car evidence for every car.
    let (app, pool) = app!();
    let token = a_signed_in_person(&app).await;
    let car = a_car(&app, &token).await; // no catalog match: variant_id is NULL
    let saved = send(
        &app,
        "PUT",
        &format!("/vehicles/{car}/build"),
        Some(json!({ "visibility": "community" })),
        Some(&token),
    )
    .await;
    assert_eq!(saved.status(), StatusCode::NO_CONTENT);

    let part = an_existing_part(&app, &token, &format!("Free text {}", Uuid::now_v7())).await;
    let part_id = Uuid::parse_str(&part).expect("a uuid");
    add_modification(&app, &token, &car, &part).await;

    let some_variant = a_catalog_variant(&pool).await;
    let mut conn = pool.acquire().await.expect("acquiring a connection");
    let viewer = Uuid::now_v7();

    let filtered = build_repo::evidence_counts(&mut conn, &[part_id], viewer, Some(some_variant))
        .await
        .expect("counting")
        .into_iter()
        .next()
        .expect("a row for the part");
    assert_eq!(
        filtered.active_build_count, 0,
        "a free-text car's NULL variant matched a filter it must never match"
    );

    let unfiltered = build_repo::evidence_counts(&mut conn, &[part_id], viewer, None)
        .await
        .expect("counting")
        .into_iter()
        .next()
        .expect("a row for the part");
    assert_eq!(
        unfiltered.active_build_count, 1,
        "the unfiltered count must still include a free-text car's evidence"
    );
}

#[tokio::test]
async fn the_list_pages_without_repeating_or_skipping_a_build() {
    // Walk limit=2 pages to the end and assert the three builds made here
    // each turn up exactly once. A grouping bug or an off-by-one in the
    // cursor either drops one of the three or repeats one across pages.
    let (app, _pool) = app!();
    let token = a_signed_in_person(&app).await;
    let made: Vec<String> = {
        let mut ids = Vec::new();
        for _ in 0..3 {
            ids.push(a_build(&app, &token).await);
        }
        ids
    };

    let mut seen: Vec<String> = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..50_u32 {
        let path = after.as_ref().map_or_else(
            || "/builds?limit=2".to_owned(),
            |cursor| format!("/builds?limit=2&after={cursor}"),
        );
        let body = json(send(&app, "GET", &path, None, Some(&token)).await).await;
        seen.extend(
            body["data"]["items"]
                .as_array()
                .expect("a list")
                .iter()
                .map(|item| item["id"].as_str().expect("an id").to_owned()),
        );
        after = body["data"]["next_cursor"].as_str().map(ToOwned::to_owned);
        if after.is_none() {
            break;
        }
    }

    for id in &made {
        assert_eq!(
            seen.iter().filter(|s| *s == id).count(),
            1,
            "build {id} did not appear exactly once across the pages walked"
        );
    }
}

#[tokio::test]
async fn a_page_of_twenty_returns_twenty_items_not_twenty_one() {
    // `page_visible` fetches limit + 1 as a has-more probe. Keeping the
    // extra row on the page it belongs to makes a page of 20 come back as
    // 21; keeping it on the next page's cursor repeats a real, visible build
    // at the top of the next page.
    let (app, _pool) = app!();
    let token = a_signed_in_person(&app).await;
    for _ in 0..21 {
        a_build(&app, &token).await;
    }

    let page1 = json(send(&app, "GET", "/builds", None, Some(&token)).await).await;
    let items1 = page1["data"]["items"].as_array().expect("a list");
    assert_eq!(
        items1.len(),
        20,
        "a page must never carry the has-more probe row"
    );
    let cursor = page1["data"]["next_cursor"]
        .as_str()
        .expect("more than twenty builds exist, so a next page must be offered")
        .to_owned();

    let page2 = json(
        send(
            &app,
            "GET",
            &format!("/builds?after={cursor}"),
            None,
            Some(&token),
        )
        .await,
    )
    .await;
    let items2 = page2["data"]["items"].as_array().expect("a list");

    let ids1: HashSet<&str> = items1
        .iter()
        .map(|item| item["id"].as_str().expect("an id"))
        .collect();
    for item in items2 {
        let id = item["id"].as_str().expect("an id");
        assert!(
            !ids1.contains(id),
            "build {id} appeared on both pages — the probe row leaked into the next page"
        );
    }
}

#[tokio::test]
async fn the_cursor_cannot_be_used_to_probe() {
    // Paging with a private build's id as the cursor must be
    // indistinguishable from paging with an id that was never issued —
    // otherwise a stranger learns a private build exists by watching the
    // page shift under a cursor that should mean nothing to them.
    let (app, _pool) = app!();
    let owner = a_signed_in_person(&app).await;
    let stranger = a_signed_in_person(&app).await;
    let build_id = a_build(&app, &owner).await;

    // Adjacent to the real, private id — one bit off, so it was never handed
    // out by `Uuid::now_v7()` — but close enough in sort order that no real
    // row can exist between the two.
    let real = Uuid::parse_str(&build_id).expect("a uuid");
    let mut bytes = *real.as_bytes();
    let last = bytes.len() - 1;
    bytes[last] = bytes[last].wrapping_sub(1);
    let phantom = Uuid::from_bytes(bytes);

    let with_real = json(
        send(
            &app,
            "GET",
            &format!("/builds?after={build_id}"),
            None,
            Some(&stranger),
        )
        .await,
    )
    .await;
    let with_phantom = json(
        send(
            &app,
            "GET",
            &format!("/builds?after={phantom}"),
            None,
            Some(&stranger),
        )
        .await,
    )
    .await;

    assert_eq!(
        with_real["data"], with_phantom["data"],
        "paging past a real private build differs from paging past an id nobody issued"
    );
}
