//! When a car is next due for something.
//!
//! Pure: dates in, decisions out. No async, no database, no clock — today
//! is an argument, so a test can put the car anywhere in time without
//! waiting for it.

use time::{Date, Month};

/// The kinds of work a service record can describe.
///
/// This is the domain's copy. The adapter keeps its own enum carrying the
/// sqlx and serde derives, and converts at the boundary — the persistence
/// model and the domain model are allowed to drift, and the compiler
/// reports it as a non-exhaustive match when they do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ServiceCategory {
    EngineOil,
    TransmissionOil,
    Brakes,
    Suspension,
    TuneUp,
    AirConditioning,
    Electrical,
    Bodywork,
    Other,
}

/// How often a kind of work comes around.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceInterval {
    pub months: u32,
    /// Absent for work that is only worth tracking by time.
    pub km: Option<i32>,
}

impl ServiceCategory {
    /// The maintenance interval, for the categories that have one.
    ///
    /// `None` means the work is **reactive** — electrical faults and body
    /// repair happen when something breaks, and a reminder for them would
    /// be noise pretending to be a schedule. `Other` is a catch-all and
    /// cannot carry an interval by definition.
    ///
    /// The figures are ordinary Indonesian service practice, not
    /// per-model manufacturer schedules. A real schedule varies by engine
    /// and by how the car is driven; when the catalog carries one, it
    /// overrides this — that is why the interval is returned rather than
    /// hard-coded into the reminder itself.
    #[must_use]
    pub const fn interval(self) -> Option<ServiceInterval> {
        match self {
            Self::EngineOil => Some(ServiceInterval {
                months: 6,
                km: Some(5_000),
            }),
            Self::TransmissionOil => Some(ServiceInterval {
                months: 24,
                km: Some(40_000),
            }),
            Self::Brakes | Self::Suspension | Self::TuneUp => Some(ServiceInterval {
                months: 12,
                km: Some(20_000),
            }),
            Self::AirConditioning => Some(ServiceInterval {
                months: 12,
                km: None,
            }),
            Self::Electrical | Self::Bodywork | Self::Other => None,
        }
    }
}

/// The most recent time a kind of work was done.
#[derive(Debug, Clone, Copy)]
pub struct LastService {
    pub category: ServiceCategory,
    pub date: Date,
    pub mileage_km: Option<i32>,
}

/// How much attention a reminder deserves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    /// Past due, by time or by distance.
    Overdue,
    /// Within a month, or within 500 km.
    Soon,
    /// Neither.
    Later,
}

/// One piece of work, and when it comes around again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reminder {
    pub category: ServiceCategory,
    pub due_on: Date,
    /// Negative when the date has passed.
    pub days_remaining: i64,
    /// Absent when the last service recorded no odometer reading.
    pub due_at_km: Option<i32>,
    /// Absent when either the due mileage or the current reading is unknown.
    pub km_remaining: Option<i32>,
    pub urgency: Urgency,
}

/// Within this much, a reminder is [`Urgency::Soon`].
const SOON_DAYS: i64 = 30;
const SOON_KM: i32 = 500;

/// What the car is due for, most urgent first.
///
/// `history` carries the latest service per category, not the whole
/// history — the caller reads it with one `DISTINCT ON` rather than
/// loading years of records to look at the top of each group.
///
/// `odometer_km` is the highest reading ever recorded for the car. It is
/// an estimate, and a deliberately conservative one: a car driven since
/// its last recorded service is further along than we know, so a distance
/// reminder here is never later than the truth, only earlier.
///
/// A category with no interval produces nothing. A category never
/// serviced produces nothing either — there is no last date to count
/// from, and inventing one from the car's purchase date would put a
/// confident due date on a car whose history simply has not been entered.
///
/// Complexity: `O(n log n)` over the number of categories with a
/// recorded service — at most nine, and the sort is what dominates.
#[must_use]
pub fn derive_reminders(
    history: &[LastService],
    odometer_km: Option<i32>,
    today: Date,
) -> Vec<Reminder> {
    let mut reminders: Vec<Reminder> = history
        .iter()
        .filter_map(|last| reminder_for(last, odometer_km, today))
        .collect();

    // Most urgent first, then by how soon — so the top of the list is
    // what to act on, and ties break to the thing due earliest.
    reminders.sort_by_key(|r| (r.urgency, r.days_remaining));
    reminders
}

