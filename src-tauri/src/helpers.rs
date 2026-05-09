use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Serializes `value` as pretty-printed JSON and atomically writes it to `path`.
///
/// Writes to a sibling `.tmp` file first, then renames on success, preventing
/// partial writes from corrupting the target file on crash or power loss.
///
/// # Errors
///
/// Returns an error string if serialization, temp-file write, or rename fails.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize JSON: {e}"))?;
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, json.as_bytes())
        .map_err(|e| format!("Failed to write temp file: {e}"))?;
    std::fs::rename(&tmp_path, path).map_err(|e| format!("Failed to persist file: {e}"))?;
    Ok(())
}

/// Acquires a `Mutex` lock, recovering from a poisoned state if necessary.
///
/// If the mutex was poisoned by a prior panicking holder, logs a warning and
/// returns the inner value rather than propagating the panic.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| {
        log::warn!("recovering poisoned mutex");
        e.into_inner()
    })
}

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
        let now = Instant::now();
        let window = Duration::from_millis(500);

        let mut map = HashMap::new();
        assert!(!is_debounced(&mut map, "action", now, window));
        let later = now + Duration::from_millis(600);
        assert!(!is_debounced(&mut map, "action", later, window));

        // elapsed == window (strict less-than boundary): also not debounced
        let mut map2 = HashMap::new();
        assert!(!is_debounced(&mut map2, "action", now, window));
        let equal_to_window = now + window;
        assert!(!is_debounced(&mut map2, "action", equal_to_window, window));
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
