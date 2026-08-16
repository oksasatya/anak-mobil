//! Reading and writing parts.
//!
//! None of these take an owner. A part is reference data — the catalog the
//! whole community shares — in the same way the vehicle catalog is, and
//! scoping it to a person would make the same wheel exist once per user.
//!
//! `suggested_by` records who first typed it, which is curation provenance,
//! not ownership.

use sqlx::PgConnection;
use sqlx::types::BigDecimal;
use uuid::Uuid;

use anakmobil_domain::build::policy;

/// PRD §10, mirrored from the Postgres enum.
///
/// The persistence enum, not the domain enum. The derives make the variant
/// name the source of both the SQL label and the JSON wire form, so there are
/// no string literals here to drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "modification_category", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PartCategory {
    Wheels,
    Tyres,
    Suspension,
    Brakes,
    Engine,
    Intake,
    Exhaust,
    Ecu,
    Transmission,
    Exterior,
    Interior,
    Lighting,
    Audio,
    Electronics,
    Other,
}

/// A part is approved or waiting for a curator. Never rejected: a rejected
/// part would orphan every modification already using it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "part_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PartStatus {
    Pending,
    Approved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "suspension_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SuspensionKind {
    Coilover,
    LoweringSpring,
    Air,
}

impl From<PartCategory> for policy::PartCategory {
    /// Non-exhaustive the moment either side gains a variant — the compiler
    /// reports the drift rather than a wrong default swallowing it.
    fn from(category: PartCategory) -> Self {
        match category {
            PartCategory::Wheels => Self::Wheels,
            PartCategory::Tyres => Self::Tyres,
            PartCategory::Suspension => Self::Suspension,
            PartCategory::Brakes => Self::Brakes,
            PartCategory::Engine => Self::Engine,
            PartCategory::Intake => Self::Intake,
            PartCategory::Exhaust => Self::Exhaust,
            PartCategory::Ecu => Self::Ecu,
            PartCategory::Transmission => Self::Transmission,
            PartCategory::Exterior => Self::Exterior,
            PartCategory::Interior => Self::Interior,
            PartCategory::Lighting => Self::Lighting,
            PartCategory::Audio => Self::Audio,
            PartCategory::Electronics => Self::Electronics,
            PartCategory::Other => Self::Other,
        }
    }
}

impl From<policy::PartCategory> for PartCategory {
    fn from(category: policy::PartCategory) -> Self {
        match category {
            policy::PartCategory::Wheels => Self::Wheels,
            policy::PartCategory::Tyres => Self::Tyres,
            policy::PartCategory::Suspension => Self::Suspension,
            policy::PartCategory::Brakes => Self::Brakes,
            policy::PartCategory::Engine => Self::Engine,
            policy::PartCategory::Intake => Self::Intake,
            policy::PartCategory::Exhaust => Self::Exhaust,
            policy::PartCategory::Ecu => Self::Ecu,
            policy::PartCategory::Transmission => Self::Transmission,
            policy::PartCategory::Exterior => Self::Exterior,
            policy::PartCategory::Interior => Self::Interior,
            policy::PartCategory::Lighting => Self::Lighting,
            policy::PartCategory::Audio => Self::Audio,
            policy::PartCategory::Electronics => Self::Electronics,
            policy::PartCategory::Other => Self::Other,
        }
    }
}

impl From<SuspensionKind> for policy::SuspensionKind {
    fn from(kind: SuspensionKind) -> Self {
        match kind {
            SuspensionKind::Coilover => Self::Coilover,
            SuspensionKind::LoweringSpring => Self::LoweringSpring,
            SuspensionKind::Air => Self::Air,
        }
    }
}

impl From<policy::SuspensionKind> for SuspensionKind {
    fn from(kind: policy::SuspensionKind) -> Self {
        match kind {
            policy::SuspensionKind::Coilover => Self::Coilover,
            policy::SuspensionKind::LoweringSpring => Self::LoweringSpring,
            policy::SuspensionKind::Air => Self::Air,
        }
    }
}

/// A part as stored.
#[derive(Debug, Clone)]
pub struct Part {
    pub id: Uuid,
    pub category: PartCategory,
    pub brand: String,
    pub product_name: String,
    pub status: PartStatus,
    /// `None` means this part is its own canonical form.
    pub canonical_part_id: Option<Uuid>,

    pub wheel_diameter_in: Option<BigDecimal>,
    pub wheel_width_in: Option<BigDecimal>,
    pub offset_et_mm: Option<i16>,
    pub pcd_bolt_count: Option<i16>,
    pub pcd_diameter_mm: Option<BigDecimal>,
    pub center_bore_mm: Option<BigDecimal>,
    pub tyre_width_mm: Option<i16>,
    pub tyre_aspect_ratio: Option<i16>,
    pub tyre_rim_diameter_in: Option<BigDecimal>,
    pub suspension_kind: Option<SuspensionKind>,
    pub spring_rate_kgmm: Option<BigDecimal>,
}

