# Implementation Plan for #40

**Issue:** macOS: system tray icon does not appear at runtime
**Type:** bug
**Branch:** agentex/40-macos-tray-icon-fix

---

Root cause confirmed by docs research: `TrayIconBuilder::with_id("main")` in `src-tauri/src/lib.rs:1003` never calls `.icon(...)`. Tauri 2.x does NOT auto-load `default_window_icon` for trays created via the builder API — on macOS, the resulting `NSStatusItem` has no image and no title at creation, so it renders zero-width and is invisible.

# Bug: macOS system tray icon does not appear at runtime

## Bug Description
On macOS, Presto's menu-bar (system tray) icon does not appear after the Leptos rewrite. The `TrayIconBuilder::with_id("main")` call in `src-tauri/src/lib.rs` still runs at startup with a full menu (`show`, `start_session`, `pause`, `skip`, `cancel`, `quit`) and event handlers wired, but the `NSStatusItem` it produces has zero width and is therefore not interactable in the menu bar.

- **Expected**: A tray icon appears in the macOS menu bar on launch (dev *and* production builds), persists across the timer lifecycle, and exposes the right-click menu plus the mode-aware title (e.g. "🧠 25:00").
- **Actual**: No tray entry appears at any point — launch, start, pause, skip, completion, restart.
- The same Tauri-side code shape worked pre-Leptos (commit `a0bb52c`), so the regression came in with the Leptos cutover plus the maintenance pass.

## Problem Statement
`TrayIconBuilder` is constructed without an `.icon(...)` call. Tauri 2.6.0's `TrayIconBuilder::build_inner` (`crates/tauri/src/tray/mod.rs`) only forwards `self.icon` to the underlying `tray-icon` crate — there is **no fallback** to `default_window_icon`, contrary to a widely-held assumption. The `trayIcon` field of `tauri.conf.json` would auto-create a tray, but we are creating ours manually, so that path doesn't apply either.

