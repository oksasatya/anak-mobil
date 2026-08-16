//! Parts endpoints.
//!
//! Search, read, and the write that makes AC3 work: a part somebody types is
//! stored immediately and enters the curation queue in the same statement.
//!
//! # Why completeness is computed here and not stored
//!
//! `missing_specs` is derived on every read. A stored flag would go stale the
//! moment a curator filled in a missing number, and repairing it would cost a
//! backfill that exists only because the flag was stored.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use uuid::Uuid;

use anakmobil_domain::build::policy;

use crate::adapter::http::auth::Authenticated;
use crate::adapter::postgres::build_repo::EvidenceCount;
use crate::adapter::postgres::part_repo::{
    Part, PartCategory, PartInput, PartStatus, SuspensionKind,
};
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::pagination::clamp_limit;
use crate::shared::response::ApiResponse;
use crate::shared::validation::{self, DecimalSpec, OUT_OF_RANGE};
use crate::usecase::parts::{self, PartError};

#[derive(Debug, Serialize)]
pub struct PartResponse {
    pub id: Uuid,
    pub category: PartCategory,
    pub brand: String,
    pub product_name: String,
    pub status: PartStatus,
    /// Present when this part has been merged into another.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_part_id: Option<Uuid>,

    /// Decimal strings, exact. A `BigDecimal` keeps whatever scale the round
    /// trip left it with, and a JSON number is a double in most clients — a
    /// 114.3 PCD that arrives as 114.30000000000001 is a fitment mismatch.
    pub wheel_diameter_in: Option<String>,
    pub wheel_width_in: Option<String>,
    pub offset_et_mm: Option<i16>,
    pub pcd_bolt_count: Option<i16>,
    pub pcd_diameter_mm: Option<String>,
    pub center_bore_mm: Option<String>,
    pub tyre_width_mm: Option<i16>,
    pub tyre_aspect_ratio: Option<i16>,
    pub tyre_rim_diameter_in: Option<String>,
    pub suspension_kind: Option<SuspensionKind>,
    pub spring_rate_kgmm: Option<String>,

    /// AC2: flagged, never guessed. Column names, so a client can key an
    /// Indonesian label off each one.
    pub missing_specs: Vec<&'static str>,
    pub is_complete: bool,

    /// Builds where this part is fitted right now.
    pub active_build_count: i64,
    /// Builds where this part has ever been fitted, including ones where it
    /// was later taken off. Never smaller than `active_build_count`, and
    /// named separately so the wrong one does not end up on a screen — see
    /// [`crate::adapter::postgres::build_repo::evidence_counts`].
    pub ever_installed_build_count: i64,
}