/// What a caller may set. No `status` and no `canonical_part_id`: a caller
/// cannot approve their own part, and merging is a curator's operation.
#[derive(Debug, Clone)]
pub struct PartInput {
    pub category: PartCategory,
    pub brand: String,
    pub product_name: String,

    pub wheel_diameter_in: Option<BigDecimal>,
    pub wheel_width_in: Option<BigDecimal>,
    pub offset_et_mm: Option<i16>,
    pub pcd_bolt_count: Option<i16>,
    pub pcd_diameter_mm: Option<BigDecimal>,
    pub center_bore_mm: Option<BigDecimal>,
    pub tyre_width_mm: Option<i16>,
    pub tyre_aspect_ratio: Option<i16>,
    pub tyre_rim_diameter_in: Option<BigDecimal>,
    pub suspension_kind: Option<SuspensionKind>,
    pub spring_rate_kgmm: Option<BigDecimal>,
}

impl Part {
    /// The domain's view of this part's numbers.
    ///
    /// `BigDecimal` does not cross the boundary — the domain crate has no sqlx
    /// dependency to name it with. `f64` is enough for a completeness check,
    /// which only asks whether a value is present.
    #[must_use]
    pub fn specs(&self) -> policy::PartSpecs {
        let decimal = |value: &Option<BigDecimal>| -> Option<f64> {
            use std::str::FromStr as _;
            value
                .as_ref()
                .map(ToString::to_string)
                .and_then(|text| f64::from_str(&text).ok())
        };

        policy::PartSpecs {
            wheel_diameter_in: decimal(&self.wheel_diameter_in),
            wheel_width_in: decimal(&self.wheel_width_in),
            offset_et_mm: self.offset_et_mm,
            pcd_bolt_count: self.pcd_bolt_count,
            pcd_diameter_mm: decimal(&self.pcd_diameter_mm),
            center_bore_mm: decimal(&self.center_bore_mm),
            tyre_width_mm: self.tyre_width_mm,
            tyre_aspect_ratio: self.tyre_aspect_ratio,
            tyre_rim_diameter_in: decimal(&self.tyre_rim_diameter_in),
            suspension_kind: self.suspension_kind.map(Into::into),
            spring_rate_kgmm: decimal(&self.spring_rate_kgmm),
        }
    }
}

// The repeated column list below is deliberate — not extracted to a `const`.
// The quality gate's "duplicated literal ≥3× → a module-level `const`" rule
// does not reach here: sqlx's `query_as!` requires SQL that is a literal it
// can parse at compile time, so a `const` interpolated into the query would
// mean `format!` into SQL — forbidden by the same gate, and it would silently
// give up the compile-time schema check that is the whole reason these macros
// are used. Two queries repeating a column list is the price of compile-time
// verification, and it is worth paying.

/// One part by id.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn find(conn: &mut PgConnection, id: Uuid) -> Result<Option<Part>, sqlx::Error> {
    sqlx::query_as!(
        Part,
        r#"
        SELECT
            id,
            category AS "category: PartCategory",
            brand,
            product_name,
            status AS "status: PartStatus",
            canonical_part_id,
            wheel_diameter_in,
            wheel_width_in,
            offset_et_mm,
            pcd_bolt_count,
            pcd_diameter_mm,
            center_bore_mm,
            tyre_width_mm,
            tyre_aspect_ratio,
            tyre_rim_diameter_in,
            suspension_kind AS "suspension_kind: SuspensionKind",
            spring_rate_kgmm
        FROM parts
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(conn)
    .await
}

/// Parts matching a category and a scrap of text.
///
/// Both filters are optional; with neither, this is the newest parts in the
/// catalog. Merged-away parts are returned like any other — filtering them out
/// belongs to the merge story, and hiding them here would make a search miss
/// the row a curator is trying to find.
///
/// Complexity: `O(log n + k)` when only the category is given, using
/// `parts_category_brand_idx`. With a text query it is `O(n)` — a sequential
/// scan, because `ILIKE '%…%'` cannot use a B-tree.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn search(
    conn: &mut PgConnection,
    category: Option<PartCategory>,
    query: Option<&str>,
    limit: u16,
) -> Result<Vec<Part>, sqlx::Error> {
    // ponytail: a sequential ILIKE scan. The catalog starts empty and grows by
    // hand; a GIN index over pg_trgm is the upgrade, and it needs
    // CREATE EXTENSION pg_trgm in the extensions migration — privileges an
    // application role should not hold, which is why that file exists. Add it
    // when the table passes roughly ten thousand rows, not before.
    let pattern = query.map(|text| format!("%{}%", text.trim()));

    sqlx::query_as!(
        Part,
        r#"
        SELECT
            id,
            category AS "category: PartCategory",
            brand,
            product_name,
            status AS "status: PartStatus",
            canonical_part_id,
            wheel_diameter_in,
            wheel_width_in,
            offset_et_mm,
            pcd_bolt_count,
            pcd_diameter_mm,
            center_bore_mm,
            tyre_width_mm,
            tyre_aspect_ratio,
            tyre_rim_diameter_in,
            suspension_kind AS "suspension_kind: SuspensionKind",
            spring_rate_kgmm
        FROM parts
        WHERE ($1::modification_category IS NULL OR category = $1)
          AND (
                $2::text IS NULL
                OR brand        ILIKE $2
                OR product_name ILIKE $2
              )
        ORDER BY status, brand, product_name, id
        LIMIT $3
        "#,
        category as Option<PartCategory>,
        pattern,
        i64::from(limit),
    )
    .fetch_all(conn)
    .await
}