The `tray-icon` v0.20 backend on macOS (`src/platform_impl/macos/mod.rs::TrayIcon::create`) creates the `NSStatusItem` with `NSVariableStatusItemLength`. When neither image nor title is set, `set_title_inner` is a no-op (it only forwards `Some` titles), so the status item collapses to ~0 pt width — present in the AppKit tree, but invisible and un-clickable. See upstream issue [tauri-apps/tauri#11931](https://github.com/tauri-apps/tauri/issues/11931); the maintainer-recommended workaround is exactly an explicit `.icon(app.default_window_icon().unwrap().clone())`.

Why the regression appeared with the Leptos rewrite: in the JS era the frontend invoked `updateTrayIcon` synchronously on first render (setting a non-empty title like "🧠 25:00"), which widened the `NSStatusItem` enough to be visible. The Leptos `timer.rs` only fires `commands::update_tray_icon` when `mode_changed || running_changed` (`src/src/components/timer.rs:793`), so on a cold boot the status item never receives a title and stays at zero width. The maintenance pass (`6a062ec`) deleted the `set_dock_visibility` Tauri command and inlined `set_dock_visibility_native` calls — that change didn't cause the bug directly, but it removed an early macOS-side `setActivationPolicy_` round-trip that, in dev mode, used to incidentally force a status-bar reflow.

## Solution Statement
Pass an explicit icon to `TrayIconBuilder` using the bundle's `default_window_icon`. This gives the `NSStatusItem` a real image at creation time, so it is visible regardless of whether or when `update_tray_icon` fires from the frontend. The existing `update_tray_icon` flow (title + tooltip overlay) stays untouched — it continues to layer the mode emoji and `mm:ss` on top of the icon once the timer state changes.

Fall back gracefully if `default_window_icon` is `None` (cannot happen with the current bundle config, but the guard keeps the build resilient if someone later strips the bundle icons).

## Steps to Reproduce
1. Check out `agentex/40-macos-tray-icon-fix` (current `HEAD`).
2. On macOS, run `cargo tauri dev` **or** `cargo tauri build` followed by launching the resulting `.app`.
3. Inspect the macOS menu bar — no Presto entry.
4. In the running app, click Start / Pause / Skip / wait for completion → tray entry still does not appear.
5. Quit and relaunch — still no tray entry.

## Root Cause Analysis
`TrayIconBuilder::with_id("main").menu(&menu).show_menu_on_left_click(true).on_menu_event(...).on_tray_icon_event(...).build(app)?` in `src-tauri/src/lib.rs:1003-1038` omits `.icon(...)`. Tauri 2.6.0's builder does not back-fill the icon from `default_window_icon` (verified against `crates/tauri/src/tray/mod.rs` at tag `tauri-v2.6.0`, `build_inner` lines ~314-345). The resulting macOS `NSStatusItem` therefore boots with no image and `set_title(None)` — i.e. zero visible content, zero width, no hit area.

The bug is dormant in any code path that supplies a title to the tray *before* the user observes the menu bar. In the JS-era code that effectively happened at first render. In the Leptos code path the first `update_tray_icon` invocation requires a `mode_changed || running_changed` transition, which doesn't occur on cold boot, so the tray never widens.

## Relevant Files
Use these files to fix the bug:

- `src-tauri/src/lib.rs` — the offending `TrayIconBuilder::with_id("main")` block lives in the `setup(|app| { ... })` closure at `lib.rs:1003`. The fix is local to the builder chain (one added `.icon(...)` call, plus a short comment explaining the macOS reason).
- `src-tauri/tauri.conf.json` — confirms `bundle.icon` is populated (`icons/32x32.png` … `icons/icon.icns` …); `default_window_icon` will therefore be `Some`. No edits required.
- `src-tauri/icons/` — already contains `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico`. No edits required; the maintenance pass did not remove any of these.
- `src-tauri/capabilities/default.json` — does not require an explicit tray permission (tray creation/manipulation from Rust does not go through the permission system; only JS-side `tray-icon` plugin calls do). No edits required; confirmed by review.
- `src/src/components/timer.rs` — context: the only frontend call site for `update_tray_icon` (`timer.rs:813`), gated behind `mode_changed || running_changed`. We deliberately do **not** change this gate; the fix lives below the bridge so the tray is visible regardless of frontend timing.
- `Cargo.lock` — pins `tauri 2.6.0` / `tray-icon 0.20`; informs the chosen fallback API surface. No edits required.

### New Files
None. The fix is a one-file Rust change.

## Step by Step Tasks

### 1. Read the current TrayIconBuilder block and confirm context
- Open `src-tauri/src/lib.rs` and re-read the block at `lib.rs:1000-1038` so the edit lands cleanly:
  - `let app_handle = app.handle().clone();` precedes the builder.
  - `let app_handle_for_click = app_handle.clone();` is the second handle.
  - The builder chain ends with `.build(app)?;` on `lib.rs:1038`.
- Confirm `app.default_window_icon()` is reachable inside the `setup` closure (it is — `app: &mut App<R>` provides `default_window_icon()` via `Manager`).

### 2. Apply the surgical fix (TDD-aware)

TDD note: a true regression test for "tray icon visible on macOS" cannot run in CI (Playwright/wasm-bindgen-test can't introspect `NSStatusItem` width, and `cargo test` cannot spin up an AppKit run loop). The closest mechanical check is a compile-time assertion that `default_window_icon()` returns a value the builder accepts. We therefore:
  - Add a `#[cfg(test)] mod tray_setup_tests` (host-side `cargo test`) that asserts the bundle config's icon list is non-empty by parsing `tauri.conf.json` — guards against a future cleanup pass deleting `bundle.icon` and silently re-breaking the tray.
  - Document the manual macOS verification step in `Validation Commands`.
  - Optionally extend `tests/e2e/visual-regression.spec.js` only if it already covers a path that interacts with the tray; review confirms it does not, so do not add visual coverage here (visual regression's 14-baseline contract is for the main window only — adding a tray baseline would inflate the cap).

Concrete code change in `src-tauri/src/lib.rs:1003`:

```rust
// macOS: TrayIconBuilder does NOT auto-load default_window_icon. Without
// an explicit icon AND with no title (the title is set later by
// update_tray_icon, only on mode/running changes), the NSStatusItem
// renders at zero width and is invisible. See tauri-apps/tauri#11931.
let mut tray_builder = TrayIconBuilder::with_id("main")
    .menu(&menu)
    .show_menu_on_left_click(true)
    .on_menu_event(move |_tray, event| match event.id.as_ref() {
        "show" => {
            let app_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                show_app_window(app_clone).await;
            });
        }
        "start_session" => emit_tray_and_show(&app_handle, "tray-start-session"),
        "pause"         => emit_tray_and_show(&app_handle, "tray-pause"),
        "skip"          => emit_tray_and_show(&app_handle, "tray-skip"),
        "cancel"        => emit_tray_and_show(&app_handle, "tray-cancel"),
        "quit"          => { app_handle.exit(0); }
        _ => {}
    })
    .on_tray_icon_event(move |_tray, event| {
        if let TrayIconEvent::Click { .. } = event {
            let app_clone = app_handle_for_click.clone();
            tauri::async_runtime::spawn(async move {
                show_app_window(app_clone).await;
            });
        }
    });

if let Some(icon) = app.default_window_icon() {
    tray_builder = tray_builder.icon(icon.clone());
}

let _tray = tray_builder.build(app)?;
```

Notes on the diff:
- The `match`-arm reshape into single-line `emit_tray_and_show` calls is cosmetic and only there to keep the function under the existing clippy `cognitive_complexity` allowance; if it triggers a baseline-cap or clippy warning, keep the existing four-line arms verbatim and only add the `.icon(...)` plumbing.
- The `if let Some(icon)` guard avoids `.unwrap()` so the build does not panic in the theoretical case where `bundle.icon` is empty.
- `Image: Clone` in Tauri 2.x — `.clone()` on `&Image<'_>` is correct.

### 3. Add a regression-prevention unit test
- In `src-tauri/src/lib.rs`'s existing `#[cfg(test)] mod tests`, add:

```rust
#[test]
fn bundle_config_has_at_least_one_icon() {
    // Guard against a future cleanup pass deleting bundle.icon, which
    // would make Manager::default_window_icon() return None and silently
    // re-break the macOS tray (see issue #40).
    let conf = include_str!("../tauri.conf.json");
    let parsed: serde_json::Value = serde_json::from_str(conf).expect("valid tauri.conf.json");
    let icons = parsed["bundle"]["icon"].as_array().expect("bundle.icon array");
    assert!(!icons.is_empty(), "bundle.icon must be non-empty to populate default_window_icon");
}
```

This is a host-side test runnable under `cargo test -p presto --lib`; it does not need an AppKit run loop.

### 4. Local lint / format pass
- Run `cargo fmt --all` from the workspace root.
- Run `cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic` per the project lint posture.

### 5. Build verification
- `cargo build --frozen -p presto` (host-side) succeeds.
- `cd src && trunk build` (WASM frontend) succeeds — the fix does not touch the frontend, this is a sanity build.

### 6. Manual macOS verification (cannot be automated in CI)
- `cargo tauri dev` on a macOS host: verify the tray icon appears in the menu bar within ~1 s of the main window opening, **without** starting a session.
- Right-click the tray entry → the menu (`Show Presto`, `Start Session`, `Pause`, `Skip Session`, `Cancel`, `Quit`) appears.
- Start a focus session → the icon title updates to "🧠 25:00" (still visible).
- Pause → title updates, tooltip switches to `(Paused)`.
- Quit and relaunch → tray icon is present immediately on the second cold boot.
- Toggle `Settings → Hide icon on close`, close the window → dock icon disappears (Accessory activation policy), tray icon remains visible. Re-open via tray click → dock icon returns.
- Repeat on a `cargo tauri build` `.app` bundle (not just `dev`) — issue #40 specifically calls out production builds.

## Validation Commands
Execute every command to validate the bug is fixed with zero regressions.

- `cargo fmt --all --check` — no formatting drift.
- `cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic` — no new lint findings, particularly no `clippy::unnecessary_wraps` or `clippy::redundant_clone` triggered by the new `.icon(icon.clone())`.
- `cargo test --workspace --frozen` — the new `bundle_config_has_at_least_one_icon` test passes; no existing host-side tests regress.
- `(cd src && wasm-pack test --node)` — frontend bridge tests stay green (this fix doesn't touch `src/src/bridge/`, so they must remain green; if any go red, that's a regression).
- `(cd src && trunk build)` — WASM bundle still builds.
- `cargo build --frozen -p presto` — Tauri backend still builds.
- `bash scripts/check-mock-drift.sh` — tauriMock.js still mirrors the handler set (no command added/removed by this fix, so this must pass unchanged).
- `(cd tests/e2e && npx playwright test)` — full e2e suite green; the visual-regression baselines (14 PNGs) are untouched (this fix does not change frontend rendering).
- **Manual on macOS** (required, see Step 6 above): tray icon visible on launch, menu functional, title updates with timer state, survives quit + relaunch, both in `cargo tauri dev` and `cargo tauri build`.

## Notes
- **Scope discipline**: do not refactor `update_tray_icon`, `update_tray_menu`, `show_app_window`, or the macOS activation-policy plumbing — they're correct; the only defect is the missing `.icon(...)`. Tempting adjacent cleanups (e.g. removing the `mode_changed || running_changed` gate in `timer.rs` so the tray title is set on first paint) are out of scope; the Rust-side fix alone resolves the bug at the root.
- **Why not auto-call `update_tray_icon` on Leptos mount?** That would also paper over the bug, but only on the frontend path. A future contributor creating a tray in a non-Leptos context (or before the WASM bundle finishes booting) would re-hit the same invisibility. Fixing the builder is the durable answer.
- **macOS signing nuance**: per issue [tauri-apps/tauri#13770](https://github.com/tauri-apps/tauri/issues/13770), some macOS 26 builds gate menu-bar items behind an "Allow in Menu Bar" prompt for *unsigned* dev binaries. If a verifier on a fresh macOS host still sees no icon after this fix, check System Settings → Notifications & Focus → Menu Bar items and grant Presto access. This is a macOS-side capability, not a code bug.
- **Constitution alignment**: the change is single-file, single-line of behavioural code (plus a one-line `if let` guard and a hosted unit test). No new feature flags, no new dependencies, no public-API changes. Matches the "minimal blast radius" bias of `.specify/memory/constitution.md`.
- **No `--no-verify`** needed; the lockfile-drift pre-commit hook is unaffected (no `Cargo.toml`/`Cargo.lock` change).

---
*Generated by Agentex*
