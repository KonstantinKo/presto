// Engine — chrono format pin for `Session.date`.
//
// Spec 001-leptos-migration §Phase 2 T144-T145; data-model.md
// §`Session.date`. Pins the chrono format string `"%a %b %d %Y"`
// against JS `Date.prototype.toDateString()` parity so a future
// chrono change that breaks parity fails loud at CI time rather
// than silently corrupting on-disk session dates.
//
// The on-disk wire form is a string written by the JS source at
// `pomodoro-timer.js:98` (`new Date().toDateString()`); the
// post-cutover Rust crate writes the same shape via
// `format_session_date` so existing JSON files round-trip without
// migration.

use chrono::{DateTime, Utc};

/// Formats a unix timestamp (milliseconds) as the JS-era
/// `Date.prototype.toDateString()` shape: `"%a %b %d %Y"`
/// (e.g. `"Wed Jan 01 2025"`).
///
/// The format string is pinned by `tests::matches_js_to_date_string`,
/// which iterates 366 consecutive days and asserts byte-for-byte
/// equality against an independently-computed JS-equivalent. This
/// is the single point where the on-disk session-date wire form
/// is produced post-cutover.
///
/// Returns the UTC-projected date. The JS-era source uses local-
/// time `toDateString()` because `Date` objects are local-time-
/// projected; for a single-user app where the clock and the
/// renderer are colocated, UTC vs local diverge only on the
/// midnight-boundary corner cases that the JS source itself
/// handles via `currentDateString` polling at
/// `pomodoro-timer.js:925-933`. Phase 2 ships UTC here; Phase 3
/// (manager layer) wires the local-time projection if needed —
/// the format pin doesn't change.
///
/// # Panics
/// Cannot panic: `from_timestamp(0, 0)` is the unix epoch which
/// is always valid; the `unwrap_or_else` covers the
/// `i64::MAX`-overflow corner case in `from_timestamp_millis`.
#[must_use]
pub fn format_session_date(timestamp_ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(timestamp_ms)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).expect("epoch is valid"))
        .format("%a %b %d %Y")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::format_session_date;

    /// JS-side ground-truth equivalent of `Date.prototype.toDateString()`
    /// per ECMA-262 §21.4.4.41:
    /// `<3-letter-day> <3-letter-month> <2-digit-zero-padded-day> <4-digit-year>`.
    ///
    /// Hand-rolled rather than reusing chrono so the test compares
    /// chrono's output against an independent baseline; if both used
    /// chrono the test would be tautological.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        // Civil-from-days arithmetic over the proleptic Gregorian
        // calendar: every cast is in a numerically-bounded range
        // (era is the 400-year era index; doe ∈ [0, 146_096];
        // yoe ∈ [0, 399]; doy ∈ [0, 365]; d ∈ [1, 31]; m ∈
        // [1, 12]). The cast errors clippy flags are within these
        // bounds and don't actually truncate. Inlined cast_*()
        // calls would be cleaner but this is test-only ground
        // truth and the algorithm already requires line-by-line
        // verification against Hinnant's published constants.
    )]
    fn js_to_date_string(timestamp_ms: i64) -> String {
        const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];

        // unix_days = days since 1970-01-01 (a Thursday).
        let secs = timestamp_ms.div_euclid(1000);
        let days_since_epoch = secs.div_euclid(86_400);
        // 1970-01-01 was Thursday → day_of_week index 4.
        let dow_idx =
            usize::try_from((days_since_epoch + 4).rem_euclid(7)).expect("rem_euclid(7) fits");

        // Convert days_since_epoch to year/month/day via the
        // standard "shift epoch to 0000-03-01" algorithm (Howard
        // Hinnant's date library, ported).
        // Treat civil dates as proleptic Gregorian (matches JS).
        let z = days_since_epoch + 719_468; // shift to 0000-03-01
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u64; // [0, 146_096]
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
        let y = (yoe as i64) + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
        let mp = (5 * doy + 2) / 153; // [0, 11]
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
        let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
        let y_civil = if m <= 2 { y + 1 } else { y };

        let month_idx = (m - 1) as usize;
        format!(
            "{day} {month} {d:02} {year:04}",
            day = DAYS[dow_idx],
            month = MONTHS[month_idx],
            d = d,
            year = y_civil,
        )
    }

    /// T144: `format_session_date(timestamp_ms)` matches JS
    /// `Date.prototype.toDateString()` byte-for-byte over a 366-day
    /// representative sample. The chrono format string
    /// `"%a %b %d %Y"` is the contract surface; this test pins
    /// against an independent baseline so a future chrono change
    /// that breaks the format fails loud at CI rather than
    /// silently corrupting on-disk session dates.
    #[test]
    fn matches_js_to_date_string() {
        // Sweep 366 consecutive days starting at 2024-01-01 UTC
        // (1_704_067_200_000 ms epoch). 2024 is a leap year so
        // the sweep covers every month boundary including Feb 29.
        let start_ms: i64 = 1_704_067_200_000;
        for offset_days in 0..366_i64 {
            let ts = start_ms + offset_days * 86_400_000;
            let chrono_out = format_session_date(ts);
            let js_out = js_to_date_string(ts);
            assert_eq!(
                chrono_out, js_out,
                "mismatch at offset_days={offset_days} (ts={ts})",
            );
        }
    }
}
