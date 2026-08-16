//! What a part still needs before its numbers can be compared.
//!
//! Pure: data in, a decision out. No async, no database.
//!
//! Completeness is **derived, never stored**. A stored flag goes stale the
//! moment a curator fills in a missing number, and a backfill to repair it is
//! a migration that exists only because the flag was stored. Derived, filling
//! one part's specs propagates to every build using it immediately.
//!
//! The SQL fitment queries must use a predicate equivalent to
//! [`missing_specs`]. Two definitions of "complete" that can disagree is a
//! defect waiting for its first curator.

/// The modification taxonomy, from PRD §10.
///
/// The domain's copy. The adapter keeps its own enum carrying the sqlx and
/// serde derives, and converts at the boundary — the persistence model and
/// the domain model are allowed to drift, and the compiler reports it as a
/// non-exhaustive match when they do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// How a suspension part adjusts ride height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspensionKind {
    Coilover,
    LoweringSpring,
    Air,
}

/// One typed specification a part may carry.
///
/// Named after the column, so a client can key an Indonesian label off it
/// without a second mapping table on this side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecField {
    WheelDiameterIn,
    WheelWidthIn,
    OffsetEtMm,
    PcdBoltCount,
    PcdDiameterMm,
    CenterBoreMm,
    TyreWidthMm,
    TyreAspectRatio,
    TyreRimDiameterIn,
    SuspensionKind,
    SpringRateKgmm,
}

impl SpecField {
    /// The column name, which is also the wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WheelDiameterIn => "wheel_diameter_in",
            Self::WheelWidthIn => "wheel_width_in",
            Self::OffsetEtMm => "offset_et_mm",
            Self::PcdBoltCount => "pcd_bolt_count",
            Self::PcdDiameterMm => "pcd_diameter_mm",
            Self::CenterBoreMm => "center_bore_mm",
            Self::TyreWidthMm => "tyre_width_mm",
            Self::TyreAspectRatio => "tyre_aspect_ratio",
            Self::TyreRimDiameterIn => "tyre_rim_diameter_in",
            Self::SuspensionKind => "suspension_kind",
            Self::SpringRateKgmm => "spring_rate_kgmm",
        }
    }
}

/// The typed specifications a part carries, one field per column.
///
/// Plain `Option`s of the column types rather than validated newtypes: the
/// range CHECK in the database already refuses an impossible value, and a
/// second enforcement here would be a second definition to keep in step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PartSpecs {
    pub wheel_diameter_in: Option<f64>,
    pub wheel_width_in: Option<f64>,
    pub offset_et_mm: Option<i16>,
    pub pcd_bolt_count: Option<i16>,
    pub pcd_diameter_mm: Option<f64>,
    pub center_bore_mm: Option<f64>,
    pub tyre_width_mm: Option<i16>,
    pub tyre_aspect_ratio: Option<i16>,
    pub tyre_rim_diameter_in: Option<f64>,
    pub suspension_kind: Option<SuspensionKind>,
    pub spring_rate_kgmm: Option<f64>,
}

