use crate::engine::clock::Clock;

pub(super) struct BrowserClock;

impl Clock for BrowserClock {
    #[cfg(target_arch = "wasm32")]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        // `Date.now()` is f64 milliseconds since the unix epoch;
        // values up to year 2038 fit easily within i64 (and even
        // i53 — the f64 mantissa). The cast is safe for any
        // realistic wall-clock value during the engine's lifetime.
    )]
    fn now_ms(&self) -> i64 {
        js_sys::Date::now() as i64
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn now_ms(&self) -> i64 {
        // Host-side fallback for `cargo test` / `cargo clippy`
        // builds. The component is never mounted on the host
        // target — the binary is wasm-only — so this body is
        // unreachable under real execution. Returning a constant
        // keeps the trait satisfied without pulling `std::time`
        // into the wasm target's dependency graph.
        0
    }
}
