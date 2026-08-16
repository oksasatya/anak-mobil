//! Dates on the wire, as `YYYY-MM-DD`.
//!
//! `time::Date`'s own serde implementation is **not** ISO 8601. It encodes a
//! date as `[year, ordinal]` — `[2026, 10]` for the tenth day of 2026 — and
//! refuses to read `"2026-01-10"` at all.
//!
//! That is defensible for a compact internal format and wrong for a public
//! API in two directions at once: a client sending a normal date string gets
//! a deserialization failure, and a client reading a stored date gets a pair
//! of numbers it has to know the encoding of.
//!
//! The failure is quiet, which is the dangerous part. It compiles, and it only
//! surfaces when a request actually carries a date — which is why
//! `vehicle_private.purchase_date` shipped with it and nothing noticed until a
//! service record needed a date of its own.
//!
//! Every `Date` crossing the API boundary goes through this module.

use serde::{Deserialize, Deserializer, Serializer};
use time::Date;
use time::format_description::well_known::Iso8601;

/// `YYYY-MM-DD`.
///
/// # Errors
///
/// Fails when the date cannot be formatted, which a valid `Date` cannot do.
pub fn serialize<S: Serializer>(date: &Date, serializer: S) -> Result<S::Ok, S::Error> {
    let text = date
        .format(&Iso8601::DATE)
        .map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&text)
}

/// Read `YYYY-MM-DD`.
///
/// # Errors
///
/// Fails when the string is not an ISO 8601 date.
pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Date, D::Error> {
    let text = String::deserialize(deserializer)?;
    Date::parse(&text, &Iso8601::DATE).map_err(serde::de::Error::custom)
}

/// The same, for a field that may be absent.
pub mod option {
    use super::{Date, Deserialize, Deserializer, Iso8601, Serializer};

    /// # Errors
    ///
    /// Fails when the date cannot be formatted.
    pub fn serialize<S: Serializer>(date: &Option<Date>, serializer: S) -> Result<S::Ok, S::Error> {
        match date {
            Some(date) => super::serialize(date, serializer),
            None => serializer.serialize_none(),
        }
    }

    /// # Errors
    ///
    /// Fails when the string is not an ISO 8601 date.
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Date>, D::Error> {
        let text = Option::<String>::deserialize(deserializer)?;
        text.map(|text| Date::parse(&text, &Iso8601::DATE))
            .transpose()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use time::Month;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Required {
        #[serde(with = "super")]
        date: Date,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Optional {
        #[serde(
            with = "super::option",
            skip_serializing_if = "Option::is_none",
            default
        )]
        date: Option<Date>,
    }

    fn a_date() -> Date {
        Date::from_calendar_date(2026, Month::January, 10).expect("a real date")
    }

    #[test]
    fn a_date_is_written_as_a_string_a_person_can_read() {
        // The regression: `time`'s own encoding is `[2026, 10]`, where the 10
        // is the ordinal day of the year rather than the day of the month.
        let json = serde_json::to_string(&Required { date: a_date() }).expect("serialising");
        assert_eq!(json, r#"{"date":"2026-01-10"}"#);
    }

    #[test]
    fn a_normal_date_string_is_accepted() {
        let parsed: Required =
            serde_json::from_str(r#"{"date":"2026-01-10"}"#).expect("deserialising");
        assert_eq!(parsed.date, a_date());
    }

    #[test]
    fn an_optional_date_round_trips() {
        let json = serde_json::to_string(&Optional {
            date: Some(a_date()),
        })
        .expect("serialising");
        assert_eq!(json, r#"{"date":"2026-01-10"}"#);

        let parsed: Optional = serde_json::from_str(&json).expect("deserialising");
        assert_eq!(parsed.date, Some(a_date()));
    }

    #[test]
    fn an_absent_date_stays_absent() {
        let json = serde_json::to_string(&Optional { date: None }).expect("serialising");
        assert_eq!(json, "{}");

        let parsed: Optional = serde_json::from_str("{}").expect("deserialising");
        assert_eq!(parsed.date, None);
    }

    #[test]
    fn nonsense_is_refused() {
        for raw in [
            r#"{"date":"10-01-2026"}"#,
            r#"{"date":"besok"}"#,
            r#"{"date":"2026-13-01"}"#,
        ] {
            assert!(
                serde_json::from_str::<Required>(raw).is_err(),
                "accepted {raw}"
            );
        }
    }
}
