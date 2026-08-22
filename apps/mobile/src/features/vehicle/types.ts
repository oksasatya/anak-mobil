/**
 * These mirror the Rust response structs field-for-field, snake_case
 * included, so drift against the server is visible rather than absorbed by a
 * mapping layer. The source of truth is
 * apps/api/crates/runtime/src/adapter/http/catalog.rs.
 *
 * Only the fields this app actually reads are declared. A variant carries
 * pcd, torque, and bolt circle on the wire; the wizard shows none of them,
 * and typing a field nobody reads is how a type starts lying about what the
 * code depends on.
 */
export interface CatalogEntry {
  readonly id: string;
  readonly name: string;
}

export interface Generation extends CatalogEntry {
  readonly year_start: number;
  /** Null means the generation is still in production. */
  readonly year_end: number | null;
  /** Prebuilt by the server: "2015–2021", or "2021–" while in production. */
  readonly years: string;
}

export interface Variant extends CatalogEntry {
  readonly engine_code: string | null;
}
