//! Catalog endpoints.
//!
//! Four read levels that walk brand → model → generation → variant, and one
//! write: the report that a car is missing.
//!
//! Authenticated, like everything else. The catalog is not secret, but the
//! only consumer today is the signed-in app, and opening an unauthenticated
//! surface means giving it its own throttling and its own abuse story for no
//! caller that exists. Making it public later is additive.

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapter::http::auth::Authenticated;
use crate::adapter::postgres::catalog_repo::SuggestionInput;
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::response::ApiResponse;
use crate::usecase::catalog::{self, CatalogError};

#[derive(Debug, Serialize)]
pub struct EntryResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
}

impl From<crate::adapter::postgres::catalog_repo::CatalogEntry> for EntryResponse {
    fn from(entry: crate::adapter::postgres::catalog_repo::CatalogEntry) -> Self {
        Self {
            id: entry.id,
            name: entry.name,
            slug: entry.slug,
        }
    }
}

/// A brand, with where its logo comes from.
///
/// `logo_domain` is a DOMAIN and not a URL, and the client composes the rest.
/// The server has no opinion about the size, the format, the CDN, or the API
/// key any one client needs, and encoding one here would make every client
/// take the mobile app's answer. See the migration that added the column.
#[derive(Debug, Serialize)]
pub struct BrandResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub logo_domain: Option<String>,
}