fn reminder_for(last: &LastService, odometer_km: Option<i32>, today: Date) -> Option<Reminder> {
    let interval = last.category.interval()?;

    let due_on = add_months(last.date, interval.months);
    let days_remaining = (due_on - today).whole_days();

    let due_at_km = match (last.mileage_km, interval.km) {
        (Some(at), Some(every)) => Some(at.saturating_add(every)),
        _ => None,
    };
    let km_remaining = match (due_at_km, odometer_km) {
        (Some(due), Some(now)) => Some(due.saturating_sub(now)),
        _ => None,
    };

    // Either clock can make it due, and whichever runs out first wins.
    // A car that covers 20,000 km in four months is due on distance long
    // before the date, and one parked for two years is due on time
    // whatever the odometer says.
    let urgency = if days_remaining < 0 || km_remaining.is_some_and(|km| km < 0) {
        Urgency::Overdue
    } else if days_remaining <= SOON_DAYS || km_remaining.is_some_and(|km| km <= SOON_KM) {
        Urgency::Soon
    } else {
        Urgency::Later
    };

    Some(Reminder {
        category: last.category,
        due_on,
        days_remaining,
        due_at_km,
        km_remaining,
        urgency,
    })
}

/// `date` plus `months`, clamped to the end of the month it lands in.
///
/// `time` has no month arithmetic, and the reason is that months are not
/// a fixed length: 31 January plus one month has no 31 February to land
/// on. Clamping to the 28th, 29th, or 30th is what a person means when
/// they say "in a month", and it is what every calendar app does.
fn add_months(date: Date, months: u32) -> Date {
    let total = u32::from(u8::from(date.month())) - 1 + months;
    let year = date.year() + i32::try_from(total / 12).unwrap_or(0);
    let month =
        Month::try_from(u8::try_from(total % 12 + 1).unwrap_or(1)).unwrap_or(Month::January);

    let day = date.day().min(days_in(year, month));
    Date::from_calendar_date(year, month, day).unwrap_or(date)
}

