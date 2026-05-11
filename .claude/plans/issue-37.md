# Implementation Plan for #37

**Issue:** Timer engine: restore event-driven side effects + fix unit/clamp/timezone errors
**Type:** bug
**Branch:** agentex/37-timer-side-effects-fix

---

I have full context now. Output the plan below.

---

# Bug: Timer engine — restore event-driven side effects + fix unit/clamp/timezone errors

## Bug Description

The Leptos-port timer engine (`src/src/engine/timer.rs`) emits `TimerEvent` variants correctly, but most of the user-visible side effects that the JS-era app drove off those events are missing or wrong in the Rust port. Symptoms cluster into five behavioural groups:

1. **Notifications/sounds gone.** Focus completion no longer plays a chime or shows a system notification. No per-action in-app toast pings. No 2-minute / 30-second remaining warnings (toast + `.warning` class on `.timer-container`).
2. **Smart-pause broken.** Two compounding bugs: (a) `start_activity_monitoring(timeout_secs)` is called with `smart_pause_timeout * 60` — the field is seconds, so the default `30` is being passed as 1 800 s = 30 min of required idle; (b) nothing in the Leptos app subscribes to `user-activity` / `user-inactivity` Tauri events, so the engine's `observe_activity()` API is never invoked.
3. **Auto-start partially wired.** Auto-start fires after natural completion but not after Skip. `allow_continuous_sessions` is connected to a Settings field but the engine never reads it, so the JS-era overtime path (negative remaining, `.overtime` class, `+` title prefix, distinct tray icon) is dead code.
4. **`+5` clamp wrong.** `adjust_remaining_secs` clamps the upper bound at the configured mode duration, blocking power users from extending a focus session past 25 minutes. JS-era only floored at 1 s.
5. **Time zones wrong.** `synth_completed_session` formats `start_time` / `end_time` from `chrono::Utc`; `engine::date_format::format_session_date` does the same. A user in UTC+2 sees `12:00 – 12:25` for a 14:00 local session, and sessions near midnight file under the wrong calendar day.

Expected: the JS-era behaviour at `git show a0bb52c:src/core/pomodoro-timer.js` is the reference — events drive a chime + system notification, every state transition fires an in-app toast, smart-pause engages after the configured seconds of real idle, continuous sessions enter overtime instead of cutting to break, `+5` only floors at 1 s, and times displayed to the user are in local time.

## Problem Statement

The Leptos cutover preserved the engine state machine but dropped the consumer side of the event bus and slipped a unit conversion, a clamp ceiling, and two UTC formats into the port. Each gap has a single load-bearing call site; the fix is wiring + minimal engine surface extensions, not a rewrite.

## Solution Statement

- **Engine** (`src/src/engine/timer.rs`):
  - Drop the upper clamp in `adjust_remaining_secs` (`proposed.max(1)`, not `proposed.clamp(1, max_secs)`).
  - Add new `TimerEvent::TwoMinutesRemaining` and `TimerEvent::ThirtySecondsRemaining` variants emitted from `tick_drift_compensation` on the 120→≤120 and 30→≤30 crossings during focus mode.
  - Add `allow_continuous_sessions: bool` state (+ `set_allow_continuous_sessions(bool)` setter) and a `session_completed_but_not_saved: bool` flag. On the focus-mode zero-cross, when continuous is on: emit `PomodoroCompleted` + new `TimerEvent::OvertimeStarted { mode }`, re-anchor the wall clock with `timer_duration_secs = Some(0)` so subsequent ticks make `time_remaining_secs` go negative, and DO NOT change mode. Skip during overtime must not double-count.
- **App router** (`src/src/app.rs`):
  - Drop the `* 60` multiplier on `smart_pause_timeout` in the activity-monitor effect (the field is documented as seconds).
  - Add `events::listen::<()>` subscriptions for `USER_ACTIVITY` and `USER_INACTIVITY` that call `engine.observe_activity(ActivitySignal::Active|Idle, &BrowserClock)` against a context-provided `RwSignal<TimerState>`. To make that possible, lift the engine signal from inside `TimerView` to `App` via `provide_context`.
  - Add an app-level `Toast` signal (`AppToast`, modelled on `SettingsToast`) provided via context and rendered as `.notification-ping` at the App root.
- **Timer component** (`src/src/components/timer.rs`):
  - Consume engine events from each tick / start / pause / resume / skip / adjust call. Per event, push a toast, optionally play the audio chime, optionally fire a desktop notification, optionally toggle the `.warning` / `.overtime` class on `.timer-container`.
  - Add a 1-Hz `js_sys::AudioContext` chime helper (oscillator 800 Hz, 0.5 s sine envelope — port of the JS-era `playNotificationSound`), gated on `Settings.notifications.sound_notifications`.
  - Add a Tauri-notification helper (binds to `window.__TAURI__.notification.sendNotification` via wasm-bindgen; already mocked in `tests/e2e/fixtures/tauriMock.js`), gated on `Settings.notifications.desktop_notifications`.
  - Fix `on_skip` to read `Settings.notifications.auto_start_timer` and call `state.start(&BrowserClock)` after the skip mutation.
  - Pipe `Settings.notifications.allow_continuous_sessions` into the engine via an Effect that calls `engine.update(|s| s.set_allow_continuous_sessions(...))`.
  - Replace UTC formatting in `synth_completed_session` with `js_sys::Date::new` field extraction (`get_hours`/`get_minutes` on wasm32).
  - Wire `.warning` / `.overtime` classes on `.timer-container`. Surface the `+` overtime sign by deriving the displayed minutes/seconds off `i64` (negative) when overtime.
