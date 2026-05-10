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