/// What this part still needs, in a stable order.
///
/// Empty means complete. A category with no typed columns is always complete —
/// it cannot be missing a number it has no place to put, and flagging it would
/// mark every exhaust in the catalog incomplete forever, which teaches a
/// curator to ignore the flag.
///
/// AC2's second half lives here by omission: nothing in this function invents
/// a value. A plausible guessed offset reads as evidence and is worse than a
/// blank one.
///
/// Complexity: `O(k)` time and space, `k` ≤ 6 — the required fields for the
/// category. No loop over parts.
#[must_use]
pub fn missing_specs(category: PartCategory, specs: &PartSpecs) -> Vec<SpecField> {
    let mut missing = Vec::new();
    let mut require = |absent: bool, field: SpecField| {
        if absent {
            missing.push(field);
        }
    };

    match category {
        PartCategory::Wheels => {
            require(
                specs.wheel_diameter_in.is_none(),
                SpecField::WheelDiameterIn,
            );
            require(specs.wheel_width_in.is_none(), SpecField::WheelWidthIn);
            require(specs.offset_et_mm.is_none(), SpecField::OffsetEtMm);
            require(specs.pcd_bolt_count.is_none(), SpecField::PcdBoltCount);
            require(specs.pcd_diameter_mm.is_none(), SpecField::PcdDiameterMm);
            require(specs.center_bore_mm.is_none(), SpecField::CenterBoreMm);
        }
        PartCategory::Tyres => {
            require(specs.tyre_width_mm.is_none(), SpecField::TyreWidthMm);
            require(
                specs.tyre_aspect_ratio.is_none(),
                SpecField::TyreAspectRatio,
            );
            require(
                specs.tyre_rim_diameter_in.is_none(),
                SpecField::TyreRimDiameterIn,
            );
        }
        // The kind is required; the rate is not. An air setup has no spring
        // rate, and demanding one would flag every air build forever.
        PartCategory::Suspension => {
            require(specs.suspension_kind.is_none(), SpecField::SuspensionKind);
        }
        // Columns are added when a category earns them. Today three do.
        PartCategory::Brakes
        | PartCategory::Engine
        | PartCategory::Intake
        | PartCategory::Exhaust
        | PartCategory::Ecu
        | PartCategory::Transmission
        | PartCategory::Exterior
        | PartCategory::Interior
        | PartCategory::Lighting
        | PartCategory::Audio
        | PartCategory::Electronics
        | PartCategory::Other => {}
    }

    missing
}

