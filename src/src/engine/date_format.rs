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

/// Formats a unix timestamp (milliseconds) as the JS-era
/// `Date.prototype.toDateString()` shape: `"%a %b %d %Y"`
/// (e.g. `"Wed Jan 01 2025"`).
///
/// On `wasm32` this delegates to `js_sys::Date::new(ms).toDateString()`
/// so the output uses the user's **local** time zone — matching the
/// JS-era source which always used local-time `toDateString()`. On
/// all other targets (host-side tests, CI) it falls back to the
/// chrono-UTC path; the 366-day parity test
/// (`tests::matches_js_to_date_string`) pins that fallback.
///
/// This is the single point where the on-disk session-date wire form
/// is produced post-cutover. Both `synth_completed_session` in
/// `components::timer` and the `CalendarView` grouping route through
/// this helper so they remain consistent.
///
/// # Panics
/// Cannot panic: `from_timestamp(0, 0)` is the unix epoch which
/// is always valid; the `unwrap_or_else` covers the
/// `i64::MAX`-overflow corner case in `from_timestamp_millis`.
#[must_use]
pub fn format_session_date(timestamp_ms: i64) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        // Mirrors JS-era `new Date(ms).toDateString()` — local-time
        // projection. Both session-save producers MUST agree, so we
        // route both through this helper.
        #[allow(
            clippy::cast_precision_loss,
            reason = "Millisecond timestamps fit in f64 for realistic session dates."
        )]
        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp_ms as f64));
        d.to_date_string().as_string().unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use chrono::{DateTime, Utc};
        DateTime::<Utc>::from_timestamp_millis(timestamp_ms)
            .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).expect("epoch is valid"))
            .format("%a %b %d %Y")
            .to_string()
    }
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
        reason = "Civil-from-days arithmetic keeps every cast in a bounded calendar range; test oracle mirrors Hinnant constants."
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
