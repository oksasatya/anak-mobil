//! Garage endpoints.
//!
//! # The response types are the privacy boundary
//!
//! [`VehicleResponse`] has no field for a plate, a VIN, or a price. Not a
//! skipped field, not an `Option` that happens to be `None` — no field. It is
//! used by both the list and the detail, so there is no way to accidentally
//! serialise private data into either.
//!
//! [`PrivateResponse`] exists separately and appears in exactly one place: the
//! detail of a car, for the person who owns it. Adding private data to a list
//! response would require changing a type signature, which a reviewer sees.

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use time::Date;
use uuid::Uuid;

use crate::adapter::http::auth::Authenticated;
use crate::adapter::postgres::vehicle_repo::{VehicleInput, VehiclePrivate};
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::response::{ApiResponse, NoContent};
use crate::usecase::garage::{self, GarageError};

/// A car, as anyone may see it. There is deliberately nowhere here to put a
/// plate.
#[derive(Debug, Serialize)]
pub struct VehicleResponse {
    pub id: Uuid,
    pub variant_id: Option<Uuid>,
    pub name: String,
    pub nickname: Option<String>,
    pub year: Option<i16>,
    pub colour: Option<String>,
    pub mileage_km: Option<i32>,
    pub position: i32,
}

impl From<crate::adapter::postgres::vehicle_repo::Vehicle> for VehicleResponse {
    fn from(v: crate::adapter::postgres::vehicle_repo::Vehicle) -> Self {
        // What to call the car: the owner's nickname, else the catalog name,
        // else what they typed. A car always has something to show.
        let name = v
            .nickname
            .clone()
            .or_else(|| v.catalog_name.clone())
            .or_else(|| v.described_as.clone())
            .unwrap_or_else(|| "Mobil".to_owned());

        Self {
            id: v.id,
            variant_id: v.variant_id,
            name,
            nickname: v.nickname,
            year: v.year,
            colour: v.colour,
            mileage_km: v.mileage_km,
            position: v.position,
        }
    }
}

/// The owner's own private fields. Returned from one endpoint only.
#[derive(Debug, Serialize)]
pub struct PrivateResponse {
    pub plate: Option<String>,
    pub vin: Option<String>,
    pub purchase_price: Option<String>,
    pub purchase_date: Option<Date>,
}