impl From<crate::adapter::postgres::catalog_repo::Brand> for BrandResponse {
    fn from(brand: crate::adapter::postgres::catalog_repo::Brand) -> Self {
        Self {
            id: brand.id,
            name: brand.name,
            slug: brand.slug,
            logo_domain: brand.logo_domain,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GenerationResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub code: Option<String>,
    pub year_start: i16,
    pub year_end: Option<i16>,
    /// Prebuilt so three clients do not each invent their own way of writing
    /// "2021–" versus "2015–2021".
    pub years: String,
}

impl From<crate::adapter::postgres::catalog_repo::Generation> for GenerationResponse {
    fn from(g: crate::adapter::postgres::catalog_repo::Generation) -> Self {
        let years = g.year_end.map_or_else(
            || format!("{}–", g.year_start),
            |end| format!("{}–{end}", g.year_start),
        );
        Self {
            id: g.id,
            name: g.name,
            slug: g.slug,
            code: g.code,
            year_start: g.year_start,
            year_end: g.year_end,
            years,
        }
    }
}

/// A variant, with the numbers the fitment engine compares.
///
/// The decimals cross the wire as strings for the same reason a price does:
/// a JSON number is a double in most clients, and a bolt circle of 114.3 is
/// not a value to round.
#[derive(Debug, Serialize)]
pub struct VariantResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub engine_code: Option<String>,
    pub displacement_cc: Option<i32>,
    pub power_hp: Option<i16>,
    pub torque_nm: Option<i16>,
    pub pcd_bolt_count: Option<i16>,
    pub pcd_diameter_mm: Option<String>,
    pub center_bore_mm: Option<String>,
    pub oem_et_mm: Option<i16>,
    /// `5x114.3`, assembled once here rather than in every client that wants
    /// to show it.
    pub pcd: Option<String>,
}

impl From<crate::adapter::postgres::catalog_repo::Variant> for VariantResponse {
    fn from(v: crate::adapter::postgres::catalog_repo::Variant) -> Self {
        let pcd = match (v.pcd_bolt_count, v.pcd_diameter_mm.as_ref()) {
            (Some(count), Some(diameter)) => Some(format!("{count}x{}", trim_decimal(diameter))),
            _ => None,
        };

        Self {
            id: v.id,
            name: v.name,
            slug: v.slug,
            engine_code: v.engine_code,
            displacement_cc: v.displacement_cc,
            power_hp: v.power_hp,
            torque_nm: v.torque_nm,
            pcd_bolt_count: v.pcd_bolt_count,
            pcd_diameter_mm: v.pcd_diameter_mm.as_ref().map(trim_decimal),
            center_bore_mm: v.center_bore_mm.as_ref().map(trim_decimal),
            oem_et_mm: v.oem_et_mm,
            pcd,
        }
    }
}

/// Render a decimal without a trailing zero: `114.3`, and `100` rather than
/// `100.0`. A `NUMERIC(5,1)` always comes back with its scale attached, and
/// "PCD 5x100.0" is not how anybody writes it.
fn trim_decimal(value: &sqlx::types::BigDecimal) -> String {
    let rendered = value.to_string();
    if rendered.contains('.') {
        rendered
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    } else {
        rendered
    }
}

#[derive(Debug, Deserialize)]
pub struct SuggestionRequest {
    pub brand: String,
    pub model: String,
    pub generation: Option<String>,
    pub variant: Option<String>,
    pub year: Option<i16>,
    pub note: Option<String>,
}

impl SuggestionRequest {
    fn check(&self) -> Result<SuggestionInput, ApiError> {
        let brand = self.brand.trim();
        let model = self.model.trim();

        let mut problems = serde_json::Map::new();
        if brand.is_empty() {
            problems.insert("brand".into(), "Wajib diisi.".into());
        }
        if model.is_empty() {
            problems.insert("model".into(), "Wajib diisi.".into());
        }
        if let Some(year) = self.year
            && !(1900..=2100).contains(&year)
        {
            problems.insert("year".into(), "Tahun tidak masuk akal.".into());
        }

        if !problems.is_empty() {
            return Err(ApiError::validation(serde_json::Value::Object(problems)));
        }

        Ok(SuggestionInput {
            brand: brand.to_owned(),
            model: model.to_owned(),
            generation: self
                .generation
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
            variant: self
                .variant
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
            year: self.year,
            note: self
                .note
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
        })
    }
}

/// `GET /catalog/brands`
///
/// # Errors
///
/// A storage failure.
pub async fn brands(
    State(state): State<AppState>,
    _caller: Authenticated,
) -> Result<ApiResponse<Vec<BrandResponse>>, ApiError> {
    let entries = catalog::brands(&state.pool).await.map_err(to_api_error)?;
    Ok(ApiResponse::ok(
        entries.into_iter().map(Into::into).collect(),
    ))
}

/// `GET /catalog/brands/{id}/models`
///
/// # Errors
///
/// A storage failure.
pub async fn models(
    State(state): State<AppState>,
    _caller: Authenticated,
    Path(brand_id): Path<Uuid>,
) -> Result<ApiResponse<Vec<EntryResponse>>, ApiError> {
    let entries = catalog::models(&state.pool, brand_id)
        .await
        .map_err(to_api_error)?;
    Ok(ApiResponse::ok(
        entries.into_iter().map(Into::into).collect(),
    ))
}

/// `GET /catalog/models/{id}/generations`
///
/// # Errors
///
/// A storage failure.
pub async fn generations(
    State(state): State<AppState>,
    _caller: Authenticated,
    Path(model_id): Path<Uuid>,
) -> Result<ApiResponse<Vec<GenerationResponse>>, ApiError> {
    let entries = catalog::generations(&state.pool, model_id)
        .await
        .map_err(to_api_error)?;
    Ok(ApiResponse::ok(
        entries.into_iter().map(Into::into).collect(),
    ))
}

/// `GET /catalog/generations/{id}/variants`
///
/// # Errors
///
/// A storage failure.
pub async fn variants(
    State(state): State<AppState>,
    _caller: Authenticated,
    Path(generation_id): Path<Uuid>,
) -> Result<ApiResponse<Vec<VariantResponse>>, ApiError> {
    let entries = catalog::variants(&state.pool, generation_id)
        .await
        .map_err(to_api_error)?;
    Ok(ApiResponse::ok(
        entries.into_iter().map(Into::into).collect(),
    ))
}

/// `POST /catalog/suggestions`
///
/// # Errors
///
/// Validation failure, today's allowance spent, or a storage failure.
pub async fn suggest(
    State(state): State<AppState>,
    caller: Authenticated,
    Json(body): Json<SuggestionRequest>,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    let input = body.check()?;
    let id = catalog::suggest(&state.pool, caller.user_id, &input)
        .await
        .map_err(to_api_error)?;

    Ok(ApiResponse::created(serde_json::json!({ "id": id })))
}

fn to_api_error(err: CatalogError) -> ApiError {
    match err {
        CatalogError::TooManySuggestions => ApiError::too_many_requests(),
        CatalogError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn a_suggestion() -> SuggestionRequest {
        SuggestionRequest {
            brand: "Datsun".to_owned(),
            model: "Go+ Panca".to_owned(),
            generation: None,
            variant: None,
            year: Some(2018),
            note: None,
        }
    }

    #[test]
    fn a_suggestion_needs_a_brand_and_a_model() {
        let mut request = a_suggestion();
        request.brand = "  ".to_owned();
        assert!(request.check().is_err());

        let mut request = a_suggestion();
        request.model = String::new();
        assert!(request.check().is_err());
    }

    #[test]
    fn an_implausible_year_is_refused() {
        let mut request = a_suggestion();
        request.year = Some(1200);
        assert!(request.check().is_err());
    }

    #[test]
    fn whitespace_is_trimmed_rather_than_stored() {
        let mut request = a_suggestion();
        request.brand = "  Datsun  ".to_owned();
        request.note = Some("   ".to_owned());

        let input = request.check().expect("valid");
        assert_eq!(input.brand, "Datsun");
        assert!(input.note.is_none(), "a note of only spaces is not a note");
    }

    #[test]
    fn a_pcd_reads_the_way_people_write_it() {
        // "5x114.3", not "5x114.30" and not "5x114.3000". A NUMERIC(5,1)
        // always returns its scale, and the client should not have to strip it.
        let diameter = sqlx::types::BigDecimal::from_str("114.3").expect("a decimal");
        assert_eq!(trim_decimal(&diameter), "114.3");

        let round = sqlx::types::BigDecimal::from_str("100.0").expect("a decimal");
        assert_eq!(
            trim_decimal(&round),
            "100",
            "a whole number should not carry a decimal point"
        );
    }

    #[test]
    fn a_generation_still_in_production_has_an_open_range() {
        let current = crate::adapter::postgres::catalog_repo::Generation {
            id: Uuid::now_v7(),
            name: "W100".to_owned(),
            slug: "w100".to_owned(),
            code: None,
            year_start: 2021,
            year_end: None,
        };
        assert_eq!(GenerationResponse::from(current).years, "2021–");
    }

    #[test]
    fn a_finished_generation_shows_both_years() {
        let past = crate::adapter::postgres::catalog_repo::Generation {
            id: Uuid::now_v7(),
            name: "F650".to_owned(),
            slug: "f650".to_owned(),
            code: Some("F650".to_owned()),
            year_start: 2015,
            year_end: Some(2021),
        };
        assert_eq!(GenerationResponse::from(past).years, "2015–2021");
    }

    #[test]
    fn a_variant_with_half_a_pcd_reports_none() {
        // The CHECK in the schema makes this unreachable through the database,
        // but the mapping should not invent "5x" if it ever were.
        let half = crate::adapter::postgres::catalog_repo::Variant {
            id: Uuid::now_v7(),
            name: "1.5 G".to_owned(),
            slug: "1-5-g".to_owned(),
            engine_code: None,
            displacement_cc: None,
            power_hp: None,
            torque_nm: None,
            pcd_bolt_count: Some(5),
            pcd_diameter_mm: None,
            center_bore_mm: None,
            oem_et_mm: None,
        };
        assert!(VariantResponse::from(half).pcd.is_none());
    }
}