- **Date format** (`src/src/engine/date_format.rs`):
  - On `target_arch = "wasm32"`, project through `js_sys::Date::new(...).to_date_string()` so the day-grouping key matches JS-era `Date.prototype.toDateString()` (local time). Keep the chrono-UTC path on the host so the existing parity test still passes.
- **Notification bridge** (new `src/src/bridge/notification.rs`):
  - One `wasm_bindgen` extern binding `tauri_send_notification(opts: JsValue) -> js_sys::Promise` against `window.__TAURI__.notification.sendNotification`, plus a small Rust `pub async fn send_notification(title, body)` wrapper. Short-circuits when `bridge_available().is_absent()`.

## Steps to Reproduce

1. `cargo tauri dev` (or any working build that exposes the Tauri JS bridge — the bug is most observable in a real Tauri build because Settings persistence + system notifications round-trip through real plugins).
2. **Notifications/sounds**: start a focus session, force-shorten to a few seconds via the debug toggle (`Settings → Advanced → Debug mode` → 3 s timers), wait for zero-cross. No chime, no system notification, no completion toast.
3. **Smart-pause**: enable Smart-Pause in Settings (Notifications tab), set timeout to 30 (seconds). Start focus, walk away for 30 s. Engine does NOT auto-pause. Walk away 30 minutes — engine still doesn't auto-pause because nothing wires `user-inactivity` into the engine.
4. **Auto-start after skip**: enable "Auto-start sessions" in Settings → Notifications. Click `#skip-btn` mid-focus. Engine advances to Break and idles instead of starting Break.
5. **Continuous sessions**: enable "Allow continuous sessions" in Settings → Notifications. Start a focus session, let it cross zero. Engine cuts to Break, not overtime.
6. **+5 clamp**: start a focus session, immediately press `#timer-plus-btn` repeatedly. Display stops at 25:00 instead of climbing past.
7. **Timezone**: in UTC+2, finish a focus session at 14:00 local. Open Calendar — the session timeline row shows `12:00 – 12:25`. Run a session at 23:55 local time; it files under tomorrow's UTC date instead of today's.

## Root Cause Analysis

Each symptom maps to a single point of failure:

- **No notifications / sounds / toasts**: the tick closure inside `TimerView::Effect` (around `src/src/components/timer.rs:493-614`) discards `state.tick(&BrowserClock)`'s `Vec<TimerEvent>`. Likewise, `on_play_pause`, `on_skip`, and `on_adjust_*` discard the events from `pause`/`resume`/`start`/`skip`/`adjust_remaining_secs`. The events are emitted; nothing consumes them.
- **No 2-min / 30-sec warnings**: the engine's `tick_drift_compensation` doesn't have variants for the warning crossings, so even a perfect consumer wouldn't see them. JS source: `pomodoro-timer.js:758-775`.
- **Smart-pause multiplier**: `src/src/app.rs:347` reads `u64::from(s.notifications.smart_pause_timeout) * 60`. The settings field is documented as seconds (`bridge/types.rs:232`). Multiplying drops the user-set timeout into a 60× larger window.
- **No activity → engine wire**: `src/src/app.rs` subscribes to `UPDATE_AVAILABLE` and `GLOBAL_SHORTCUT` but never to `USER_ACTIVITY` / `USER_INACTIVITY`. The bridge-side `ActivityMonitor` emits them; nothing in WASM listens.
- **Auto-start after skip**: `TimerView::on_skip` (`components/timer.rs:455-459`) only calls `state.skip()`. The post-completion auto-start branch in the tick effect (lines 553-567) only triggers when the engine transitions out of running mid-tick; the engine `skip()` clears `is_running` to false immediately, so by the next tick `was_running` is already false and the branch doesn't fire.
- **Continuous sessions never overtime**: `TimerState` has no `allow_continuous_sessions` field. The Settings value is just ignored at the engine boundary.
- **`+5` ceiling**: `engine/timer.rs:456` clamps `proposed.clamp(1, max_secs)`. The JS-era `adjustTimer` only floored at zero; the upper bound was deliberately added to the Rust port and the rustdoc even calls it out — that decision was the regression.
- **Timezone UTC**: `synth_completed_session` formats `chrono::DateTime::<chrono::Utc>` (`components/timer.rs:111-129`). `engine::date_format::format_session_date` projects through `DateTime::<Utc>::from_timestamp_millis(...).format("%a %b %d %Y")` (`engine/date_format.rs:42-47`). The single-user app's calendar/timeline shows wall-clock-local values to the user; UTC is wrong for both producers.

## Relevant Files

Use these files to fix the bug:

