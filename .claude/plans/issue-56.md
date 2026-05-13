# Implementation Plan for #56

**Issue:** Notification chime unreliable on macOS (start + transition sounds intermittent)
**Type:** bug
**Branch:** agentex/56-fix-macos-chime-sounds

---

Investigation complete. Root cause is clear: `play_chime` constructs a fresh `AudioContext` per call (one each fire), never calls `.resume()`. Metronome works because it caches a singleton context (lines 415-416). On macOS WKWebView fresh contexts start `suspended` and `.resume()` is required to unlock playback — particularly when scheduled outside a user-gesture stack (transition chimes fire from the 1Hz `setInterval` tick, never a gesture). Browsers also cap concurrent `AudioContext` instances (~4-6 on WebKit), so even the start chime under a gesture sporadically fails once prior contexts pile up unreleased.

Below is the complete plan.

```md
# Bug: macOS chime sounds unreliable (start + transition chimes intermittent)

## Bug Description
On macOS, the notification chime (`play_chime` in `src/src/components/timer/mod.rs:381`) fires unreliably:
- **Start chime** often silent even when the window has focus and the call is downstream of the user's Start click.
- **Transition chimes** (focus→break, break→focus, long-break) intermittently silent.
- The opt-in **metronome tick** (Feature 002) plays reliably — only the one-shot `play_chime` is affected.
- Expected: every event gated on `settings.notifications.sound_notifications` plays an audible 800 Hz sine ping.
- Actual: silence in roughly the majority of cases on macOS WKWebView.

Symptom is macOS-specific per the issue; Linux/Windows are out of scope.

## Problem Statement
`play_chime` constructs a fresh `web_sys::AudioContext` on every call and never calls `.resume()` on it. On macOS WKWebView:
1. A freshly-constructed `AudioContext` starts in the `suspended` state unless creation happens inside a live user-gesture event handler — and even then, scheduling oscillators reliably requires an explicit `.resume()`.
2. Transition chimes fire from the 1 Hz `setInterval` tick (`Effect::new(... set_interval_with_handle(...) ...)` at `src/src/components/timer/mod.rs:1220-1372`), which is **not** a user-gesture context, so a freshly-created context cannot be unlocked from there at all.
3. WebKit caps concurrent `AudioContext` instances (≈4–6); rapidly leaking new contexts across a session (chime, then transition, then overtime, …) eventually returns silently-failing constructions.

The metronome tick avoids all three failure modes by caching a singleton `AudioContext` in a `thread_local!` (`src/src/components/timer/mod.rs:415-422`) — the first metronome tick fires within ~1 s of the Start click (still inside the gesture-trust window), unlocks the cached context, and every subsequent tick reuses the same unlocked context.

## Solution Statement
Mirror the metronome's pattern in `play_chime`:
1. **Cache the `AudioContext`** in a module-local `thread_local!<RefCell<Option<AudioContext>>>` so exactly one context exists for the lifetime of the WASM module.
2. **Resume the cached context on every call** by invoking `ctx.resume()` (its returned `Promise` may be ignored — it is idempotent on a `running` context and unlocks a `suspended` context whenever current document focus + prior-gesture state permit).
3. **Prime the cached context on the user's Start click** by adding a small `prime_audio_context()` helper called from `on_play_pause` (line 1105) and from the keyboard-shortcut start path (line 808). This guarantees the cached context is created inside a user gesture even on flows where the FIRST audible chime is a transition (e.g. `auto_start_timer` rolls focus→break without an interim click).

This is the smallest change that addresses the confirmed root cause and aligns `play_chime`'s lifecycle with the metronome's known-good pattern. No new files, no audio assets, no notification-plugin routing.

## Steps to Reproduce
1. `cargo tauri dev` on macOS with `Settings → Notifications → Sound notifications` ON (the default).
2. Click **Start** on the timer.
3. **Expected**: an audible 800 Hz sine ping on the Start click; another at focus → break; another at break → focus; another at the long-break boundary.
4. **Actual**: the Start chime is silent (often); the transition chime is silent (often); the metronome tick (if enabled) plays fine.
5. Open Tauri DevTools (right-click → Inspect Element). At a `play_chime` call site, log `AudioContext.state` immediately after `AudioContext::new()` — on macOS WKWebView it is `"suspended"` outside a fresh gesture.

## Root Cause Analysis
`src/src/components/timer/mod.rs:381-399`:

```rust
fn play_chime() {
    use web_sys::{AudioContext, OscillatorType};
    let Ok(ctx) = AudioContext::new() else { return };  // (1) fresh per call
    let Ok(osc) = ctx.create_oscillator() else { return };
    ...
    let _ = osc.start();                                 // (2) no `.resume()` before scheduling
    let _ = osc.stop_with_when(now + 0.5);
}
```

Two cooperative defects:
- **(1) per-call construction.** Each invocation allocates a new `AudioContext`. Macros pile up: by the time a user has run a few focus/break cycles, the WebKit per-page `AudioContext` budget is exhausted and `AudioContext::new()` returns `Err` (silently swallowed by the `let Ok(...) else { return }`). The metronome avoids this by caching its context (`src/src/components/timer/mod.rs:415-422`).
- **(2) no `.resume()`.** A freshly-constructed `AudioContext` on macOS WKWebView starts in the `suspended` state unless creation is inside the synchronous call stack of a `click`/`keydown` handler — and even there, `.resume()` is the documented unlock primitive. The 1 Hz tick interval drops the gesture context entirely, so transition chimes never unlock.

The start-chime intermittence (issue's counter-evidence: "fails immediately after clicking Start, which IS a user gesture") is explained by **(1)** — once prior contexts have leaked, even the in-gesture construction fails. Confirmed by browser/WebKit behavior; not yet measured in this repo, but the fix addresses both vectors regardless of which dominates per session.

The bug-report hypothesis "**tauri-plugin-notification interference**" is set aside: the Start chime path (`TimerEvent::SessionStarted` at `src/src/components/timer/mod.rs:538-543`) fires NO desktop notification at all, so plugin collision cannot explain its silence. The Start chime symptom must be a Web Audio defect — which is what this fix targets.

## Relevant Files
Use these files to fix the bug:

- **`src/src/components/timer/mod.rs`** — contains `play_chime` (line 381), the cached `play_metronome_tick` pattern to mirror (line 412), all five call sites (lines 501, 520, 541, 548, 564), and the user-gesture entry points to prime the context (`on_play_pause` at line 1105, the keyboard-shortcut start path at line 810).
- **`src/Cargo.toml`** — `[dependencies.web-sys]` features list (lines 32-74); needs `AudioContextState` added so we can inspect the cached context's state if we choose to gate the resume call.
- **`src/src/components/timer/mod.rs` (test module at bottom)** — host for a new wasm-bindgen-test that asserts the cached AudioContext singleton invariant (one allocation across N calls).

### New Files
None — the fix is in-place in `play_chime` and `on_play_pause`/`on_play_pause`-equivalent click handlers.

## Step by Step Tasks
IMPORTANT: Execute every step in order, top to bottom.

### 1. Reproduce + confirm root cause in a Tauri dev build (macOS only)
- `cargo tauri dev` on a macOS host.
- Open DevTools (right-click → Inspect Element).
- Temporarily instrument `play_chime` to `web_sys::console::log_1(&format!("chime ctx state: {:?}", ctx.state()).into());` immediately after `AudioContext::new()`.
- Run a focus session through to a transition. Confirm:
  - Start chime path logs either `suspended` (autoplay-policy hit) or an `Err` from `AudioContext::new()` (budget hit).
  - Transition chime (tick-driven) logs `suspended`.
- Revert the instrumentation before moving on. The instrumentation exists only to confirm the root cause; the fix doesn't depend on the exact log output.

### 2. TDD: failing wasm-bindgen-test for the cached-singleton invariant
Per the constitution's Principle V (test-first for stateful engines / managers), AND the existing convention that the chime is a UI side-effect rather than engine state, we add a minimal **wasm-bindgen-test** that pins the cached-context invariant. The test can run via `(cd src && wasm-pack test --node)`.

- Extract a tiny `chime_audio_context()` accessor that returns a clone of the cached `AudioContext` from the `thread_local!` (None if uninitialised). Mark `#[cfg(test)]` so it doesn't bloat the release build.
- New `#[wasm_bindgen_test]` `play_chime_reuses_audio_context_across_calls`:
  - Call `play_chime()` three times.
  - Assert `chime_audio_context()` returns `Some(ctx)`.
  - Snapshot the `JsValue` of the first context, call `play_chime()` again, snapshot again, assert pointer-equal (or assert `Rc::strong_count` if we use `Rc` internally — match the metronome's pattern).
- Run the test — **expect it to fail** (today's `play_chime` doesn't cache).
- Commit the failing test (Principle V's RED-before-GREEN ordering — see `AGENTS.md §Test-first commit ordering`).

### 3. Add `AudioContextState` to the `web-sys` feature list
- Edit `src/Cargo.toml` lines 60-69: add `"AudioContextState"` so we can read `ctx.state()` if we choose to short-circuit the resume call. Order alphabetically with the existing audio features.
- Run `cargo build --frozen` to confirm `Cargo.lock` does not drift (no new transitive deps from a feature flag).

### 4. Refactor `play_chime` to mirror the metronome's cached-singleton pattern
In `src/src/components/timer/mod.rs:381-399`:

- Replace the body with the same `thread_local!<RefCell<Option<AudioContext>>>` pattern used by `play_metronome_tick` at lines 415-422.
- After acquiring the cached context, call `let _ = ctx.resume();` (returns a `Promise`; ignore it — `.resume()` is idempotent on a `running` context and unlocks `suspended` whenever WKWebView's autoplay policy permits).
- Keep the existing oscillator / gain / envelope wiring identical (800 Hz sine, 0.5 s exp decay, 0.3 gain peak) — visual / audio contract for the chime tone stays exactly the same.
- Keep the `#[cfg(target_arch = "wasm32")]` / `#[cfg(not(target_arch = "wasm32"))]` split intact.

Resulting structure (illustrative — match the metronome's exact thread_local closure shape):

```rust
#[cfg(target_arch = "wasm32")]
fn play_chime() {
    use std::cell::RefCell;
    use web_sys::{AudioContext, OscillatorType};
    thread_local! {
        static CTX: RefCell<Option<AudioContext>> = const { RefCell::new(None) };
    }
    CTX.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = AudioContext::new().ok();
        }
        let Some(ctx) = slot.as_ref() else { return };
        let _ = ctx.resume();
        let Ok(osc) = ctx.create_oscillator() else { return };
        let Ok(gain) = ctx.create_gain() else { return };
        osc.set_type(OscillatorType::Sine);
        osc.frequency().set_value(800.0);
        let now = ctx.current_time();
        let _ = gain.gain().set_value_at_time(0.3, now);
        let _ = gain
            .gain()
            .exponential_ramp_to_value_at_time(0.01, now + 0.5);
        let _ = osc.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&ctx.destination());
        let _ = osc.start();
        let _ = osc.stop_with_when(now + 0.5);
    });
}
```

### 5. Add a user-gesture primer for the auto_start_timer flow
A small `prime_audio_context()` helper that touches the same `CTX` thread_local (instantiates + resumes if uninstantiated/suspended). Call it from inside the click + keyboard-shortcut start handlers — i.e. **on every Start/Resume click**, not just on first ever call — so the cached context is unconditionally created inside a live gesture:

- New `#[cfg(target_arch = "wasm32")] fn prime_audio_context()` right after `play_chime` — same `CTX.with(...)` block, but it only constructs+resumes; no oscillator scheduling. (Alternatively: extract the construct-+-resume preamble into a shared helper and have both `play_chime` and `prime_audio_context` call it. Whichever keeps both fns short.)
- A `#[cfg(not(target_arch = "wasm32"))] const fn prime_audio_context() {}` no-op for the host-test build.
- Inside `on_play_pause` (`src/src/components/timer/mod.rs:1105`): call `prime_audio_context()` as the first line of the closure, BEFORE the `engine.try_update`. This guarantees the cached context is constructed inside the click's synchronous call stack regardless of whether `play_chime` ends up being called on this tick.
- Inside the keyboard-shortcut start branch at `src/src/components/timer/mod.rs:810`: same — call `prime_audio_context()` as the first statement inside the `if matches_shortcut || matches_space { ... }` block.
- The audio context lives once primed, so no need to prime on every event; but calling on every Start/Resume click is cheap (resume is idempotent) and defends against the rare flow where the *first* sound the user is meant to hear is a tick-driven transition (e.g. user toggles `auto_start_timer` ON, dismisses focus to a break without an interim click — the FIRST chime would be the break completed beep).

### 6. Re-run the wasm-bindgen-test from step 2
- `(cd src && wasm-pack test --node)`.
- The test now passes — the cached singleton invariant is enforced.
- Commit the implementation (GREEN commit follows the RED commit from step 2 — see `AGENTS.md §Test-first commit ordering`).

### 7. Tauri-mock parity check
- No new Tauri commands are introduced, so `tests/e2e/fixtures/tauriMock.js` does not need to change (`bash scripts/check-mock-drift.sh` should remain green).

### 8. e2e visual-regression check
- `(cd tests/e2e && npx playwright test visual-regression.spec.js)`.
- Expected: green — no DOM/CSS change, only an internal helper rewrite.
- If diffs appear, investigate and fix the actual cause; do NOT regenerate baselines without explicit visual review (per `tests/e2e/CLAUDE.md §Updating visual baselines`).

### 9. Lint + format sweep
- `cargo fmt --all --check`.
- `cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic`.
- The `thread_local!` + `RefCell` pattern matches the metronome's existing pattern, which already satisfies pedantic; no new `#[allow]` should be needed. If clippy::pedantic flags the new fn for cognitive-complexity, refactor by extracting the construct-+-resume preamble (the cleaner of the two solutions discussed in step 5) rather than adding `#[allow]`.

### 10. Manual macOS verification (the irreducible step)
This bug is macOS-specific; no headless / CI environment exercises the WKWebView audio path. The fix's validation is a deterministic manual run:

- `cargo tauri dev` on a macOS host.
- Click Start → audible 800 Hz ping (Start chime works).
- Wait through to focus → break → audible ping (transition chime works).
- Click Skip → audible ping (overtime / break-completed paths work).
- Toggle `Settings → Notifications → Sound notifications` OFF → no chime on subsequent transitions.
- Toggle back ON → chimes resume.
- Open DevTools console, inspect `AudioContext.state` at the call sites — should now read `running` in every case after the first Start click.

Document the manual-test pass in the PR body (per `tests/e2e/CLAUDE.md`-style "visual review" precedent — this is the audio analogue).

## Validation Commands
Execute every command to validate the bug is fixed with zero regressions.

```bash
# 1. Frontend wasm tests (covers the new cached-singleton invariant test).
(cd src && wasm-pack test --node)

# 2. Host-side workspace tests (engine + manager state machines + persistence helpers).
cargo test --workspace --frozen

# 3. Strict-deny pedantic + nursery lint pass across both crates.
cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic

# 4. Format gate.
cargo fmt --all --check

# 5. e2e suite (selectors + flows + visual regression).
(cd tests/e2e && npm ci && npx playwright test)

# 6. Visual-regression slice (in case a single failure obscures it above).
(cd tests/e2e && npx playwright test visual-regression.spec.js)

# 7. Tauri-mock parity gate (no new Tauri commands; should remain green).
bash scripts/check-mock-drift.sh

# 8. Lockfile drift gate (Cargo.toml ↔ Cargo.lock pair after web-sys feature add).
bash scripts/check-lockfile-drift.sh   # or equivalent target if named differently — see `scripts/`

# 9. Manual macOS run — the irreducible audio verification step.
cargo tauri dev
# Click Start → expect chime. Wait through focus → break → expect chime. Toggle sound off/on.
# Open DevTools and confirm AudioContext.state === "running" at every play_chime call site.
```

## Notes
- **Bundle / dep cost**: adding `"AudioContextState"` to the `web-sys` features list is a single-symbol increment; no new transitive crates. Negligible WASM size impact.
- **Tone parity**: the oscillator wiring (800 Hz sine, 0.5 s exp decay, 0.3 peak gain) is unchanged byte-for-byte. Users who liked / disliked the chime tone get exactly the same tone — only its delivery becomes reliable.
- **Why not route through `tauri-plugin-notification`'s native-sound parameter (hypothesis listed in the issue)?** It would bypass WKWebView's autoplay policy entirely BUT (a) requires fanning out per-platform sound files, (b) only fires when a desktop notification also fires — the `SessionStarted` chime path (`src/src/components/timer/mod.rs:538-543`) intentionally emits no notification, so it would still need a Web Audio fallback. The cached-singleton fix solves all five call sites uniformly with one diff; the plugin route is strictly bigger for no marginal correctness gain.
- **Why not preloaded `<audio>` element with a vendored MP3?** Bigger bundle, vendored asset, baseline-update required for any settings preview button that surfaces the chime tone, CSP risk. The cached-singleton fix is closer to the existing metronome pattern and is the surgical minimum.
- **TDD scope**: the chime is a UI side-effect of engine events, not engine state. The engine already has a regression test pinning `SessionStarted` emission (`src/src/engine/timer.rs:1851` `start_emits_session_started_event`) — that test is intact. The new wasm-bindgen-test pins the UI-side cached-singleton invariant, which is the layer where the bug lives.
- **Smart-pause / auto-resume interactions**: `AutoResumed` does NOT trigger `play_chime` today (see `src/src/components/timer/mod.rs:558` — it shows a toast only); this fix does not change that behavior. `SessionResumed` (manual resume) does chime (line 547) — same code path as Start, same fix applies.
- **No upstream-merge consideration** per `CLAUDE.md §Conventions` (`No upstream compatibility burden`).
```

---
*Generated by Agentex*
