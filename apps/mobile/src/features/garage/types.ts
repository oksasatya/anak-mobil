/**
 * `GET /vehicles`, on the wire.
 *
 * Hand-written against `VehicleResponse` and `ListSummaryResponse` in
 * apps/api/crates/runtime/src/adapter/http/{vehicles,summary}.rs. Neither
 * struct carries `#[serde(rename_all)]`, so every key is the Rust field name
 * unchanged — snake_case, deliberately, rather than a camelCase copy that
 * would have to be mapped somewhere.
 *
 * `packages/api-types` does not exist yet; when it does, this file is what it
 * replaces.
 *
 * There is no plate, VIN, or price here, and there is no field for one. That
 * is the server's privacy boundary (vehicles.rs module docs), and this type
 * mirrors it rather than re-deciding it.
 */
export interface VehicleSummary {
  readonly service_count: number;
  /** A decimal string — money, never a JSON number. Null when nothing is recorded. */
  readonly total_cost: string | null;
  /** ISO `YYYY-MM-DD`. Absent (not null) when there is no service history. */
  readonly last_service_date?: string;
  readonly overdue_count: number;
  readonly due_soon_count: number;
}

export interface Vehicle {
  readonly id: string;
  readonly variant_id: string | null;
  /**
   * The brand primary domain — `toyota.com`. Null when the car is not matched
   * to a catalog variant, which is also when there is no brand to draw. The
   * client composes the logo URL from it; see features/catalog/brandLogo.ts.
   */
  readonly brand_logo_domain: string | null;
  /** The nickname, else the catalog name, else what the owner typed. Never empty. */
  readonly name: string;
  readonly nickname: string | null;
  readonly year: number | null;
  readonly colour: string | null;
  readonly mileage_km: number | null;
  readonly position: number;
  /** Present on the list endpoint; the server omits it nowhere today. */
  readonly summary?: VehicleSummary;
}