- **`src/src/engine/timer.rs`** — engine state machine. Needs: drop upper clamp in `adjust_remaining_secs`; add `TwoMinutesRemaining` / `ThirtySecondsRemaining` / `OvertimeStarted` variants; add `allow_continuous_sessions` + `session_completed_but_not_saved` fields and the `set_allow_continuous_sessions` setter; modify `tick_drift_compensation` to gate the focus zero-cross on `!allow_continuous_sessions` and enter overtime when it's on; modify `skip()` to handle the overtime "already counted" case. Existing tests `adjust_remaining_adds_seconds_when_idle` and `adjust_remaining_rebases_anchor_when_running` need updating because they currently assert the ceiling clamp.
- **`src/src/engine/date_format.rs`** — `format_session_date`. Needs the wasm32 branch that delegates to `js_sys::Date::new(...).to_date_string()` for local-time parity. Keep the chrono-UTC host fallback so the existing parity test stays green.
- **`src/src/app.rs`** — App router. Needs: drop the `* 60` in the smart-pause Effect; add `USER_ACTIVITY` / `USER_INACTIVITY` `events::listen` subscriptions wired into a lifted `engine` signal; provide an app-level `AppToast` signal via context and render it as `.notification-ping` at the App root.
- **`src/src/components/timer.rs`** — `TimerView`. Needs: consume `state.tick()` / `start()` / `pause()` / `resume()` / `skip()` / `adjust_remaining_secs()` return values; route each `TimerEvent` to the toast surface + optional sound + optional desktop notification; fix `on_skip` to call `start()` when auto-start is on; thread `Settings.notifications.allow_continuous_sessions` into the engine; replace UTC formatting in `synth_completed_session` with `js_sys::Date`-based local time; add `.warning` and `.overtime` class derivations on `.timer-container`; pull the engine `RwSignal<TimerState>` out of `TimerView` and read it from context (so `app.rs` can dispatch activity signals into the same engine).
- **`src/src/bridge/types.rs`** — `NotificationSettings` field shapes. No changes — `allow_continuous_sessions`, `auto_start_timer`, `desktop_notifications`, `sound_notifications`, `smart_pause`, `smart_pause_timeout` already exist. Reference only.
- **`src/src/bridge/events.rs`** — event-name constants. No changes — `USER_ACTIVITY` and `USER_INACTIVITY` already declared (E1/E2). Reference only.
- **`src/src/bridge/mod.rs`** — register the new `notification` submodule (one-line `pub mod notification;`).
- **`tests/e2e/fixtures/tauriMock.js`** — already mocks `window.__TAURI__.notification.{sendNotification, isPermissionGranted, requestPermission}`. No changes needed; the new bridge wrapper consumes the existing mock surface.
- **`src/style/animations.css`** — `.timer-container.warning .timer-display` rule already exists at line 144. No CSS change required.
- **`src/style/timer.css`** — `.container.overtime` rules exist (line 457-473) but reference `.container.overtime` not `.timer-container.overtime`. The Leptos root for the timer's container is `<div class="timer-container">`. Either add a `.timer-container.overtime` CSS rule (cheap, two selectors) or apply the class to whichever element matches the existing rule. See "Step by Step Tasks".

### New Files

- **`src/src/bridge/notification.rs`** — typed wrapper around `window.__TAURI__.notification.sendNotification`. Mirrors the shape of existing wrappers in `bridge/commands.rs` (one wasm-bindgen extern binding + one `pub async fn`). Module-level allow `clippy::future_not_send` for the same wasm-only reason as `commands.rs`.

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom.

### Task 1 — Engine: write failing tests for the new behaviour (RED)

Add the following tests to `src/src/engine/timer.rs` `#[cfg(test)] mod tests` BEFORE touching production engine code. TDD posture per Constitution Principle V: the timer engine state machine is in scope.

- `adjust_remaining_does_not_clamp_at_mode_duration` — start with focus 1500, call `adjust_remaining_secs(300, &clock)` four times; assert `time_remaining_secs() == 1500 + 4*300 = 2700`. (Replaces the existing `adjust_remaining_adds_seconds_when_idle` assertion that the ceiling holds at 1500.) Update the existing test name as well.
- `two_minutes_warning_fires_on_120_crossing_focus_only` — start focus with `Durations { focus: 240, ... }`, tick to advance past 120 → 119 boundary, assert `events.iter().any(|e| matches!(e, TimerEvent::TwoMinutesRemaining))`. Repeat starting in Break mode; assert the variant is NOT emitted.
- `thirty_seconds_warning_fires_on_30_crossing` — analogous to the 2-min test with a 60-s focus duration; tick past 30 → 29; assert variant present.
- `continuous_focus_zero_cross_enters_overtime` — set `allow_continuous_sessions = true` via setter; start focus 60 s; advance 61 s; tick. Assert `events` contains `PomodoroCompleted` and `OvertimeStarted { mode: TimerMode::Focus }`; assert `current_mode() == TimerMode::Focus` (no mode flip); assert `is_running()`. Advance 5 more seconds; tick; assert `state.time_remaining_secs_signed() == -5` (or whatever accessor name we choose — see Task 3 below).
- `continuous_skip_during_overtime_does_not_double_count` — drive the engine into overtime as above (completed = 1 after zero-cross). Call `state.skip()`. Assert `completed_pomodoros() == 1` (not 2). Assert the engine advanced to `TimerMode::Break`.

