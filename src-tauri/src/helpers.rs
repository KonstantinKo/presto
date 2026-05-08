use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Returns `true` when `action` was last called within `window` of `now`,
/// and records `now` as the latest call time otherwise.
///
/// Extracting this logic as a pure function (caller-supplied state) makes
/// it trivially testable without touching the global `SHORTCUT_DEBOUNCE` mutex.
#[must_use]
// pub(super) is the correct visibility here: the function is used by the parent module
// (lib.rs) and its descendants (the test module), but not from anywhere else in the crate.
// clippy::redundant_pub_crate fires because the enclosing module is private; however,
// pub(super) is intentionally more restrictive than pub(crate).
#[allow(clippy::redundant_pub_crate)]
pub(super) fn is_debounced(
    map: &mut HashMap<String, Instant>,
    action: &str,
    now: Instant,
    window: Duration,
) -> bool {
    if let Some(last) = map.get(action) {
        if now.duration_since(*last) < window {
            return true;
        }
    }
    map.insert(action.to_owned(), now);
    false
}

#[cfg(test)]
mod tests {
    use super::is_debounced;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    #[test]
    fn first_call_records_time_and_returns_false() {
        let mut map = HashMap::new();
        let now = Instant::now();
        assert!(!is_debounced(
            &mut map,
            "action",
            now,
            Duration::from_millis(500)
        ));
        assert!(map.contains_key("action"));
    }

    #[test]
    fn immediate_second_call_is_debounced() {
        let mut map = HashMap::new();
        let now = Instant::now();
        let window = Duration::from_millis(500);
        assert!(!is_debounced(&mut map, "action", now, window));
        assert!(is_debounced(&mut map, "action", now, window));
    }

    #[test]
    fn call_after_window_expires_is_not_debounced() {
        let mut map = HashMap::new();
        let now = Instant::now();
        let window = Duration::from_millis(500);
        assert!(!is_debounced(&mut map, "action", now, window));
        let later = now + Duration::from_millis(600);
        assert!(!is_debounced(&mut map, "action", later, window));
    }

    #[test]
    fn different_actions_are_independent() {
        let mut map = HashMap::new();
        let now = Instant::now();
        let window = Duration::from_millis(500);
        assert!(!is_debounced(&mut map, "a1", now, window));
        assert!(!is_debounced(&mut map, "a2", now, window));
        assert!(is_debounced(&mut map, "a1", now, window));
    }
}