impl PartResponse {
    /// A `From` impl cannot carry the counts as a second argument, so this
    /// is a plain constructor instead.
    fn new(part: Part, counts: EvidenceCount) -> Self {
        let category = policy::PartCategory::from(part.category);
        let missing = policy::missing_specs(category, &part.specs());
        // `normalized()` before `to_string()`, and it is not cosmetic.
        //
        // Postgres transmits NUMERIC in base-10000 groups, so decoding 114.3
        // from a NUMERIC(5,1) column yields a BigDecimal carrying trailing
        // zeros — it renders as "114.3000". A client comparing spec strings
        // then sees two different values for one PCD, which is exactly the
        // comparability AC2 exists to provide. The same drift produced
        // "185000000.5000" for a purchase price in AM-360.
        //
        // Normalising rather than pinning a scale, because the scale differs
        // per column: a centre bore holds two decimals and a wheel width one,
        // and a fixed `with_scale` would render 114.30 for a PCD nobody types
        // that way.
        let text = |value: Option<BigDecimal>| value.map(|amount| amount.normalized().to_string());

        Self {
            id: part.id,
            category: part.category,
            brand: part.brand,
            product_name: part.product_name,
            status: part.status,
            canonical_part_id: part.canonical_part_id,
            wheel_diameter_in: text(part.wheel_diameter_in),
            wheel_width_in: text(part.wheel_width_in),
            offset_et_mm: part.offset_et_mm,
            pcd_bolt_count: part.pcd_bolt_count,
            pcd_diameter_mm: text(part.pcd_diameter_mm),
            center_bore_mm: text(part.center_bore_mm),
            tyre_width_mm: part.tyre_width_mm,
            tyre_aspect_ratio: part.tyre_aspect_ratio,
            tyre_rim_diameter_in: text(part.tyre_rim_diameter_in),
            suspension_kind: part.suspension_kind,
            spring_rate_kgmm: text(part.spring_rate_kgmm),
            is_complete: missing.is_empty(),
            missing_specs: missing.iter().map(|field| field.as_str()).collect(),
            active_build_count: counts.active_build_count,
            ever_installed_build_count: counts.ever_installed_build_count,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PartRequest {
    pub category: PartCategory,
    pub brand: String,
    pub product_name: String,

    /// Decimal strings on the way in too, for the same reason they are on the
    /// way out: a client that sends 114.3 as a float has already lost it.
    pub wheel_diameter_in: Option<String>,
    pub wheel_width_in: Option<String>,
    pub offset_et_mm: Option<i16>,
    pub pcd_bolt_count: Option<i16>,
    pub pcd_diameter_mm: Option<String>,
    pub center_bore_mm: Option<String>,
    pub tyre_width_mm: Option<i16>,
    pub tyre_aspect_ratio: Option<i16>,
    pub tyre_rim_diameter_in: Option<String>,
    pub suspension_kind: Option<SuspensionKind>,
    pub spring_rate_kgmm: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub category: Option<PartCategory>,
    pub q: Option<String>,
    pub limit: Option<u16>,
}

impl PartRequest {
    /// Validate, and turn the decimal strings into `BigDecimal`s.
    ///
    /// The ranges here mirror the `CHECK` constraints exactly. Duplicated on
    /// purpose and worth saying why: the database constraint is the guarantee,
    /// and this is the message. Without it a caller gets a 500 from a
    /// constraint violation, which breaks AC3's promise that a part is usable
    /// the moment it is typed — a promise a 500 does not keep.
    ///
    /// `pub(super)` rather than private: `builds.rs`'s inline-part branch on
    /// `ModificationRequest` reuses this exact validation rather than a
    /// second copy of these ranges, which is what keeps them from drifting
    /// out of step with the `CHECK` constraints in one place but not the
    /// other.
    pub(super) fn check(&self) -> Result<PartInput, ApiError> {
        let mut problems = serde_json::Map::new();

        let brand = self.brand.trim();
        let product_name = self.product_name.trim();
        if brand.is_empty() {
            problems.insert("brand".into(), "Wajib diisi.".into());
        }
        if product_name.is_empty() {
            problems.insert("product_name".into(), "Wajib diisi.".into());
        }
        // Bounded, for the same reason every number is. `parts_identity` and
        // `parts_category_brand_idx` are B-trees over these two columns, and a
        // B-tree tuple caps at 2704 bytes — measured, a 3000-character brand
        // gives `index row size 3016 exceeds btree version 4 maximum`, which
        // reaches the caller as a 500 on the one endpoint whose criteria
        // forbid one. Worse than the numeric case: the insert fails, so no row
        // is written and the daily allowance is never consumed, leaving it
        // unbounded. The numeric path was guarded and the text path beside it
        // was never asked the same question.
        if brand.chars().count() > MAX_BRAND {
            problems.insert("brand".into(), too_long(MAX_BRAND).into());
        }
        if product_name.chars().count() > MAX_PRODUCT_NAME {
            problems.insert("product_name".into(), too_long(MAX_PRODUCT_NAME).into());
        }

        // Refused, not rounded. Postgres coerces a value past the column's
        // scale silently, and the idempotent insert then re-selects with what
        // the caller sent — which no longer matches what was stored, so a
        // second identical POST fails as "not found" and reaches the client
        // as a 500. Rounding here would only move the mismatch; storing 66.1
        // for a typed 66.06 is the same quiet alteration this module refuses
        // everywhere else. See [`validation::decimal`] for the guard itself
        // and why `normalized()` runs before `fractional_digit_count()`.
        let mut decimal = |spec: DecimalSpec<'_>| -> Option<BigDecimal> {
            validation::decimal(spec, &mut problems)
        };

        let wheel_diameter_in = decimal(DecimalSpec {
            field: "wheel_diameter_in",
            raw: self.wheel_diameter_in.as_deref(),
            scale: 1,
            min: 10.0,
            max: 30.0,
        });
        let wheel_width_in = decimal(DecimalSpec {
            field: "wheel_width_in",
            raw: self.wheel_width_in.as_deref(),
            scale: 1,
            min: 4.0,
            max: 20.0,
        });
        let pcd_diameter_mm = decimal(DecimalSpec {
            field: "pcd_diameter_mm",
            raw: self.pcd_diameter_mm.as_deref(),
            scale: 1,
            min: 80.0,
            max: 250.0,
        });
        let center_bore_mm = decimal(DecimalSpec {
            field: "center_bore_mm",
            raw: self.center_bore_mm.as_deref(),
            scale: 2,
            min: 40.0,
            max: 150.0,
        });
        let tyre_rim_diameter_in = decimal(DecimalSpec {
            field: "tyre_rim_diameter_in",
            raw: self.tyre_rim_diameter_in.as_deref(),
            scale: 1,
            min: 10.0,
            max: 30.0,
        });
        let spring_rate_kgmm = decimal(DecimalSpec {
            field: "spring_rate_kgmm",
            raw: self.spring_rate_kgmm.as_deref(),
            scale: 1,
            min: 1.0,
            max: 40.0,
        });

        for (field, value, min, max) in [
            ("offset_et_mm", self.offset_et_mm, -100, 100),
            ("pcd_bolt_count", self.pcd_bolt_count, 3, 8),
            ("tyre_width_mm", self.tyre_width_mm, 105, 405),
            ("tyre_aspect_ratio", self.tyre_aspect_ratio, 20, 95),
        ] {
            if value.is_some_and(|v| !(min..=max).contains(&v)) {
                problems.insert(field.into(), OUT_OF_RANGE.into());
            }
        }

        if !problems.is_empty() {
            return Err(ApiError::validation(serde_json::Value::Object(problems)));
        }

        Ok(PartInput {
            category: self.category,
            brand: brand.to_owned(),
            product_name: product_name.to_owned(),
            wheel_diameter_in,
            wheel_width_in,
            offset_et_mm: self.offset_et_mm,
            pcd_bolt_count: self.pcd_bolt_count,
            pcd_diameter_mm,
            center_bore_mm,
            tyre_width_mm: self.tyre_width_mm,
            tyre_aspect_ratio: self.tyre_aspect_ratio,
            tyre_rim_diameter_in,
            suspension_kind: self.suspension_kind,
            spring_rate_kgmm,
        })
    }
}

/// Text longer than these overflows the B-tree index behind `parts_identity`
/// and `parts_category_brand_idx` — a 3000-character brand measured
/// `index row size 3016 exceeds btree version 4 maximum`. Generous against
/// real part names, far under that limit.
const MAX_BRAND: usize = 120;
const MAX_PRODUCT_NAME: usize = 200;

/// The message for text longer than its column's index can hold.
fn too_long(limit: usize) -> String {
    format!("Maksimal {limit} karakter.")
}

/// `GET /parts`
///
/// # Errors
///
/// A storage failure.
pub async fn search(
    State(state): State<AppState>,
    caller: Authenticated,
    Query(query): Query<SearchQuery>,
) -> Result<ApiResponse<Vec<PartResponse>>, ApiError> {
    let parts = parts::search(
        &state.pool,
        caller.user_id,
        query.category,
        query.q.as_deref().filter(|text| !text.trim().is_empty()),
        clamp_limit(query.limit),
    )
    .await
    .map_err(to_api_error)?;

    Ok(ApiResponse::ok(
        parts
            .into_iter()
            .map(|(part, counts)| PartResponse::new(part, counts))
            .collect(),
    ))
}

/// `GET /parts/{id}`
///
/// # Errors
///
/// Not found, or a storage failure.
pub async fn detail(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(id): Path<Uuid>,
) -> Result<ApiResponse<PartResponse>, ApiError> {
    let (part, counts) = parts::detail(&state.pool, caller.user_id, id)
        .await
        .map_err(to_api_error)?;
    Ok(ApiResponse::ok(PartResponse::new(part, counts)))
}

/// `POST /parts`
///
/// Returns `201` with the id, whether the part was created or already existed.
/// Typing a part somebody else already typed is not an error.
///
/// # Errors
///
/// Validation failure, today's allowance spent, or a storage failure.
pub async fn suggest(
    State(state): State<AppState>,
    caller: Authenticated,
    Json(body): Json<PartRequest>,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    let input = body.check()?;
    let id = parts::suggest(&state.pool, caller.user_id, &input)
        .await
        .map_err(to_api_error)?;

    Ok(ApiResponse::created(serde_json::json!({ "id": id })))
}

pub(super) fn to_api_error(err: PartError) -> ApiError {
    match err {
        PartError::NotFound => ApiError::not_found(),
        PartError::TooManyParts => ApiError::too_many_requests(),
        PartError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_wheel() -> PartRequest {
        PartRequest {
            category: PartCategory::Wheels,
            brand: "Enkei".to_owned(),
            product_name: "RPF1".to_owned(),
            wheel_diameter_in: Some("18".to_owned()),
            wheel_width_in: Some("8.5".to_owned()),
            offset_et_mm: Some(40),
            pcd_bolt_count: Some(5),
            pcd_diameter_mm: Some("114.3".to_owned()),
            center_bore_mm: Some("73.1".to_owned()),
            tyre_width_mm: None,
            tyre_aspect_ratio: None,
            tyre_rim_diameter_in: None,
            suspension_kind: None,
            spring_rate_kgmm: None,
        }
    }

    #[test]
    fn a_normal_wheel_is_accepted() {
        assert!(a_wheel().check().is_ok());
    }

    #[test]
    fn a_pcd_keeps_its_exact_value() {
        // 114.3 through a float is 114.30000000000001, and a fitment engine
        // comparing that to a catalog 114.3 finds no match.
        let input = a_wheel().check().expect("valid");
        assert_eq!(input.pcd_diameter_mm.expect("a pcd").to_string(), "114.3");
    }

    #[test]
    fn a_deep_dish_offset_is_accepted() {
        // The bound that matters most: an unsigned column, or a floor of zero,
        // would reject exactly the wheels people fit.
        let mut request = a_wheel();
        request.offset_et_mm = Some(-25);
        assert!(request.check().is_ok());
    }

    #[test]
    fn an_impossible_bolt_count_is_refused_here_rather_than_by_the_database() {
        // A constraint violation reaching the caller as a 500 breaks AC3's
        // promise that a part is usable the moment it is typed.
        let mut request = a_wheel();
        request.pcd_bolt_count = Some(0);
        assert!(request.check().is_err());
    }

    #[test]
    fn a_tyre_size_typed_into_one_field_is_refused() {
        let mut request = a_wheel();
        request.category = PartCategory::Tyres;
        request.tyre_aspect_ratio = Some(900);
        assert!(request.check().is_err());
    }

    #[test]
    fn a_part_with_no_specs_at_all_is_accepted() {
        // AC2 flags it incomplete; it does not refuse it. Refusing would mean
        // a person who does not know their offset cannot record their wheels.
        let mut request = a_wheel();
        request.wheel_diameter_in = None;
        request.wheel_width_in = None;
        request.offset_et_mm = None;
        request.pcd_bolt_count = None;
        request.pcd_diameter_mm = None;
        request.center_bore_mm = None;
        assert!(request.check().is_ok());
    }

    #[test]
    fn a_blank_brand_is_refused() {
        let mut request = a_wheel();
        request.brand = "   ".to_owned();
        assert!(request.check().is_err());
    }

    #[test]
    fn a_category_round_trips_through_its_wire_form() {
        let json = serde_json::to_string(&PartCategory::Wheels).expect("serialising");
        assert_eq!(json, "\"wheels\"");
    }

    #[test]
    fn every_range_message_names_its_field() {
        // Two fields wrong reports two fields, not the first one.
        let mut request = a_wheel();
        request.pcd_bolt_count = Some(0);
        request.tyre_width_mm = Some(9_000);
        let err = request.check().expect_err("two problems");
        assert_eq!(err.code(), crate::shared::i18n::ErrorCode::ValidationFailed);
    }

    #[test]
    fn a_value_the_column_cannot_hold_is_refused_rather_than_rounded() {
        // The defect this closes: NUMERIC(_,1) rounds 66.06 to 66.1 on insert,
        // the idempotent re-select then looks for 66.06 and matches nothing,
        // and a second identical POST reaches the client as a 500 where the
        // acceptance criteria promise a 201 with the same id. Proven against
        // the live schema before the column was widened.
        let mut request = a_wheel();

        request.center_bore_mm = Some("66.06".to_owned());
        assert!(
            request.check().is_ok(),
            "66.06 is a real VAG bore and must be storable"
        );

        request.center_bore_mm = Some("66.066".to_owned());
        assert!(
            request.check().is_err(),
            "a third decimal exceeds NUMERIC(6,2)"
        );

        let mut request = a_wheel();
        request.wheel_width_in = Some("8.55".to_owned());
        assert!(
            request.check().is_err(),
            "width is NUMERIC(3,1); 8.55 would round to 8.6"
        );
    }

    #[test]
    fn every_range_guard_can_actually_fail() {
        // A review found that nine of the twelve guards could be deleted with
        // the suite still green — and deleting one is exactly what turns a
        // constraint violation into the 500 that AC6 forbids. One case per
        // guard, so removing any single check makes precisely this test red.
        /// Named so clippy's type_complexity has something to hold on to, and
        /// so the pair reads as "the field, and the way to break it".
        type Break = (&'static str, fn(&mut PartRequest));

        let cases: [Break; 12] = [
            ("wheel_diameter_in", |r| {
                r.wheel_diameter_in = Some("99".to_owned())
            }),
            ("wheel_width_in", |r| {
                r.wheel_width_in = Some("99".to_owned())
            }),
            ("pcd_diameter_mm", |r| {
                r.pcd_diameter_mm = Some("999".to_owned())
            }),
            ("center_bore_mm", |r| {
                r.center_bore_mm = Some("999".to_owned())
            }),
            ("tyre_rim_diameter_in", |r| {
                r.tyre_rim_diameter_in = Some("99".to_owned())
            }),
            ("spring_rate_kgmm", |r| {
                r.spring_rate_kgmm = Some("999".to_owned())
            }),
            ("pcd_bolt_count", |r| r.pcd_bolt_count = Some(0)),
            ("tyre_width_mm", |r| r.tyre_width_mm = Some(9_000)),
            ("tyre_aspect_ratio", |r| r.tyre_aspect_ratio = Some(900)),
            ("offset_et_mm", |r| r.offset_et_mm = Some(-999)),
            ("brand", |r| r.brand = "   ".to_owned()),
            ("product_name", |r| r.product_name = "   ".to_owned()),
        ];

        for (field, break_it) in cases {
            let mut request = a_wheel();
            break_it(&mut request);
            assert!(
                request.check().is_err(),
                "{field} is out of range and check() accepted it — that reaches the database as a 500"
            );
        }
    }

    #[test]
    fn a_trailing_zero_is_not_extra_precision() {
        // The regression this closes: bare `fractional_digit_count()` counts
        // declared scale, so "114.30" reported two digits and was refused
        // against a one-decimal column — a value NUMERIC(5,1) stores exactly,
        // and one that round-tripped correctly BEFORE the guard existed.
        // A client formatting decimals to two places is ordinary.
        let mut request = a_wheel();
        request.pcd_diameter_mm = Some("114.30".to_owned());
        assert!(
            request.check().is_ok(),
            "114.30 stores as 114.3 with no loss"
        );

        request.pcd_diameter_mm = Some("114.35".to_owned());
        assert!(
            request.check().is_err(),
            "a real second decimal must still be refused"
        );
    }

    #[test]
    fn every_scale_mirrors_its_column() {
        // The shared guard is pinned; the per-field number was not. Setting
        // pcd_diameter_mm to scale 2 left the whole suite green while "114.35"
        // was accepted, rounded by Postgres to 114.4, re-selected as 114.35,
        // matched by nothing, and returned as the 500 the guard exists to stop.
        // One case per column, so a wrong mirror reddens exactly this test.
        type TooPrecise = (&'static str, fn(&mut PartRequest));
        let cases: [TooPrecise; 6] = [
            ("wheel_diameter_in", |r| {
                r.wheel_diameter_in = Some("18.55".to_owned())
            }),
            ("wheel_width_in", |r| {
                r.wheel_width_in = Some("8.55".to_owned())
            }),
            ("pcd_diameter_mm", |r| {
                r.pcd_diameter_mm = Some("114.35".to_owned())
            }),
            ("center_bore_mm", |r| {
                r.center_bore_mm = Some("66.066".to_owned())
            }),
            ("tyre_rim_diameter_in", |r| {
                r.tyre_rim_diameter_in = Some("18.55".to_owned())
            }),
            ("spring_rate_kgmm", |r| {
                r.spring_rate_kgmm = Some("8.55".to_owned())
            }),
        ];

        for (field, break_it) in cases {
            let mut request = a_wheel();
            break_it(&mut request);
            assert!(
                request.check().is_err(),
                "{field} accepted a scale its column cannot hold — Postgres rounds it, \
                 the idempotent re-select then matches nothing, and a duplicate POST is a 500"
            );
        }
    }

    #[test]
    fn text_longer_than_its_index_can_hold_is_refused() {
        // A B-tree tuple caps at 2704 bytes and both columns sit in two of
        // them. Measured: a 3000-character brand gives `index row size 3016
        // exceeds btree version 4 maximum` — a 500, and one that consumes no
        // allowance because the insert never happens.
        let mut request = a_wheel();
        request.brand = "x".repeat(MAX_BRAND + 1);
        assert!(
            request.check().is_err(),
            "an over-long brand must not reach the index"
        );

        let mut request = a_wheel();
        request.product_name = "x".repeat(MAX_PRODUCT_NAME + 1);
        assert!(
            request.check().is_err(),
            "an over-long product name must not reach the index"
        );

        let mut request = a_wheel();
        request.brand = "Ö".repeat(MAX_BRAND);
        assert!(
            request.check().is_ok(),
            "the limit counts characters, not bytes"
        );
    }
}