// `ORDER BY status` puts `pending` before `approved` because the enum
// declares it first. That is deliberate: a person who just typed a part
// expects to see it, and a curator reading the same list wants the queue at
// the top. If that ordering ever needs reversing, reverse it here rather than
// reordering the enum — enum order is a schema change.

/// The id of the part with exactly these specs, creating it as `pending` if
/// nobody has typed it before.
///
/// AC3: the part is usable the moment it is typed, and it enters the curation
/// queue in the same breath. Both halves, one statement.
///
/// Idempotent by way of the `parts_identity` constraint. `ON CONFLICT DO
/// NOTHING` then a re-select rather than `DO UPDATE`: there is nothing to
/// update — the conflicting row is by definition the same configuration — and
/// `DO UPDATE` would bump `updated_at` on somebody else's approved part every
/// time a second person typed it.
///
/// The caller supplies `id` so the use case can generate a UUID v7 without
/// this function reaching for a clock.
///
/// Complexity: `O(log n)`, one or two index probes. Never a scan.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn find_or_create_pending(
    conn: &mut PgConnection,
    id: Uuid,
    suggested_by: Uuid,
    input: &PartInput,
) -> Result<Uuid, sqlx::Error> {
    let inserted = sqlx::query_scalar!(
        r#"
        INSERT INTO parts (
            id, category, brand, product_name, status, suggested_by,
            wheel_diameter_in, wheel_width_in, offset_et_mm,
            pcd_bolt_count, pcd_diameter_mm, center_bore_mm,
            tyre_width_mm, tyre_aspect_ratio, tyre_rim_diameter_in,
            suspension_kind, spring_rate_kgmm
        )
        VALUES ($1, $2, $3, $4, 'pending', $5,
                $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
        ON CONFLICT ON CONSTRAINT parts_identity DO NOTHING
        RETURNING id
        "#,
        id,
        input.category as PartCategory,
        input.brand,
        input.product_name,
        suggested_by,
        input.wheel_diameter_in,
        input.wheel_width_in,
        input.offset_et_mm,
        input.pcd_bolt_count,
        input.pcd_diameter_mm,
        input.center_bore_mm,
        input.tyre_width_mm,
        input.tyre_aspect_ratio,
        input.tyre_rim_diameter_in,
        input.suspension_kind as Option<SuspensionKind>,
        input.spring_rate_kgmm,
    )
    .fetch_optional(&mut *conn)
    .await?;

    if let Some(id) = inserted {
        return Ok(id);
    }

    // Somebody has typed this exact configuration before. `NULLS NOT DISTINCT`
    // on the constraint is what makes `IS NOT DISTINCT FROM` the matching
    // predicate here — plain `=` would never match two NULL spec columns.
    sqlx::query_scalar!(
        r#"
        SELECT id FROM parts
        WHERE category = $1
          AND brand = $2
          AND product_name = $3
          AND wheel_diameter_in    IS NOT DISTINCT FROM $4
          AND wheel_width_in       IS NOT DISTINCT FROM $5
          AND offset_et_mm         IS NOT DISTINCT FROM $6
          AND pcd_bolt_count       IS NOT DISTINCT FROM $7
          AND pcd_diameter_mm      IS NOT DISTINCT FROM $8
          AND center_bore_mm       IS NOT DISTINCT FROM $9
          AND tyre_width_mm        IS NOT DISTINCT FROM $10
          AND tyre_aspect_ratio    IS NOT DISTINCT FROM $11
          AND tyre_rim_diameter_in IS NOT DISTINCT FROM $12
          AND suspension_kind      IS NOT DISTINCT FROM $13
          AND spring_rate_kgmm     IS NOT DISTINCT FROM $14
        "#,
        input.category as PartCategory,
        input.brand,
        input.product_name,
        input.wheel_diameter_in,
        input.wheel_width_in,
        input.offset_et_mm,
        input.pcd_bolt_count,
        input.pcd_diameter_mm,
        input.center_bore_mm,
        input.tyre_width_mm,
        input.tyre_aspect_ratio,
        input.tyre_rim_diameter_in,
        input.suspension_kind as Option<SuspensionKind>,
        input.spring_rate_kgmm,
    )
    .fetch_one(conn)
    .await
}

/// How many parts this person has added since a moment in time.
///
/// The same shape as `catalog_repo::suggestions_since` — the queue is read by
/// a human, so flooding it is a denial of service against curation rather
/// than against the server.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn suggested_since(
    conn: &mut PgConnection,
    suggested_by: Uuid,
    since: time::OffsetDateTime,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM parts WHERE suggested_by = $1 AND created_at >= $2"#,
        suggested_by,
        since
    )
    .fetch_one(conn)
    .await
    .map(|count| count.unwrap_or(0))
}
