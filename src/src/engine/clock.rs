// Engine — abstract `Clock` trait.
//
// Spec 001-leptos-migration §Phase 2: the engine never reads the
// real wall clock directly (Principle I — engine is pure). Instead,
// callers thread a `Clock` impl through the engine API. The bridge
// layer wires a `WasmClock` backed by `js_sys::Date::now()` in
// production; the test harness uses `MockClock` to drive
// deterministic time arithmetic and drift-compensation scenarios.
//
// `now_ms()` returns the unix timestamp in milliseconds. The engine
// computes elapsed-time arithmetic in milliseconds and downsamples
// to seconds at the display boundary, matching the JS-side
// `Date.now()` arithmetic.

/// Abstract wall-clock source.
///
/// Returns the current unix timestamp in milliseconds. The contract
/// is monotonically non-decreasing for typical use; tests using
/// `MockClock` may step the clock arbitrarily to simulate OS
/// suspend / resume.
pub trait Clock {
    /// Current unix timestamp in milliseconds.
    fn now_ms(&self) -> i64;
}
