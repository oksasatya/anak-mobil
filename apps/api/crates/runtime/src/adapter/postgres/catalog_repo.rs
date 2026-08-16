//! Reading the vehicle catalog, and recording what is missing from it.
//!
//! The catalog is reference data: read by everyone, written by curation. None
//! of the read functions take an owner, because there is nothing here that
//! belongs to anybody.

use sqlx::PgConnection;
use uuid::Uuid;

/// A row in one level of the catalog. Every level has the same shape from a
/// client's point of view — an id, a name, and a slug for the URL — so they
/// share a type rather than repeating four near-identical structs.
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
}

/// A generation, which carries the years that let somebody pick theirs.
#[derive(Debug, Clone)]
pub struct Generation {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    /// The chassis code — FD1, W100. Often the only term a forum thread uses,
    /// and frequently what an owner recognises faster than the model year.
    pub code: Option<String>,
    pub year_start: i16,
    /// `None` means still in production.
    pub year_end: Option<i16>,
}

/// A variant, with the specifications that make fitment answerable.
#[derive(Debug, Clone)]
pub struct Variant {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub engine_code: Option<String>,
    pub displacement_cc: Option<i32>,
    pub power_hp: Option<i16>,
    pub torque_nm: Option<i16>,
    pub pcd_bolt_count: Option<i16>,
    pub pcd_diameter_mm: Option<sqlx::types::BigDecimal>,
    pub center_bore_mm: Option<sqlx::types::BigDecimal>,
    pub oem_et_mm: Option<i16>,
}

/// What somebody says is missing.
#[derive(Debug, Clone)]
pub struct SuggestionInput {
    pub brand: String,
    pub model: String,
    pub generation: Option<String>,
    pub variant: Option<String>,
    pub year: Option<i16>,
    pub note: Option<String>,
}

/// Every brand.
///
/// Unpaginated on purpose: there are a few dozen car brands sold in Indonesia
/// and the number does not grow. Paginating a list that fits in one screen
/// costs the client a loop for nothing.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn brands(conn: &mut PgConnection) -> Result<Vec<CatalogEntry>, sqlx::Error> {
    sqlx::query_as!(
        CatalogEntry,
        r#"SELECT id, name, slug FROM brands ORDER BY name"#
    )
    .fetch_all(conn)
    .await
}

/// Models under one brand.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn models(
    conn: &mut PgConnection,
    brand_id: Uuid,
) -> Result<Vec<CatalogEntry>, sqlx::Error> {
    sqlx::query_as!(
        CatalogEntry,
        r#"SELECT id, name, slug FROM vehicle_models WHERE brand_id = $1 ORDER BY name"#,
        brand_id
    )
    .fetch_all(conn)
    .await
}

/// Generations of one model, newest first.
///
/// Newest first because the person adding a car usually owns a recent one, and
/// making them scroll past the 1990s to reach their own is the kind of small
/// insult that makes an app feel careless.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn generations(
    conn: &mut PgConnection,
    model_id: Uuid,
) -> Result<Vec<Generation>, sqlx::Error> {
    sqlx::query_as!(
        Generation,
        r#"
        SELECT id, name, slug, code, year_start, year_end
        FROM vehicle_generations
        WHERE model_id = $1
        ORDER BY year_start DESC
        "#,
        model_id
    )
    .fetch_all(conn)
    .await
}

/// Variants of one generation.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn variants(
    conn: &mut PgConnection,
    generation_id: Uuid,
) -> Result<Vec<Variant>, sqlx::Error> {
    sqlx::query_as!(
        Variant,
        r#"
        SELECT
            id, name, slug, engine_code, displacement_cc, power_hp, torque_nm,
            pcd_bolt_count, pcd_diameter_mm, center_bore_mm, oem_et_mm
        FROM vehicle_variants
        WHERE generation_id = $1
        ORDER BY name
        "#,
        generation_id
    )
    .fetch_all(conn)
    .await
}

/// Does this variant exist?
///
/// Used when somebody attaches a car to the catalog, so a bad id is a
/// validation failure rather than a foreign-key error surfacing as a 500.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn variant_exists(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM vehicle_variants WHERE id = $1)"#,
        id
    )
    .fetch_one(conn)
    .await
    .map(|found| found.unwrap_or(false))
}

/// Record a gap in the catalog.
///
/// Duplicates are kept rather than merged. The same missing model reported
/// forty times is the clearest demand signal curation will ever get, and
/// folding them into one row throws it away.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn insert_suggestion(
    conn: &mut PgConnection,
    id: Uuid,
    suggested_by: Uuid,
    input: &SuggestionInput,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO catalog_suggestions
            (id, suggested_by, brand, model, generation, variant, year, note)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        id,
        suggested_by,
        input.brand,
        input.model,
        input.generation,
        input.variant,
        input.year,
        input.note,
    )
    .execute(conn)
    .await
    .map(drop)
}

/// How many suggestions this person has filed since a moment in time.
///
/// The queue is read by a human, so flooding it is a denial of service against
/// curation rather than against the server.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn suggestions_since(
    conn: &mut PgConnection,
    suggested_by: Uuid,
    since: time::OffsetDateTime,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) FROM catalog_suggestions
        WHERE suggested_by = $1 AND created_at >= $2
        "#,
        suggested_by,
        since
    )
    .fetch_one(conn)
    .await
    .map(|count| count.unwrap_or(0))
}