Run `cargo test --workspace --frozen -- --nocapture` to confirm every new test FAILS with the current production code. Do not edit production code yet.

### Task 2 — Engine: add new event variants

Edit `src/src/engine/timer.rs`. Extend `enum TimerEvent` with:

```rust
TwoMinutesRemaining,
ThirtySecondsRemaining,
OvertimeStarted { mode: TimerMode },
```

Document each variant alongside the existing ones (JS-era source line refs: `pomodoro-timer.js:758-775` for the warnings, `:776-785` for the overtime branch). Note in the variant rustdoc that `OvertimeStarted` fires once on the zero-cross; subsequent ticks merely advance `time_remaining_secs` further negative.

### Task 3 — Engine: drop the `+5` ceiling

In `adjust_remaining_secs`, replace `let clamped = proposed.clamp(1, max_secs);` with `let clamped = proposed.max(1);` and remove the now-unused `max_secs` local (and its `for_mode` lookup). Update the rustdoc to match: "Floors at 1 second; no upper bound — the JS-era `adjustTimer` only floored at zero, allowing power users to run longer-than-configured sessions."

### Task 4 — Engine: continuous-sessions + overtime path

Add fields to `TimerState`:

```rust
allow_continuous_sessions: bool,
session_completed_but_not_saved: bool,
```

Initialise both to `false` in `new`. Add setter:

```rust
pub const fn set_allow_continuous_sessions(&mut self, enabled: bool) {
    self.allow_continuous_sessions = enabled;
}
```

Modify `tick_drift_compensation`'s zero-cross block so the focus arm splits:

- When `self.allow_continuous_sessions && self.current_mode == TimerMode::Focus`:
  - increment `completed_pomodoros`, integrate `current_session_elapsed_secs` into `total_focus_secs`, reset `current_session_elapsed_secs = 0`.
  - emit `PomodoroCompleted` and `OvertimeStarted { mode: TimerMode::Focus }`.
  - set `self.session_completed_but_not_saved = true`.
  - DO NOT change mode; DO NOT clear `is_running` / `timer_start_ms` / `timer_duration_secs`. Instead, re-anchor so subsequent ticks count negative: set `self.timer_start_ms = Some(now_ms)`; `self.timer_duration_secs = Some(0)`. (Threading `now_ms` into `tick_drift_compensation` requires adding it as a parameter; pass `clock.now_ms()` from `tick`.)
- When `self.allow_continuous_sessions && self.current_mode != TimerMode::Focus` (break/long-break overtime): re-anchor as above; emit `OvertimeStarted { mode: current_mode }`. No `PomodoroCompleted`, no accumulator changes.
- Otherwise: the existing non-continuous path (mode flip, clear `is_running`, etc.).

Modify `skip` so that when `self.session_completed_but_not_saved && self.current_mode == TimerMode::Focus`:
- DO NOT increment `completed_pomodoros` again (the zero-cross already did).
- DO NOT integrate `current_session_elapsed_secs` again into `total_focus_secs`.
- Still advance the mode (Break / LongBreak depending on the modulo-4 check).
- Clear `self.session_completed_but_not_saved`.

Reset `session_completed_but_not_saved` in `reset()` for completeness.

### Task 5 — Engine: 2-minute / 30-second warning events

In `tick_drift_compensation`, after computing `old_remaining` / `new_remaining` and only when `self.current_mode == TimerMode::Focus`:

```rust
if old_remaining > 120 && new_remaining <= 120 && new_remaining > 0 {
    events.push(TimerEvent::TwoMinutesRemaining);
}
if old_remaining > 30 && new_remaining <= 30 && new_remaining > 0 {
    events.push(TimerEvent::ThirtySecondsRemaining);
}
```

Place these BEFORE the zero-cross block so a tick that crosses both 120 and 0 emits the warning AND the completion. The `new_remaining > 0` guard prevents the warning from firing when the same tick also crosses zero.

### Task 6 — Engine: re-run tests, verify GREEN

`cargo test --workspace --frozen` — all engine tests including the new ones pass. The existing `adjust_remaining_rebases_anchor_when_running` test needs its "clamped at 1500" assertions updated (post-fix, +5 from 1495 yields 1795).

### Task 7 — Bridge: notification plugin wrapper (new file)

Create `src/src/bridge/notification.rs`:

```rust
// Bridge wrapper around tauri-plugin-notification's JS API
// (window.__TAURI__.notification.sendNotification).
//
// Tauri's notification plugin is exposed via the higher-level
// __TAURI__.notification.* surface rather than __TAURI_INTERNALS__.invoke.
// The existing tauriMock.js fixture mocks the JS-level surface so this
// wrapper works under both real Tauri builds and the e2e mock harness.
#![allow(clippy::future_not_send)]

use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::availability::bridge_available;
use super::error::BridgeError;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(
        js_namespace = ["__TAURI__", "notification"],
        js_name = sendNotification,
        catch
    )]
    fn tauri_send_notification(opts: JsValue) -> Result<JsValue, JsValue>;
}

#[derive(Serialize)]
struct NotificationOpts<'a> {
    title: &'a str,
    body: &'a str,
}

/// Best-effort: returns `BridgeError::BridgeUnavailable` if the Tauri
/// bridge is absent; returns `BridgeError::Internal` if the plugin call
/// rejects. Callers should generally ignore errors — a missing chime is
/// not fatal.
pub async fn send_notification(title: &str, body: &str) -> Result<(), BridgeError> {
    if bridge_available().is_absent() {
        return Err(BridgeError::BridgeUnavailable);
    }
    let opts = serde_wasm_bindgen::to_value(&NotificationOpts { title, body }).map_err(|e| {
        BridgeError::SerdeRoundtrip {
            command: "sendNotification".into(),
            error: format!("serialise opts: {e}"),
        }
    })?;
    // sendNotification returns synchronously in v2 but we treat it as
    // possibly-async (future-compat). If it returns a Promise, await it;
    // otherwise drop the immediate value.
    match tauri_send_notification(opts) {
        Ok(result) => {
            if let Ok(promise) = result.clone().dyn_into::<js_sys::Promise>() {
                JsFuture::from(promise).await.map_err(|e| BridgeError::Internal {
                    msg: format!("sendNotification rejected: {e:?}"),
                })?;
            }
            Ok(())
        }
        Err(e) => Err(BridgeError::Internal {
            msg: format!("sendNotification failed at bridge boundary: {e:?}"),
        }),
    }
}
```

Register in `src/src/bridge/mod.rs`: add `pub mod notification;`.

### Task 8 — App: lift engine signal to context + AppToast

Edit `src/src/app.rs`:

- Create `RwSignal<TimerState>` at the App level (initialised from `Settings::default()` durations); call `provide_context`. `TimerView` will switch to `use_context` for this signal (Task 9).
- Add a `#[derive(Clone, Copy)] pub struct AppToast { messages: RwSignal<Vec<(u64, String)>> }` (each toast carries an id for dismissal). Provide a `show(self, text: impl Into<String>)` method that pushes. The reasoning for `Vec` over the SettingsToast's single-`Option` shape: the JS-era surface queued pings (line ~109 of `style/notifications.css` references stacking via `.notification-ping+.notification-ping`). Auto-dismiss each entry after 2 s via per-entry `set_timeout_with_handle`.
- Render the toast container at the App root (sibling of `<UpdateNotification/>`):

```rust
<div class="notification-container">
    <For
        each=move || app_toast.messages.get()
        key=|(id, _)| *id
        children=move |(_, text)| view! { <div class="notification-ping" role="status">{text}</div> }
    />
</div>
```

- Provide `AppToast` via context so `TimerView` (and any other component that wants to ping) can use it.

### Task 9 — App: fix smart-pause multiplier + wire activity events into the engine

Still in `app.rs`:

- In the existing smart-pause Effect (around line 344-368), change `let timeout_secs = settings.with(|s| u64::from(s.notifications.smart_pause_timeout) * 60);` to `let timeout_secs = settings.with(|s| u64::from(s.notifications.smart_pause_timeout));`.
- Add two new `spawn_local` blocks alongside the existing `events::listen` calls:

```rust
spawn_local({
    let engine = engine;
    async move {
        let listener = events::listen::<serde_json::Value>(USER_ACTIVITY, move |_| {
            engine.update(|state| { let _ = state.observe_activity(ActivitySignal::Active, &BrowserClock); });
        }).await;
        if let Ok(guard) = listener { Box::leak(Box::new(guard)); }
    }
});
spawn_local({
    let engine = engine;
    async move {
        let listener = events::listen::<serde_json::Value>(USER_INACTIVITY, move |_| {
            engine.update(|state| { let _ = state.observe_activity(ActivitySignal::Idle, &BrowserClock); });
        }).await;
        if let Ok(guard) = listener { Box::leak(Box::new(guard)); }
    }
});
```

`USER_ACTIVITY` / `USER_INACTIVITY` carry `()` per the const rustdoc; `listen::<serde_json::Value>` is the unit-payload-tolerant shape that survives any future widening. Move `use` declarations as needed.

Also wire `Settings.notifications.smart_pause` into the engine: add a small Effect that calls `engine.update(|s| s.set_smart_pause_enabled(settings.with(|s| s.notifications.smart_pause)))`. Without this the engine's `observe_activity(Idle)` short-circuits because the engine's own `smart_pause_enabled` flag stays false.

### Task 10 — Date format: local-time on wasm32

Edit `src/src/engine/date_format.rs`:

```rust
#[must_use]
pub fn format_session_date(timestamp_ms: i64) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        // Mirrors JS-era `new Date(ms).toDateString()` — local-time
        // projection. This is the single point where the on-disk
        // session-date wire form is produced post-cutover; both
        // producers (TimerView's synth_completed_session and the
        // Calendar grouping) MUST agree, so we route both through
        // this helper.
        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp_ms as f64));
        return d.to_date_string().as_string().unwrap_or_default();
    }
    #[cfg(not(target_arch = "wasm32"))]
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp_ms)
        .unwrap_or_else(|| chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).expect("epoch is valid"))
        .format("%a %b %d %Y")
        .to_string()
}
```