fn days_in(year: i32, month: Month) -> u8 {
    time::util::days_in_month(month, year)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    fn last(category: ServiceCategory, on: Date, km: Option<i32>) -> LastService {
        LastService {
            category,
            date: on,
            mileage_km: km,
        }
    }

    #[test]
    fn a_month_added_to_the_end_of_january_lands_in_february() {
        // The case that has no valid answer and must not panic: there is
        // no 31 February, so it clamps to the end of the month.
        assert_eq!(add_months(date!(2026 - 01 - 31), 1), date!(2026 - 02 - 28));
        assert_eq!(add_months(date!(2024 - 01 - 31), 1), date!(2024 - 02 - 29));
    }

    #[test]
    fn months_roll_over_the_year() {
        assert_eq!(add_months(date!(2026 - 08 - 15), 6), date!(2027 - 02 - 15));
        assert_eq!(add_months(date!(2026 - 12 - 01), 24), date!(2028 - 12 - 01));
        assert_eq!(add_months(date!(2026 - 12 - 31), 1), date!(2027 - 01 - 31));
    }

    #[test]
    fn a_reactive_category_produces_no_reminder() {
        // Body repair happens when something is dented, not on a
        // schedule. A reminder here would be noise wearing a schedule's
        // clothes.
        for category in [
            ServiceCategory::Electrical,
            ServiceCategory::Bodywork,
            ServiceCategory::Other,
        ] {
            let history = [last(category, date!(2026 - 01 - 01), Some(100_000))];
            assert!(
                derive_reminders(&history, Some(100_000), date!(2026 - 08 - 16)).is_empty(),
                "{category:?} produced a reminder"
            );
        }
    }

    #[test]
    fn an_oil_change_falls_due_six_months_or_five_thousand_km_later() {
        let history = [last(
            ServiceCategory::EngineOil,
            date!(2026 - 01 - 10),
            Some(140_000),
        )];
        let reminders = derive_reminders(&history, Some(141_000), date!(2026 - 02 - 10));

        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].due_on, date!(2026 - 07 - 10));
        assert_eq!(reminders[0].due_at_km, Some(145_000));
        assert_eq!(reminders[0].km_remaining, Some(4_000));
        assert_eq!(reminders[0].urgency, Urgency::Later);
    }

    #[test]
    fn distance_can_make_a_service_due_long_before_the_date() {
        // A car that covers 5,000 km in a month needs its oil changed in
        // a month, whatever the calendar says. Treating the date as the
        // only clock is how a taxi ends up on a private car's schedule.
        let history = [last(
            ServiceCategory::EngineOil,
            date!(2026 - 07 - 20),
            Some(140_000),
        )];
        let reminders = derive_reminders(&history, Some(145_600), date!(2026 - 08 - 16));

        assert_eq!(reminders[0].km_remaining, Some(-600));
        assert_eq!(reminders[0].urgency, Urgency::Overdue);
        assert!(
            reminders[0].days_remaining > 0,
            "the date has not passed yet"
        );
    }

    #[test]
    fn time_can_make_a_service_due_on_a_car_that_is_barely_driven() {
        // The mirror case: parked for two years, well inside the
        // distance, still overdue.
        let history = [last(
            ServiceCategory::EngineOil,
            date!(2024 - 01 - 10),
            Some(140_000),
        )];
        let reminders = derive_reminders(&history, Some(140_100), date!(2026 - 08 - 16));

        assert!(reminders[0].days_remaining < 0);
        assert_eq!(reminders[0].km_remaining, Some(4_900));
        assert_eq!(reminders[0].urgency, Urgency::Overdue);
    }

    #[test]
    fn a_service_within_a_month_is_soon() {
        let history = [last(
            ServiceCategory::AirConditioning,
            date!(2025 - 09 - 01),
            None,
        )];
        let reminders = derive_reminders(&history, None, date!(2026 - 08 - 16));

        assert_eq!(reminders[0].urgency, Urgency::Soon);
        assert_eq!(reminders[0].days_remaining, 16);
    }

    #[test]
    fn a_category_with_no_distance_interval_reports_no_distance() {
        // Air conditioning is serviced by season, not by odometer.
        let history = [last(
            ServiceCategory::AirConditioning,
            date!(2026 - 01 - 01),
            Some(140_000),
        )];
        let reminders = derive_reminders(&history, Some(150_000), date!(2026 - 02 - 01));

        assert_eq!(reminders[0].due_at_km, None);
        assert_eq!(reminders[0].km_remaining, None);
    }

    #[test]
    fn an_unknown_odometer_leaves_the_distance_open_rather_than_guessing() {
        let history = [last(
            ServiceCategory::EngineOil,
            date!(2026 - 01 - 10),
            Some(140_000),
        )];
        let reminders = derive_reminders(&history, None, date!(2026 - 02 - 10));

        assert_eq!(
            reminders[0].due_at_km,
            Some(145_000),
            "the due reading is still known"
        );
        assert_eq!(
            reminders[0].km_remaining, None,
            "how far away it is, is not"
        );
        assert_eq!(
            reminders[0].urgency,
            Urgency::Later,
            "distance cannot make it urgent"
        );
    }

    #[test]
    fn a_service_with_no_recorded_mileage_still_reminds_by_date() {
        let history = [last(
            ServiceCategory::EngineOil,
            date!(2026 - 01 - 10),
            None,
        )];
        let reminders = derive_reminders(&history, Some(140_000), date!(2026 - 08 - 16));

        assert_eq!(reminders[0].due_at_km, None);
        assert_eq!(reminders[0].urgency, Urgency::Overdue);
    }

    #[test]
    fn the_most_urgent_comes_first() {
        let history = [
            last(
                ServiceCategory::AirConditioning,
                date!(2026 - 06 - 01),
                None,
            ), // Later
            last(
                ServiceCategory::EngineOil,
                date!(2024 - 01 - 10),
                Some(100_000),
            ), // Overdue
            last(
                ServiceCategory::Brakes,
                date!(2025 - 09 - 01),
                Some(139_000),
            ), // Soon
        ];
        let urgencies: Vec<_> = derive_reminders(&history, Some(140_000), date!(2026 - 08 - 16))
            .iter()
            .map(|r| r.urgency)
            .collect();

        assert_eq!(
            urgencies,
            vec![Urgency::Overdue, Urgency::Soon, Urgency::Later]
        );
    }

    #[test]
    fn two_overdue_services_order_by_how_overdue_they_are() {
        let history = [
            last(ServiceCategory::Brakes, date!(2025 - 01 - 01), None),
            last(ServiceCategory::EngineOil, date!(2020 - 01 - 01), None),
        ];
        let reminders = derive_reminders(&history, None, date!(2026 - 08 - 16));

        assert_eq!(
            reminders[0].category,
            ServiceCategory::EngineOil,
            "the older neglect first"
        );
        assert!(reminders[0].days_remaining < reminders[1].days_remaining);
    }

    #[test]
    fn a_car_with_no_history_is_due_for_nothing() {
        // Not "due for everything". Deriving a due date from the purchase
        // date would put a confident schedule on a car whose history
        // simply has not been entered yet.
        assert!(derive_reminders(&[], Some(140_000), date!(2026 - 08 - 16)).is_empty());
    }

    #[test]
    fn an_absurd_odometer_does_not_overflow() {
        let history = [last(
            ServiceCategory::EngineOil,
            date!(2026 - 01 - 10),
            Some(i32::MAX),
        )];
        let reminders = derive_reminders(&history, Some(i32::MIN), date!(2026 - 02 - 10));

        assert_eq!(
            reminders[0].due_at_km,
            Some(i32::MAX),
            "saturates rather than wrapping"
        );
        assert_eq!(reminders[0].urgency, Urgency::Later);
    }
}
