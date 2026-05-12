// Shared time-keeping helpers consumed by view modules under
// `components::*`. Promoted in feature 003 (T003) from
// `components::calendar` so `components::stats` and
// `components::daily` can both seed their `RwSignal<DateTime<Utc>>`
// cursors from a single source.

use chrono::{DateTime, Utc};

/// Lift a unix-timestamp (milliseconds) to a `DateTime<Utc>`.
///
/// Falls back to the unix epoch on the corner case where
/// `from_timestamp_millis` rejects the input. Defensive at the system
/// boundary (Principle III): the caller passes raw `i64` from
/// `BrowserClock::now_ms`; we project to a typed `DateTime<Utc>` here
/// and downstream code never re-reads the raw epoch.
///
/// # Panics
///
/// Panics only if `DateTime::<Utc>::from_timestamp(0, 0)` itself
/// returns `None`, which is impossible: the unix epoch is always a
/// valid `DateTime<Utc>`. The `expect` is a structural assertion, not
/// a runtime gate.
#[must_use]
pub fn datetime_from_ms(now_ms: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(now_ms)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).expect("epoch is valid"))
}

#[cfg(test)]
mod tests {
    use super::datetime_from_ms;
    use chrono::{Datelike, Timelike};

    #[test]
    fn datetime_from_ms_zero_is_unix_epoch() {
        let dt = datetime_from_ms(0);
        assert_eq!(dt.year(), 1970);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 1);
        assert_eq!(dt.hour(), 0);
    }

    #[test]
    fn datetime_from_ms_negative_falls_back_to_epoch_or_returns_pre_epoch() {
        // chrono can represent pre-epoch timestamps, so a small
        // negative value is a legal input. We only assert the helper
        // does not panic.
        let _ = datetime_from_ms(-1);
    }
}
