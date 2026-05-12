// `day_clamp::clamp_day_to_month` — Bundle B helper. Pure time-keeping
// math (Principle V exception per A1): the helper rolls a calendar
// day-of-month into a target month, clamping to the last day of that
// month when the input exceeds the month's length.
//
// Test-first per the spec — RED commit lands stubs; GREEN replaces
// the body with the real `NaiveDate::from_ymd_opt` path.

use chrono::{DateTime, Utc};

/// Roll `day_of_month` into `target_month`'s year+month, clamping to
/// the last day of that month if `day_of_month` exceeds it.
///
/// Direction-agnostic: prev-month and next-month navigation both hit
/// the same clamp path (e.g. May 31 → June 30 is the same as July 31
/// → June 30 reached via prev-month).
///
/// The returned `DateTime<Utc>` preserves the time-of-day from
/// `target_month` (consumers downstream only inspect the calendar
/// date — `format_session_date` discards the time fragment).
#[must_use]
#[allow(
    clippy::missing_const_for_fn,
    reason = "RED-state stub (T006); GREEN commit (T007) replaces the body with the real NaiveDate::from_ymd_opt path that is not const-eligible."
)]
#[allow(
    unused_variables,
    reason = "RED-state stub (T006); GREEN commit (T007) consumes both arguments."
)]
pub fn clamp_day_to_month(day_of_month: u32, target_month: DateTime<Utc>) -> DateTime<Utc> {
    // RED-state stub: returns the target_month unchanged so the
    // helper compiles but every test case fails (the days don't
    // match). The GREEN commit (T007) replaces this with the real
    // ymd_opt-fallback path.
    target_month
}

#[cfg(test)]
mod tests {
    use super::clamp_day_to_month;
    use chrono::{Datelike, TimeZone, Utc};

    #[test]
    fn clamp_no_clamp_may_31() {
        // May has 31 days; day-of-month 31 in May 2026 stays at May 31.
        let target = Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap();
        let result = clamp_day_to_month(31, target);
        assert_eq!(result.year(), 2026);
        assert_eq!(result.month(), 5);
        assert_eq!(result.day(), 31);
    }

    #[test]
    fn clamp_june_31_to_june_30() {
        // June has 30 days; day-of-month 31 clamps to June 30.
        let target = Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();
        let result = clamp_day_to_month(31, target);
        assert_eq!(result.year(), 2026);
        assert_eq!(result.month(), 6);
        assert_eq!(result.day(), 30);
    }

    #[test]
    fn clamp_feb_31_leap_year() {
        // 2024 is a leap year; February has 29 days; day-of-month 31
        // clamps to Feb 29.
        let target = Utc.with_ymd_and_hms(2024, 2, 15, 12, 0, 0).unwrap();
        let result = clamp_day_to_month(31, target);
        assert_eq!(result.year(), 2024);
        assert_eq!(result.month(), 2);
        assert_eq!(result.day(), 29);
    }

    #[test]
    fn clamp_feb_31_non_leap() {
        // 2025 is not a leap year; February has 28 days; day-of-month
        // 31 clamps to Feb 28.
        let target = Utc.with_ymd_and_hms(2025, 2, 15, 12, 0, 0).unwrap();
        let result = clamp_day_to_month(31, target);
        assert_eq!(result.year(), 2025);
        assert_eq!(result.month(), 2);
        assert_eq!(result.day(), 28);
    }

    #[test]
    fn clamp_low_boundary() {
        // day-of-month 1 in any month is always valid; no clamp.
        let target = Utc.with_ymd_and_hms(2026, 2, 15, 12, 0, 0).unwrap();
        let result = clamp_day_to_month(1, target);
        assert_eq!(result.year(), 2026);
        assert_eq!(result.month(), 2);
        assert_eq!(result.day(), 1);
    }

    #[test]
    fn clamp_backward_nav_july31_to_june() {
        // Reaching June from July 31 via prev-month hits the same
        // clamp path: day-of-month 31 in June 2026 clamps to June 30.
        // (Direction-agnostic — the helper sees day_of_month + target,
        // not the navigation direction.)
        let target = Utc.with_ymd_and_hms(2026, 6, 30, 0, 0, 0).unwrap();
        let result = clamp_day_to_month(31, target);
        assert_eq!(result.year(), 2026);
        assert_eq!(result.month(), 6);
        assert_eq!(result.day(), 30);
    }
}
