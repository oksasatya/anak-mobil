//! The decimal-range guard every numeric spec on the wire needs.
//!
//! A `NUMERIC(_, scale)` column with a range `CHECK` needs the same three
//! refusals wherever a caller can type a value for it: not a number, outside
//! the range, or carrying more decimal places than the column can hold.
//! `parts.rs` had this once; `services.rs` had a second, weaker copy that
//! checked only parseability and sign, which is how a cost of `1200000.755`
//! reached Postgres, got silently rounded to a different number than the
//! caller typed, or — past the column's total digits — came back as a
//! `22003 numeric field overflow` that surfaces to the client as a `500`
//! where the criteria promise a `422`. One guard now, used by `parts.rs`,
//! `services.rs`, and `builds.rs` alike.

use sqlx::types::BigDecimal;

/// The message every out-of-range spec shares.
pub const OUT_OF_RANGE: &str = "Angka di luar rentang yang masuk akal.";

/// One numeric spec's name, its raw text, and the range and scale the column
/// accepts.
///
/// A struct rather than five positional arguments: `clippy::too_many_arguments`
/// starts at seven, and a tuple of five unnamed values at every call site is
/// how the width and the diameter get swapped.
pub struct DecimalSpec<'a> {
    pub field: &'static str,
    pub raw: Option<&'a str>,
    pub min: f64,
    pub max: f64,
    /// Decimal places the column can hold. Anything longer is refused rather
    /// than rounded — see the module doc.
    pub scale: i64,
}

/// Validate a decimal spec, recording a problem by field name rather than
/// stopping at the first one — the caller accumulates `problems` across every
/// field on the request and returns them together.
///
/// Refused, not rounded. Postgres coerces a value past the column's scale
/// silently, and an idempotent insert then re-selects with what the caller
/// sent — which no longer matches what was stored, so a second identical
/// request fails as "not found" and reaches the client as a `500`. Rounding
/// here would only move the mismatch.
///
/// `normalized()` before `fractional_digit_count()`, and the omission was a
/// regression once already: bare `fractional_digit_count()` counts DECLARED
/// scale, so `"114.30"` reports two digits and is refused against a
/// one-decimal column — for a value the column stores with no loss at all.
pub fn decimal(
    spec: DecimalSpec<'_>,
    problems: &mut serde_json::Map<String, serde_json::Value>,
) -> Option<BigDecimal> {
    let raw = spec.raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let Ok(value) = raw.parse::<BigDecimal>() else {
        problems.insert(spec.field.into(), OUT_OF_RANGE.into());
        return None;
    };
    if !in_range(&value, spec.min, spec.max) {
        problems.insert(spec.field.into(), OUT_OF_RANGE.into());
        return None;
    }
    if value.normalized().fractional_digit_count() > spec.scale {
        problems.insert(spec.field.into(), too_precise(spec.scale).into());
        return None;
    }
    Some(value)
}

/// Whether a decimal sits inside the column's `CHECK`.
///
/// Compared as text through `f64` rather than by building two `BigDecimal`
/// bounds: the bounds are whole or one-decimal numbers a `f64` represents
/// exactly, and the comparison only decides whether to reject.
fn in_range(value: &BigDecimal, min: f64, max: f64) -> bool {
    value
        .to_string()
        .parse::<f64>()
        .is_ok_and(|v| (min..=max).contains(&v))
}

/// The message for a value carrying more decimal places than its column.
fn too_precise(scale: i64) -> String {
    if scale == 1 {
        "Maksimal satu angka di belakang koma.".to_owned()
    } else {
        format!("Maksimal {scale} angka di belakang koma.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(raw: &str, scale: i64) -> DecimalSpec<'_> {
        DecimalSpec {
            field: "cost",
            raw: Some(raw),
            min: 0.0,
            max: 999_999_999_999.99,
            scale,
        }
    }

    #[test]
    fn a_value_within_range_and_scale_is_accepted() {
        let mut problems = serde_json::Map::new();
        let value = decimal(spec("1200000.75", 2), &mut problems);
        assert_eq!(value.expect("accepted").to_string(), "1200000.75");
        assert!(problems.is_empty());
    }

    #[test]
    fn a_trailing_zero_is_not_extra_precision() {
        let mut problems = serde_json::Map::new();
        decimal(spec("114.30", 1), &mut problems);
        assert!(problems.is_empty(), "114.30 stores as 114.3 with no loss");
    }

    #[test]
    fn a_value_the_column_cannot_hold_is_refused_rather_than_rounded() {
        let mut problems = serde_json::Map::new();
        let value = decimal(spec("1200000.755", 2), &mut problems);
        assert!(value.is_none());
        assert!(problems.contains_key("cost"));
    }

    #[test]
    fn a_value_past_the_columns_range_is_refused() {
        let mut problems = serde_json::Map::new();
        let value = decimal(spec("9999999999999999", 2), &mut problems);
        assert!(value.is_none());
        assert!(problems.contains_key("cost"));
    }

    #[test]
    fn a_negative_value_is_refused_when_the_minimum_is_zero() {
        let mut problems = serde_json::Map::new();
        let value = decimal(spec("-500", 2), &mut problems);
        assert!(value.is_none());
        assert!(problems.contains_key("cost"));
    }

    #[test]
    fn an_absent_value_is_fine_and_reports_nothing() {
        let mut problems = serde_json::Map::new();
        let value = decimal(
            DecimalSpec {
                field: "cost",
                raw: None,
                min: 0.0,
                max: 1.0,
                scale: 1,
            },
            &mut problems,
        );
        assert!(value.is_none());
        assert!(problems.is_empty());
    }
}