impl From<VehiclePrivate> for PrivateResponse {
    fn from(p: VehiclePrivate) -> Self {
        Self {
            plate: p.plate,
            vin: p.vin,
            // A string, not a float. Rupiah amounts are exact and JSON
            // numbers are doubles in most clients.
            //
            // The scale is pinned rather than inherited. A `BigDecimal` keeps
            // whatever scale the round trip left it with, so a price stored as
            // 185000000.50 comes back rendering as `185000000.5000` — the same
            // number, a different string, and one every client would have to
            // normalise before displaying it.
            purchase_price: p
                .purchase_price
                .map(|amount| amount.with_scale(2).to_string()),
            purchase_date: p.purchase_date,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VehicleDetailResponse {
    #[serde(flatten)]
    pub vehicle: VehicleResponse,
    /// Absent when nothing private was recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<PrivateResponse>,
}

#[derive(Debug, Deserialize)]
pub struct VehicleRequest {
    pub variant_id: Option<Uuid>,
    pub nickname: Option<String>,
    pub described_as: Option<String>,
    pub year: Option<i16>,
    pub colour: Option<String>,
    pub mileage_km: Option<i32>,
    /// Optional at creation; a person can add their plate later.
    pub private: Option<PrivateRequest>,
}

#[derive(Debug, Deserialize)]
pub struct PrivateRequest {
    pub plate: Option<String>,
    pub vin: Option<String>,
    /// A decimal string, matching what the response emits.
    pub purchase_price: Option<String>,
    pub purchase_date: Option<Date>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderRequest {
    pub vehicle_ids: Vec<Uuid>,
}

impl VehicleRequest {
    fn to_input(&self) -> VehicleInput {
        VehicleInput {
            variant_id: self.variant_id,
            nickname: self.nickname.clone(),
            described_as: self.described_as.clone(),
            year: self.year,
            colour: self.colour.clone(),
            mileage_km: self.mileage_km,
        }
    }

    /// A car needs either a catalog match or a description, or nobody can
    /// tell what it is — including its owner.
    fn check(&self) -> Result<(), ApiError> {
        if self.variant_id.is_none() && self.described_as.as_deref().unwrap_or("").trim().is_empty()
        {
            return Err(ApiError::validation(serde_json::json!({
                "described_as": "Pilih dari katalog, atau tulis merek dan modelnya."
            })));
        }
        Ok(())
    }
}

impl PrivateRequest {
    fn to_private(&self) -> Result<VehiclePrivate, ApiError> {
        let purchase_price = match &self.purchase_price {
            Some(raw) => Some(raw.parse::<BigDecimal>().map_err(|_| {
                ApiError::validation(serde_json::json!({
                    "purchase_price": "Harus berupa angka, contoh: 185000000"
                }))
            })?),
            None => None,
        };

        Ok(VehiclePrivate {
            plate: self.plate.clone(),
            vin: self.vin.clone(),
            purchase_price,
            purchase_date: self.purchase_date,
        })
    }
}

/// `GET /vehicles`
///
/// # Errors
///
/// A storage failure.
pub async fn list(
    State(state): State<AppState>,
    caller: Authenticated,
) -> Result<ApiResponse<Vec<VehicleResponse>>, ApiError> {
    let vehicles = garage::list(&state.pool, caller.user_id)
        .await
        .map_err(to_api_error)?;
    Ok(ApiResponse::ok(
        vehicles.into_iter().map(Into::into).collect(),
    ))
}

/// `GET /vehicles/{id}`
///
/// # Errors
///
/// Not found, or a storage failure.
pub async fn detail(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(id): Path<Uuid>,
) -> Result<ApiResponse<VehicleDetailResponse>, ApiError> {
    let (vehicle, private) = garage::detail(&state.pool, caller.user_id, id)
        .await
        .map_err(to_api_error)?;

    Ok(ApiResponse::ok(VehicleDetailResponse {
        vehicle: vehicle.into(),
        private: private.map(Into::into),
    }))
}

/// `POST /vehicles`
///
/// # Errors
///
/// Validation failure, or a storage failure.
pub async fn create(
    State(state): State<AppState>,
    caller: Authenticated,
    Json(body): Json<VehicleRequest>,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    body.check()?;
    let private = body
        .private
        .as_ref()
        .map(PrivateRequest::to_private)
        .transpose()?;

    let id = garage::add(
        &state.pool,
        caller.user_id,
        &body.to_input(),
        private.as_ref(),
    )
    .await
    .map_err(to_api_error)?;

    Ok(ApiResponse::created(serde_json::json!({ "id": id })))
}

/// `PUT /vehicles/{id}`
///
/// # Errors
///
/// Validation failure, not found, or a storage failure.
pub async fn update(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(id): Path<Uuid>,
    Json(body): Json<VehicleRequest>,
) -> Result<NoContent, ApiError> {
    body.check()?;
    garage::update(&state.pool, caller.user_id, id, &body.to_input())
        .await
        .map_err(to_api_error)?;

    if let Some(private) = body.private.as_ref() {
        garage::set_private(&state.pool, caller.user_id, id, &private.to_private()?)
            .await
            .map_err(to_api_error)?;
    }

    Ok(NoContent)
}

/// `DELETE /vehicles/{id}`
///
/// # Errors
///
/// Not found, or a storage failure.
pub async fn delete(
    State(state): State<AppState>,
    caller: Authenticated,
    Path(id): Path<Uuid>,
) -> Result<NoContent, ApiError> {
    garage::remove(&state.pool, caller.user_id, id)
        .await
        .map_err(to_api_error)?;
    Ok(NoContent)
}

/// `PUT /vehicles/order`
///
/// # Errors
///
/// A listed vehicle is not the caller's, or a storage failure.
pub async fn reorder(
    State(state): State<AppState>,
    caller: Authenticated,
    Json(body): Json<ReorderRequest>,
) -> Result<NoContent, ApiError> {
    garage::reorder(&state.pool, caller.user_id, &body.vehicle_ids)
        .await
        .map_err(to_api_error)?;
    Ok(NoContent)
}

fn to_api_error(err: GarageError) -> ApiError {
    match err {
        // "Not yours" and "does not exist" answer identically, so an id cannot
        // be probed for existence.
        GarageError::NotFound | GarageError::ReorderMismatch => ApiError::not_found(),
        // A caller's mistake, not ours — the id they sent is not in the
        // catalog. Left to the foreign key this would surface as a 500.
        GarageError::UnknownVariant => ApiError::validation(serde_json::json!({
            "variant_id": "Varian tidak ada di katalog."
        })),
        GarageError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_request() -> VehicleRequest {
        VehicleRequest {
            variant_id: None,
            nickname: None,
            described_as: Some("Toyota Avanza 2019".to_owned()),
            year: Some(2019),
            colour: None,
            mileage_km: None,
            private: None,
        }
    }

    #[test]
    fn a_car_with_neither_catalog_match_nor_description_is_refused() {
        let mut request = a_request();
        request.described_as = None;
        assert!(request.check().is_err());
    }

    #[test]
    fn a_described_car_is_accepted_without_a_catalog_match() {
        // The path AC2 depends on: somebody whose model is missing must still
        // be able to add their car.
        assert!(a_request().check().is_ok());
    }

    #[test]
    fn whitespace_is_not_a_description() {
        let mut request = a_request();
        request.described_as = Some("   ".to_owned());
        assert!(request.check().is_err());
    }

    #[test]
    fn a_catalog_match_needs_no_description() {
        let mut request = a_request();
        request.described_as = None;
        request.variant_id = Some(Uuid::now_v7());
        assert!(request.check().is_ok());
    }

    #[test]
    fn a_price_that_is_not_a_number_is_refused() {
        let request = PrivateRequest {
            plate: None,
            vin: None,
            purchase_price: Some("dua ratus juta".to_owned()),
            purchase_date: None,
        };
        assert!(request.to_private().is_err());
    }

    #[test]
    fn a_price_keeps_its_exact_value() {
        // Through a decimal, not a float. 185000000.50 as an f64 is a
        // different number by the time it reaches the database.
        let request = PrivateRequest {
            plate: None,
            vin: None,
            purchase_price: Some("185000000.50".to_owned()),
            purchase_date: None,
        };
        let parsed = request
            .to_private()
            .expect("parsing")
            .purchase_price
            .expect("a price");
        assert_eq!(parsed.to_string(), "185000000.50");
    }

    #[test]
    fn the_public_response_has_no_place_to_put_a_plate() {
        // The privacy boundary, asserted against the serialised shape rather
        // than trusted. If somebody adds a plate field to VehicleResponse,
        // this fails.
        let response = VehicleResponse {
            id: Uuid::now_v7(),
            variant_id: None,
            name: "Avanza".to_owned(),
            nickname: None,
            year: Some(2019),
            colour: None,
            mileage_km: None,
            position: 0,
        };
        let json = serde_json::to_string(&response).expect("serialising");

        for forbidden in ["plate", "vin", "purchase_price", "purchase_date"] {
            assert!(
                !json.contains(forbidden),
                "a public vehicle response carried {forbidden}"
            );
        }
    }

    #[test]
    fn a_car_always_has_something_to_show() {
        let unnamed = crate::adapter::postgres::vehicle_repo::Vehicle {
            id: Uuid::now_v7(),
            variant_id: None,
            nickname: None,
            described_as: None,
            year: None,
            colour: None,
            mileage_km: None,
            position: 0,
            catalog_name: None,
        };
        assert!(!VehicleResponse::from(unnamed).name.is_empty());
    }

    #[test]
    fn the_nickname_wins_over_the_catalog_name() {
        let named = crate::adapter::postgres::vehicle_repo::Vehicle {
            id: Uuid::now_v7(),
            variant_id: None,
            nickname: Some("Si Putih".to_owned()),
            described_as: None,
            year: None,
            colour: None,
            mileage_km: None,
            position: 0,
            catalog_name: Some("Toyota Avanza 1.5 G".to_owned()),
        };
        assert_eq!(VehicleResponse::from(named).name, "Si Putih");
    }
}