`js_sys` is allowed inside the engine — the engine-purity gate (`scripts/check-engine-purity.sh`) greps for `web_sys|web-sys` only; `js_sys` is the JS-runtime layer (no DOM), and Principle I bans DOM reads, not JS-runtime reads. The existing `matches_js_to_date_string` host-side test continues to run on the chrono-UTC fallback and passes unchanged.

### Task 11 — Timer component: local-time `synth_completed_session`

Edit `src/src/components/timer.rs`. Replace the `chrono`-based body of `synth_completed_session` with:

```rust
fn synth_completed_session(now_ms: i64, focus_duration_secs: u32) -> ManualSession {
    let (hh_end, mm_end) = local_hh_mm(now_ms);
    let start_ms = now_ms - i64::from(focus_duration_secs) * 1000;
    let (hh_start, mm_start) = local_hh_mm(start_ms);
    ManualSession {
        id: format!("session-{now_ms}"),
        session_type: SessionType::Focus,
        duration: focus_duration_secs.div_euclid(60).max(1),
        start_time: format!("{hh_start:02}:{mm_start:02}"),
        end_time: format!("{hh_end:02}:{mm_end:02}"),
        notes: None,
        // RFC3339 on `created_at` stays UTC by intent — it's a serialised
        // timestamp, not a user-facing label. The day-grouping `date`
        // routes through the shared format_session_date helper so it
        // stays consistent with CalendarView.
        created_at: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
            .unwrap_or_default()
            .to_rfc3339(),
        date: crate::engine::date_format::format_session_date(now_ms),
        tags: None,
    }
}

#[cfg(target_arch = "wasm32")]
fn local_hh_mm(ms: i64) -> (u32, u32) {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    (d.get_hours(), d.get_minutes())
}

#[cfg(not(target_arch = "wasm32"))]
fn local_hh_mm(_ms: i64) -> (u32, u32) {
    (0, 0)
}
```

`get_hours` / `get_minutes` are local-time per the ECMAScript spec.

Likewise, replace the `chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms).format("%a %b %d %Y")` block in the tick effect (around `components/timer.rs:535-540`, where it builds `date_str` for `save_daily_stats`) with `crate::engine::date_format::format_session_date(now_ms)`.

### Task 12 — Timer component: consume engine events + drive side effects

This is the largest delta but mechanical. In `TimerView`:

- Replace `let engine = RwSignal::new(...)` with `let engine = use_context::<RwSignal<TimerState>>().unwrap_or_else(|| RwSignal::new(TimerState::new(initial_durations)));`.
- Replace `let toast = ...` plumbing (none today at TimerView level) with `let app_toast = use_context::<AppToast>().unwrap_or_default();`.
- Capture each engine call's event vec and route to side-effects via a helper:

```rust
fn handle_events(events: &[TimerEvent], settings: &Settings, toast: AppToast, warning_signal: RwSignal<bool>) {
    use TimerEvent::*;
    for e in events {
        match e {
            PomodoroCompleted { .. } => {
                toast.show("Pomodoro completed! Take a break 😌");
                if settings.notifications.sound_notifications { play_chime(); }
                if settings.notifications.desktop_notifications {
                    spawn_local(async {
                        let _ = bridge::notification::send_notification(
                            "Presto", "Focus session complete — take a break").await;
                    });
                }
                warning_signal.set(false);
            }
            TwoMinutesRemaining => { toast.show("2 minutes remaining! 🔥"); warning_signal.set(true); }
            ThirtySecondsRemaining => { toast.show("30 seconds left! ⏰"); warning_signal.set(true); }
            SessionPaused => toast.show("Timer paused ⏸️"),
            SessionResumed => toast.show("Timer resumed ▶️"),
            SessionSkipped { skipped_mode, .. } => {
                toast.show(match skipped_mode {
                    TimerMode::Focus => "Focus session skipped 😌",
                    TimerMode::Break => "Break skipped — ready to focus? 🍅",
                    TimerMode::LongBreak => "Long break skipped — back to work 🚀",
                });
                warning_signal.set(false);
            }
            AutoPaused => toast.show("Smart Pause: timer paused due to inactivity ⏸️"),
            AutoResumed => toast.show("Welcome back! Timer resumed ▶️"),
            ManualSessionRecorded { .. } => toast.show("Manual session recorded"),
            OvertimeStarted { .. } => {
                toast.show("Pomodoro completed! Continue working or take a break 🍅");
                if settings.notifications.sound_notifications { play_chime(); }
                if settings.notifications.desktop_notifications {
                    spawn_local(async {
                        let _ = bridge::notification::send_notification(
                            "Presto", "Focus session complete — overtime started").await;
                    });
                }
            }
        }
    }
}
```

Call `handle_events(&events, &settings.get_untracked(), app_toast, warning_signal)` from inside each engine mutation closure (`on_play_pause` for `pause`/`resume`/`start`, `on_skip` for `skip`, the tick interval body for `tick`, and `on_adjust_*` for symmetry though `adjust_remaining_secs` emits no events today). `warning_signal` is a new `RwSignal<bool>` local to the component that toggles the `.warning` class on `.timer-container`. Clear it on `PomodoroCompleted`, `SessionSkipped`, mode change, or any path that resets countdown above 120 s.