/// Whether every number this category needs is present.
#[must_use]
pub fn is_complete(category: PartCategory, specs: &PartSpecs) -> bool {
    missing_specs(category, specs).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_wheel() -> PartSpecs {
        PartSpecs {
            wheel_diameter_in: Some(18.0),
            wheel_width_in: Some(8.5),
            offset_et_mm: Some(40),
            pcd_bolt_count: Some(5),
            pcd_diameter_mm: Some(114.3),
            center_bore_mm: Some(73.1),
            ..PartSpecs::default()
        }
    }

    #[test]
    fn a_wheel_with_every_number_is_complete() {
        assert!(missing_specs(PartCategory::Wheels, &a_wheel()).is_empty());
        assert!(is_complete(PartCategory::Wheels, &a_wheel()));
    }

    #[test]
    fn a_wheel_missing_its_offset_names_the_offset() {
        // AC2: flagged as incomplete, never filled with a guess. A plausible
        // invented offset is worse than a blank one — it reads as evidence.
        let mut specs = a_wheel();
        specs.offset_et_mm = None;

        assert_eq!(
            missing_specs(PartCategory::Wheels, &specs),
            vec![SpecField::OffsetEtMm]
        );
        assert!(!is_complete(PartCategory::Wheels, &specs));
    }

    #[test]
    fn a_bare_wheel_names_all_six() {
        // The order is part of the contract: a client rendering "belum
        // lengkap: offset, center bore" must not shuffle between reads.
        assert_eq!(
            missing_specs(PartCategory::Wheels, &PartSpecs::default()),
            vec![
                SpecField::WheelDiameterIn,
                SpecField::WheelWidthIn,
                SpecField::OffsetEtMm,
                SpecField::PcdBoltCount,
                SpecField::PcdDiameterMm,
                SpecField::CenterBoreMm,
            ]
        );
    }

    #[test]
    fn a_tyre_needs_three_numbers_and_no_wheel_numbers() {
        // A tyre carrying a PCD is not incomplete for lacking one; the fields
        // a category does not own are not its business.
        let specs = PartSpecs {
            tyre_width_mm: Some(225),
            tyre_aspect_ratio: Some(40),
            tyre_rim_diameter_in: Some(18.0),
            ..PartSpecs::default()
        };
        assert!(missing_specs(PartCategory::Tyres, &specs).is_empty());

        let mut short = specs;
        short.tyre_aspect_ratio = None;
        assert_eq!(
            missing_specs(PartCategory::Tyres, &short),
            vec![SpecField::TyreAspectRatio]
        );
    }

    #[test]
    fn air_suspension_is_complete_without_a_spring_rate() {
        // An air setup has no spring rate. Requiring one would flag every air
        // build as incomplete forever, and the flag would stop meaning anything.
        let specs = PartSpecs {
            suspension_kind: Some(SuspensionKind::Air),
            ..PartSpecs::default()
        };
        assert!(missing_specs(PartCategory::Suspension, &specs).is_empty());
    }

    #[test]
    fn suspension_with_no_kind_names_the_kind() {
        assert_eq!(
            missing_specs(PartCategory::Suspension, &PartSpecs::default()),
            vec![SpecField::SuspensionKind]
        );
    }

    #[test]
    fn a_category_with_no_columns_is_always_complete() {
        // Columns are added when a category earns them; today three do. A
        // category with none cannot be incomplete, and saying otherwise would
        // flag every exhaust in the catalog for a number that does not exist.
        for category in [
            PartCategory::Brakes,
            PartCategory::Engine,
            PartCategory::Intake,
            PartCategory::Exhaust,
            PartCategory::Ecu,
            PartCategory::Transmission,
            PartCategory::Exterior,
            PartCategory::Interior,
            PartCategory::Lighting,
            PartCategory::Audio,
            PartCategory::Electronics,
            PartCategory::Other,
        ] {
            assert!(
                missing_specs(category, &PartSpecs::default()).is_empty(),
                "{category:?} demanded a spec it has no column for"
            );
        }
    }

    #[test]
    fn a_spec_field_names_itself_for_the_wire() {
        // The client renders these; a field that stringifies to its Rust name
        // would ship CamelCase into an Indonesian UI.
        assert_eq!(SpecField::OffsetEtMm.as_str(), "offset_et_mm");
        assert_eq!(SpecField::CenterBoreMm.as_str(), "center_bore_mm");
    }

    #[test]
    fn a_bare_tyre_names_all_three() {
        // The sibling of `a_bare_wheel_names_all_six`, and it exists because a
        // review proved by mutation that two of these three checks could be
        // deleted with the whole suite still green: the tyre case only ever
        // blanked the aspect ratio, so width and rim diameter were never what
        // made an assertion true.
        //
        // What that would cost: a tyre carrying only an aspect ratio reads as
        // complete, the SQL predicate mirroring this agrees, and the fitment
        // engine compares a tyre with no width. Wheels and suspension were
        // pinned against exactly that; tyres were not.
        assert_eq!(
            missing_specs(PartCategory::Tyres, &PartSpecs::default()),
            vec![
                SpecField::TyreWidthMm,
                SpecField::TyreAspectRatio,
                SpecField::TyreRimDiameterIn,
            ]
        );
    }

    #[test]
    fn every_spec_field_names_its_column() {
        // `as_str` is the column name AND the wire name — the join between this
        // policy, the client's label lookup, and any SQL keyed by column. A
        // typo raises nothing anywhere; the client simply renders a key that
        // maps to nothing. Nine of the eleven mappings were unasserted, so a
        // typo in any of them passed the suite.
        let mapped = [
            (SpecField::WheelDiameterIn, "wheel_diameter_in"),
            (SpecField::WheelWidthIn, "wheel_width_in"),
            (SpecField::OffsetEtMm, "offset_et_mm"),
            (SpecField::PcdBoltCount, "pcd_bolt_count"),
            (SpecField::PcdDiameterMm, "pcd_diameter_mm"),
            (SpecField::CenterBoreMm, "center_bore_mm"),
            (SpecField::TyreWidthMm, "tyre_width_mm"),
            (SpecField::TyreAspectRatio, "tyre_aspect_ratio"),
            (SpecField::TyreRimDiameterIn, "tyre_rim_diameter_in"),
            (SpecField::SuspensionKind, "suspension_kind"),
            (SpecField::SpringRateKgmm, "spring_rate_kgmm"),
        ];

        for (field, column) in mapped {
            assert_eq!(field.as_str(), column, "{field:?} does not name its column");
        }
    }
}