Add an `is_overtime` derived signal:

```rust
let is_overtime = Signal::derive(move || engine.with(|s| s.time_remaining_secs() == 0 && s.is_running()));
```

(Engine's `time_remaining_secs() -> u32` clamps negatives to 0; we'll need a signed accessor for the displayed-minus path — see Task 13.)

Apply both classes on `<div class="timer-container" class:warning=move || warning_signal.get() class:overtime=move || is_overtime.get() ...>`.

### Task 13 — Engine: signed `time_remaining_secs` accessor for overtime display

The current `time_remaining_secs() -> u32` clamps negatives to 0, which hides overtime from the UI. Add a sibling accessor:

```rust
#[must_use]
pub const fn time_remaining_secs_signed(&self) -> i64 {
    self.time_remaining_secs
}
```

In `TimerView`, derive `is_overtime` off this signed accessor (`< 0`) and compute the displayed minutes/seconds from `time_remaining_secs_signed().abs() as u32`, prefixing the title (`document.title`) with `+` when overtime. The `#timer-minutes` / `#timer-seconds` text stay unsigned; the sign is conveyed by the `.overtime` class + the title prefix (mirrors `pomodoro-timer.js:1617-1618`).

### Task 14 — Timer component: auto-start after skip

In `on_skip`, after the engine mutation:

```rust
let on_skip = move |_| {
    engine.update(|state| {
        let events = state.skip();
        handle_events(&events, &settings.get_untracked(), app_toast, warning_signal);
    });
    if settings.with_untracked(|s| s.notifications.auto_start_timer) {
        engine.update(|state| {
            let events = state.start(&BrowserClock).map(|()| Vec::new()).unwrap_or_default();
            handle_events(&events, &settings.get_untracked(), app_toast, warning_signal);
        });
    }
};
```

The JS-era surface used a 1.5 s setTimeout for parity; matching that requires `set_timeout_with_handle` (Leptos's helper). Per the issue ("acceptable but not required"), an immediate start is fine for the bug fix. If we add the delay, gate it on a non-test build to avoid Playwright timing flakiness — the e2e suite already waits for `#pause-icon` visibility, which only needs the engine to be running.

### Task 15 — Timer component: pipe `allow_continuous_sessions` to engine

In `TimerView`, add an Effect alongside the existing `set_durations` Effect:

```rust
Effect::new(move |_| {
    let enabled = settings.with(|s| s.notifications.allow_continuous_sessions);
    engine.update(|state| state.set_allow_continuous_sessions(enabled));
});
```

### Task 16 — Audio chime helper

Add to `src/src/components/timer.rs` (or split into `components/audio.rs` if it grows):

```rust
#[cfg(target_arch = "wasm32")]
fn play_chime() {
    use web_sys::{AudioContext, OscillatorType};
    let Ok(ctx) = AudioContext::new() else { return };
    let Ok(osc) = ctx.create_oscillator() else { return };
    let Ok(gain) = ctx.create_gain() else { return };
    osc.set_type(OscillatorType::Sine);
    osc.frequency().set_value(800.0);
    let now = ctx.current_time();
    let _ = gain.gain().set_value_at_time(0.3, now);
    let _ = gain.gain().exponential_ramp_to_value_at_time(0.01, now + 0.5);
    let _ = osc.connect_with_audio_node(&gain);
    let _ = gain.connect_with_audio_node(&ctx.destination());
    let _ = osc.start();
    let _ = osc.stop_with_when(now + 0.5);
}

#[cfg(not(target_arch = "wasm32"))]
fn play_chime() {}
```

Confirm `web_sys` features needed (`AudioContext`, `OscillatorType`, `GainNode`, `OscillatorNode`, `AudioNode`, `AudioParam`, `AudioDestinationNode`) are enabled in `src/Cargo.toml`. If any are missing, add to the `web-sys` features list. This lives in `components/` not `engine/` so the engine-purity gate is unaffected.

### Task 17 — CSS: `.timer-container.overtime`

The existing `.container.overtime` rules in `src/style/timer.css:457-473` target the wrong selector (`.container` doesn't exist on the timer view). Add `.timer-container.overtime` mirror rules (or rename the selector — the only consumer is this bug fix, so renaming is preferable). The diff is two-to-three selector renames; no new CSS rules. Verify with the visual-regression baselines — overtime is not captured in any of the 14 baselines (the engine starts idle in Focus mode pre-shot), so no baseline updates are required.

### Task 18 — Run gates and e2e

- `cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic`
- `cargo fmt --all --check`
- `cargo test --workspace --frozen`
- `(cd src && wasm-pack test --node)` — wasm-bindgen-tests covering `bridge::events` + `bridge::commands` signature pins still pass; nothing new requires wasm tests beyond compilation.
- `bash scripts/check-engine-purity.sh` — must stay green (`js_sys` is allowed; only `web_sys`/`web-sys` triggers the gate; the new `web_sys` usage lives in `components/`, not `engine/`).
- `bash scripts/check-mock-drift.sh` — must stay green (no new `#[tauri::command]` handlers; we use the existing notification plugin surface, already mocked).
- `(cd tests/e2e && npx playwright test settings-automation.spec.js)` — exercises auto-start after focus completion; should still pass.
- `(cd tests/e2e && npx playwright test timer.spec.js _smoke.spec.js)` — sanity for start/pause/skip; should still pass.
- `(cd tests/e2e && npx playwright test visual-regression.spec.js)` — should still pass with NO baseline drift (the timer view shape is identical pre-start; the `.warning` and `.overtime` classes only attach mid-session).

### Task 19 — Manual smoke (no e2e coverage)

Run `cargo tauri dev` and walk through the reproduction steps in §"Steps to Reproduce":

1. Enable debug mode (3 s timers). Start focus. Verify chime + system notification + "Pomodoro completed!" toast on completion.
2. Enable Smart Pause, set timeout to 5 s for quick test. Start focus, don't touch the machine. After 5 s, verify auto-pause toast + tray state (where applicable).
3. Enable Auto-start. Click Skip. Verify the next mode auto-starts.
4. Enable Continuous Sessions. Start focus (debug mode). Verify `time_remaining_secs` continues past zero with `+` prefix in title bar and `.overtime` class on the timer container.
5. Press `+5` repeatedly during a fresh focus session. Verify the display climbs past 25:00.
6. Set system timezone to UTC+2, finish a session at 14:00 local. Verify the Calendar timeline row reads `14:00 – 14:25`, not `12:00 – 12:25`.

## Validation Commands

Execute every command to validate the bug is fixed with zero regressions.

- `cargo test --workspace --frozen -- --nocapture` — every new engine test passes; existing engine tests stay green (with adjusted `adjust_remaining_*` assertions reflecting the dropped ceiling).
- `cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic` — no new clippy warnings.
- `cargo fmt --all --check` — no formatting drift.
- `(cd src && wasm-pack test --node)` — wasm-bindgen tests in `bridge::events` and `bridge::commands` compile and pass; the new `bridge::notification` module compiles (no tests required — it's a thin shim).
- `bash scripts/check-engine-purity.sh` — engine module stays pure (`web_sys`/`web-sys` not referenced under `src/src/engine/`).
- `bash scripts/check-mock-drift.sh` — mock surface unchanged (no new `#[tauri::command]` handlers).
- `(cd tests/e2e && npx playwright test)` — full e2e suite green, including visual regression (14 baselines unchanged).
- `cargo tauri build` (release path) — production build succeeds; no link errors from new `web_sys` features.

## Notes

- **Engine purity**: the engine module gains a `js_sys::Date` call in `date_format.rs` (wasm32 only). The CI gate (`scripts/check-engine-purity.sh`) greps `web_sys|web-sys` only; `js_sys` is the JS-runtime layer, not the DOM. Principle I ("the engine never reads from the DOM") is preserved — `js_sys::Date::new(ms)` reads a deterministic transformation of an input we already have, not a live DOM state. If a future amendment broadens the purity rule, `format_session_date` should accept a `tz_offset_minutes: i32` parameter and let the call site read the offset.
- **Toast queue vs. settings toast**: the existing `SettingsToast` deliberately stores a single `Option<&'static str>` because the settings flow shows one transient banner. The app-level `AppToast` queues `Vec<(u64, String)>` because the timer flow can fire two warnings (2-min then 30-sec) plus a completion within seconds. Both surfaces target `role="alert"` / `.notification-ping` so the e2e suite's existing assertions resolve against either.
- **Continuous-session overtime save semantics**: the JS-era `skipSession` during overtime saves the elapsed-including-overtime session before advancing. The Rust port's `skip()` already emits `SessionSkipped { elapsed_secs }` carrying `current_session_elapsed_secs`. The persistence sink in `TimerView` is responsible for honouring "save only if elapsed > 60 s" (mirrors JS `:1088-1090`); that gating is not part of this fix. Today's TimerView pushes a `synth_completed_session` on `PomodoroCompleted` only, and `OvertimeStarted` will reuse the same `PomodoroCompleted` already emitted on the focus zero-cross, so the session row lands at zero-cross time. The "save again with overtime included" enhancement is out of scope for this bug — the immediate goal is restoring the missing chime / toast / overtime mode, not perfecting the overtime persistence math.
- **Smart-pause defaults**: `NotificationSettings::default().smart_pause_timeout = 30` and the doc-comment says "Seconds." Confirm via `cargo run` against the `Default::default()` settings; this is consistent across `src-tauri/src/lib.rs:411-447`'s mirror.
- **TDD posture**: every engine-state change in Tasks 1-6 is preceded by a failing test (Constitution Principle V). UI plumbing (Tasks 8-17) is exempt by the same principle; the e2e suite + visual regression are the contract.
- **Risk surface**: lifting the `RwSignal<TimerState>` from `TimerView` to `App` is the touchiest change because every reference to `engine` inside `TimerView` flips from local owned signal to `use_context`. The `use_context::<RwSignal<TimerState>>().unwrap_or_else(|| ...)` fallback keeps host-side `cargo test` and direct-mount Storybook-style consumers working unchanged.

---
*Generated by Agentex*
