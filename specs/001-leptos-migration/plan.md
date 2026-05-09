# Implementation Plan: Leptos Frontend Migration

**Branch**: `001-leptos-migration` | **Date**: 2026-05-09 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification at `specs/001-leptos-migration/spec.md`

## Table of Contents

1. [Summary](#summary)
2. [Technical Context](#technical-context)
3. [Constitution Check](#constitution-check)
4. [Project Structure](#project-structure)
5. [Modules](#modules)
6. [Testing strategy and test-first markers](#testing-strategy-and-test-first-markers)
7. [CI gates](#ci-gates)
8. [Implementation phasing](#implementation-phasing) (Phases 0 → 7 + audit method + pre-release checklist)
9. [Post-design Constitution Check](#post-design-constitution-check)
10. [Complexity Tracking](#complexity-tracking)

## Summary

Hard-cutover migration of the presto frontend from vanilla JS + Vite + Vitest to **Leptos (CSR + WASM) + Trunk + wasm-bindgen-test**. The Tauri 2.x backend (`src-tauri/`) gets a typed `BridgeError` enum across every command (closing the `Result<T, String>` boundary FR-008 implies), and grows adapter commands for Aptabase, Supabase, and `xlsx` export, replacing JS shims that disappear with the WASM swap. Ten unused commands are deleted in the cutover commit (Principle VII). The 17 Playwright e2e specs and 14 chromium-linux PNG visual regression baselines under `tests/e2e/__screenshots__/visual-regression/` are the green-light gate; the cutover's expected baseline diff is 0% (Principle IV; tolerance is a CI-flake budget, not a drift allowance).

The repo becomes a Cargo workspace with the Leptos crate at `src/` (named `presto-web`), `src-tauri/` unchanged, and a new `tools/build-themes/` workspace member for theme code-gen. Build tool is **Trunk** (not cargo-leptos — presto is CSR-only single-window desktop, and SSR/server-fn machinery is overkill). Repo-root `package.json` is deleted; the only surviving npm scope is `tests/e2e/` with `@playwright/test` pinned. Testing is `cargo test` + `wasm-bindgen-test` + Playwright; no Vitest, no happy-dom. Detailed decisions in [research.md](./research.md); module ownership in §[Modules](#modules); the typed bridge surface in [contracts/tauri-bridge.md](./contracts/tauri-bridge.md); a contributor onboarding path in [quickstart.md](./quickstart.md).

## Technical Context

**Language/Version**: Rust 1.83+ (workspace `edition = "2021"`); WASM target `wasm32-unknown-unknown` for the Leptos crate; Rust on the Tauri side is unchanged.
**Primary Dependencies**: `leptos = "0.7"` (CSR feature), `wasm-bindgen`, `web-sys`, `js-sys`, `serde` + `serde-wasm-bindgen` (for `invoke()` payloads), `gloo-storage` (localStorage wrapper). Backend deps unchanged. Build deps (Leptos crate): Trunk (workstation tool, not a Cargo dep). Replacements: `rust_xlsxwriter` (write-only `.xlsx`) replaces JS `xlsx`; theme code-gen via a workspace binary `tools/build-themes/` invoked by Trunk pre-build hook replaces `build-themes.js`.
**Storage**: Tauri app-data directory (authoritative; unchanged). `localStorage` for the bounded `presto-guest-mode` / `presto-auth-seen` flags only. No format change to on-disk state per FR-005.
**Testing**: `cargo test --workspace --frozen` for pure-logic Rust; `wasm-bindgen-test` for DOM-coupled Leptos modules; Playwright e2e (17 specs) + visual regression suite (14 baselines, 2% tolerance) unchanged.
**Target Platform**: macOS, Linux, Windows desktops. Single-window app, CSR only. No mobile, no server.
**Project Type**: Desktop app (Tauri host + Leptos WebView frontend). Cargo workspace with two members.
**Performance Goals**: Time-to-first-paint and timer-tick smoothness must not regress observably from the JS baseline. The visual regression suite is the proxy gate — anything visibly slower would cause a screenshot mismatch (transition animations have `animations: "disabled"`, but layout-shift symptoms would still surface). No new perf budget; preserve current.
**Constraints**: Visual regression suite must pass against the existing 14 baselines without regenerating them; ≤2 baselines may be re-captured for legitimate sub-pixel rendering drift with explicit per-baseline justification (>2 = escalate). All Tauri command shapes preserved exactly. Local data format unchanged. Auto-updater path from any released `0.4.x` to the post-cutover build must succeed.
**Scale/Scope**: Single-user desktop app. Estimated Leptos LOC after migration: ~6–9k (matching the ~10.7k JS LOC in `src/main.js` + managers + core, minus glue code that disappears under typed bridge wrappers). **Tauri command count: 36 today; the cutover deletes 10 unused commands (Principle VII; enumerated in [contracts/tauri-bridge.md](./contracts/tauri-bridge.md) §Deletions) and adds 6 permanent new (`track_event`, four `supabase_*`, `export_sessions_xlsx`) plus 7 transition-only `import_legacy_*` commands (sunset one minor version after cutover). Cutover-commit handler-set total: 26 surviving + 6 permanent + 7 transition = 39. Phase-6 cleanup removes `write_excel_file` (now redundant with `export_sessions_xlsx`), settling the cutover total at 38; one minor later, the 7 transition commands are removed for a steady-state of 31.** ~14 UI screens; ~7 manager state machines.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked at end after Phase 1 design — see §[Post-design Constitution Check](#post-design-constitution-check).*

Per-principle verdicts. Cited by Roman numeral + name.

- **I. The Timer Is Sacred** — **PASS**. The post-migration timer engine (`src/src/engine/`) is a pure Rust state machine with no `web-sys` imports; DOM-sourced activity signals enter via a normalised `ActivitySignal` stream defined in the bridge layer. Drift compensation, smart-pause activity gate, max-session cap, and manual-session-entry path all flow through the same engine. Behavioural-parity tests precede implementation (Principle V).
- **II. Local-First, Privacy-Default** — **PASS**. No new network egress. Aptabase opt-in is checked Rust-side at the call site (`are_analytics_enabled` already exists in `src-tauri/src/lib.rs:141`); the new `track_event` command preserves that gate. Supabase moves from a JS SDK to a Rust REST adapter; it remains opt-in (guest mode unaffected). PII discipline preserved. `localStorage` flags `presto-guest-mode` and `presto-auth-seen` survive the migration via `gloo-storage`.
- **III. Type Safety Over Defensive Code** — **PASS**. Leptos crate configured with `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` matching backend posture. Closed domains (timer mode, session type, sound notification variant, manager-state enums) are Rust sum types per FR-013. The Tauri command error channel is the typed `BridgeError` enum on every command (see [research.md](./research.md) §13 and [data-model.md](./data-model.md) §`BridgeError`); FR-008's compile-time-mismatch promise is load-bearing. `#[allow(...)]` requires inline justification. Validation lives at the bridge boundary (deserialising `invoke()` returns) only.
- **IV. Visual Regression Is The UI Contract** — **PASS**. The 14 baselines under `tests/e2e/__screenshots__/visual-regression/` are unchanged in this feature. The 2% tolerance in `playwright.config.js` is a CI-flake budget for cross-platform AA noise, not a drift allowance — the cutover's expected baseline diff is 0%. Re-captures during the cutover PR are 0 by default; up to 2 with explicit per-PR justification; >2 escalates to a constitution-amendment-or-spec-revision discussion (see [research.md](./research.md) §11). Re-capturing all 14 is forbidden. A mechanical CI gate counts changed baseline PNGs and fails the build at >2 (see §[CI gates](#ci-gates) and §[Implementation phasing](#implementation-phasing) Phase 7).
- **V. Test-First For Stateful Engines** — **PASS**. Phases 1, 2, 3 of the implementation phasing each follow RED → GREEN → REFACTOR commit ordering. Mapping in §[Testing strategy](#testing-strategy-and-test-first-markers). UI rendering, view wiring, and theme loading are exempt — covered by e2e + visual regression per AGENTS.md §Test-first commit ordering and Principle V.
- **VI. The Tauri Boundary Is Stable** — **PASS**. IPC stays `invoke()` for commands and `listen()` for events (FR-011). Three new commands added (`track_event`, `supabase_*` adapter family, `export_sessions_xlsx`) — each is documented in [contracts/tauri-bridge.md](./contracts/tauri-bridge.md), mirrored in `tauriMock.js` first, then test-first. No new IPC mechanisms. The "frontend gracefully short-circuits when `window.__TAURI_INTERNALS__` is unavailable" rule from AGENTS.md is enforced via a `BridgeAvailable` guard signal in `bridge::availability` (every `invoke()` wrapper short-circuits to a sentinel response when absent).
- **VII. No Upstream Compatibility Burden** — **N/A**. This feature does not touch the upstream-fork question. Existing user updater compatibility IS covered (FR-007).
- **VIII. Spec-Driven Feature Flow** — **PASS**. spec.md exists at `specs/001-leptos-migration/spec.md`; this plan references the principles it brushes by name; Constitution Check passes; tasks generation is gated on this plan.
- **IX. Lock Files Are First-Class** — **PASS** with **transition-state note**. Post-cutover the repo-root `package-lock.json` is removed; `Cargo.lock` becomes the single repo-root lock file. The surviving `tests/e2e/package-lock.json` is committed and authoritative for the e2e scope. CI uses `cargo build --workspace --frozen` and `(cd tests/e2e && npm ci)` — never `cargo build` or `npm install`. The pre-commit hook drift check is updated to scan both lockfiles. This transition is a deletion, not a violation; documented in [research.md](./research.md) §4 and Phase 6 of phasing.

No principle is **VIOLATION**. Proceed to Phase 0.

Principles **II (Local-First)** and **VII (No Upstream Compatibility Burden)** flagged informationally per the template note. II is materially relevant (analytics + auth boundaries change); VII is not.

## Project Structure

### Documentation (this feature)

```text
specs/001-leptos-migration/
├── plan.md              # This file
├── research.md          # Phase 0 — 12 PM decisions w/ rationale, Trunk-vs-cargo-leptos, workspace conversion
├── data-model.md        # Phase 1 — bridge sum types & shared records (TimerMode, Session, AuthState, …)
├── quickstart.md        # Phase 1 — contributor onboarding (rustup → trunk → cargo build → tauri dev)
├── contracts/
│   └── tauri-bridge.md  # Phase 1 — every Tauri command's typed surface (post-migration enforcement story)
├── checklists/          # Authored at /speckit-specify
└── tasks.md             # Phase 2 — generated by /speckit-tasks (NOT this command)
```

### Source Code (repository root, post-migration)

```text
Cargo.toml                          # NEW: workspace root with members = ["src", "src-tauri", "tools/build-themes"]
Cargo.lock                          # SINGLE repo-root lock file
src/                                # NEW: Leptos crate root (`presto-web`); repurposed; old JS deleted in Phase 6
├── Cargo.toml                      # NEW: [package].name = "presto-web"; clippy::pedantic = deny
├── Trunk.toml                      # NEW: Trunk config; dist-dir at default (dist/ ⇒ src/dist/)
├── index.html                      # EXISTS: Trunk's entry HTML (ported, <script> tags swapped for Trunk hooks)
├── style/                          # CSS source (relocated from current src/styles/, theme files unchanged)
│   └── themes/                     # Theme CSS files; source of truth per FR-021/FR-022
├── assets/                         # Static (icon font from remixicon vendored, fork-era brand assets)
└── src/                            # NEW: Rust source for the Leptos crate
    ├── main.rs                     # WASM entry; mounts <App/>
    ├── app.rs                      # Root component, router (single-window), global signals
    ├── bridge/                     # Tauri command + event wrappers (typed surface)
    │   ├── mod.rs
    │   ├── availability.rs         # window.__TAURI_INTERNALS__ presence check; degraded-mode signal
    │   ├── commands.rs             # invoke() wrappers — one fn per command in contracts/tauri-bridge.md
    │   └── events.rs               # listen() wrappers — typed event payloads
    ├── engine/                     # Pomodoro state machine successor
    │   ├── mod.rs
    │   ├── timer.rs                # Pure state machine; no web-sys
    │   ├── activity_signal.rs      # ActivitySignal enum + folding logic for smart-pause
    │   └── tests.rs                # #[cfg(test)] behaviour tests (drift, smart-pause, max-session, manual entry)
    ├── managers/                   # Manager state machines (one module per manager)
    │   ├── mod.rs
    │   ├── auth.rs                 # AuthState sum type, sign-in/sign-out/get-session via bridge
    │   ├── session.rs              # Manual-session CRUD; calls bridge::commands
    │   ├── settings.rs             # Settings load/save, validation at boundary
    │   ├── navigation.rs           # NavView enum, view routing
    │   ├── tag.rs                  # Tag CRUD
    │   ├── team.rs                 # Team feature (parity with current JS team-manager)
    │   └── update.rs               # Update polling + UpdateInfo signal
    ├── components/                 # UI components (Leptos)
    │   ├── mod.rs
    │   ├── timer_view.rs
    │   ├── task_list.rs
    │   ├── history.rs
    │   ├── calendar.rs
    │   ├── settings/
    │   │   ├── mod.rs
    │   │   ├── general.rs
    │   │   ├── shortcuts.rs
    │   │   ├── notifications.rs
    │   │   ├── automation.rs
    │   │   ├── advanced.rs
    │   │   ├── goals.rs
    │   │   ├── theme.rs
    │   │   └── updates.rs
    │   ├── auth_modal.rs
    │   ├── update_notification.rs
    │   └── tag_manager.rs
    ├── theme/
    │   ├── mod.rs
    │   ├── themes.rs               # GENERATED by tools/build-themes (DO NOT EDIT)
    │   └── loader.rs               # Theme apply, persistence, follow-system-theme glue
    └── tests/                      # #[wasm_bindgen_test] integration cases for DOM-coupled modules

src-tauri/                          # UNCHANGED scope. Three new commands added (see contracts/).
├── Cargo.toml                      # Workspace member; existing posture unchanged
└── src/{lib.rs,helpers.rs,main.rs} # +track_event, +supabase_* family, +export_sessions_xlsx

tools/                              # NEW: workspace member for build tooling
└── build-themes/
    ├── Cargo.toml                  # [package].name = "presto-build-themes"
    └── src/main.rs                 # Reads src/style/themes/*.css → emits src/src/theme/themes.rs

tests/                              # UNCHANGED layout; e2e + baselines preserved
├── e2e/
│   ├── package.json                # SCOPED: pins @playwright/test only
│   ├── package-lock.json           # SCOPED authoritative lockfile (Principle IX scope-survives)
│   ├── *.spec.js                   # 17 specs unchanged
│   ├── fixtures/tauriMock.js       # MIRRORS bridge surface; updated per FR-010 ordering rule
│   └── __screenshots__/visual-regression/   # 14 PNGs UNCHANGED (cutover gate)
└── (deleted in Phase 6) core/, managers/, utils/   # Vitest specs replaced by Rust tests

# DELETED in Phase 6 (cutover commit):
# package.json, package-lock.json, node_modules/, vite.config.js, vitest.config.js,
# eslint.config.js, tsconfig.json, src/globals.d.ts, src/main.js, src/managers/*.js,
# src/core/*.js, src/utils/*.js, build-themes.js
```

**Structure Decision**: **Cargo workspace, two crates + one tooling crate**, with `src/` repurposed as the Leptos crate root and `src-tauri/` unchanged. Rationale in [research.md](./research.md) §1 and §3. The `src/src/` nesting is the standard Tauri+Leptos convention; awkward but expected. Lowest churn for `tauri.conf.json` `frontendDist` paths and `playwright.config.js` baseURL — both already point at `src/` / `127.0.0.1:1420`.

## Modules

The Leptos crate's intended module breakdown, with explicit "owns" / "does NOT own" boundaries. Forces clean separation; informs Phase 2 task decomposition.

### `app.rs`

**Owns**:
- Root `<App/>` component mounted by `main.rs`.
- Global signals (current `AuthState`, `BridgeAvailable`, current `NavView`).
- Top-level routing decision (single-window app — likely a `Match`/`Show` pattern over `NavView`, not a URL router).

**Does NOT own**:
- Any business state (delegated to `managers/*`).
- Any timer logic (delegated to `engine/`).
- Any direct `invoke()` call (delegated to `bridge/`).

### `engine/`

**Owns**:
- The pomodoro state machine (`engine::timer::Timer`): mode transitions, drift compensation, smart-pause gate, max-session cap.
- The `ActivitySignal` reduction (Idle ↔ Active edge detection from raw DOM events folded by `bridge/events`).
- Behaviour-level `#[cfg(test)]` tests covering every transition rule from `src/core/pomodoro-timer.js`.

**Does NOT own**:
- Any DOM read or `web-sys` import (Principle I — engine is a pure state machine).
- Persistence (delegated to `managers/session` + `bridge/commands`).
- UI rendering (delegated to `components/timer_view`).

### `bridge/`

**Owns**:
- A typed function per Tauri command (one fn per row in [contracts/tauri-bridge.md](./contracts/tauri-bridge.md)).
- Typed `listen()` wrappers for each event channel (updater, OAuth callback, tray, global-shortcut, user-activity, user-inactivity, shortcuts-updated).
- The `BridgeAvailable` signal — a one-time check of `window.__TAURI_INTERNALS__` exposed as a derived signal that every `invoke()` wrapper consults to short-circuit gracefully when absent.

**Does NOT own**:
- Business logic (delegated to managers and engine).
- UI presentation of degraded state (delegated to components).
- Mock implementations (those live in `tests/e2e/fixtures/tauriMock.js` for e2e; for non-Tauri dev runs the bridge wrappers degrade per AGENTS.md §Bridge availability).

### `managers/auth.rs`

**Owns**:
- `AuthState` enum (`Guest | Unauthenticated | SignedIn { user_id, email, … }`).
- Sign-in / sign-out / get-session calls — via the new Supabase Tauri-side adapter (see [research.md](./research.md) §6).
- The `presto-guest-mode` and `presto-auth-seen` localStorage flags (read at startup, written on sign-out / continue-as-guest).
- The one-shot localStorage Supabase-token migration (see [research.md](./research.md) §6, "One-shot Supabase token migration"). Entry point invoked from `bridge::storage::migrate_legacy_localstorage()` on first post-cutover launch.

**Does NOT own**:
- Direct Supabase HTTP — that's in `src-tauri/`.
- Auth modal rendering (`components/auth_modal`).

### `managers/session.rs`

**Owns**:
- Manual-session CRUD signals: load, create, update, delete, list-by-date.
- Reduction of `PomodoroSession` updates (the live timer feeds completed sessions into this manager).

**Does NOT own**:
- The timer state machine (that's `engine/`).
- Calendar UI (`components/calendar`).
- Excel export logic — only triggers `bridge::commands::export_sessions_xlsx`.

### `managers/settings.rs`

**Owns**:
- `Settings` record + signals.
- Load/save via bridge.
- Missing-field migration (FR-005): if the loaded JSON shape is missing fields, fill in `Default` values and write back; idempotent.
- Legacy `hide_status_bar` → `status_bar_display` migration (see [data-model.md](./data-model.md) §"Settings legacy migration"). The custom serde deserializer reads the legacy bool field once and resolves to `StatusBarDisplay::Default` or `StatusBarDisplay::IconOnly`; the next save writes the new shape and drops the legacy field.

**Does NOT own**:
- The settings UI (`components/settings/*`).
- Theme application (`theme/loader`).

### `managers/navigation.rs`

**Owns**:
- `NavView` enum (Timer | Tasks | History | Calendar | Settings(Tab) | Tags | Team).
- View transitions and the active-view signal.

**Does NOT own**:
- View rendering (each view is in `components/`).
- Persisting last-active view across sessions (that's `managers/settings`).

### `managers/tag.rs`

**Owns**:
- `Tag` CRUD signals; tag-list reduction.

**Does NOT own**:
- Tag UI (`components/tag_manager`).
- Session-tag association (that's `managers/session`).

### `managers/team.rs`

**Owns**:
- Parity with current `team-manager.js` (mostly demo-fixture-driven; not a major feature).

**Does NOT own**:
- Backend team functionality (no backend exists; pure-frontend demo today).

### `managers/update.rs`

**Owns**:
- `UpdateInfo` enum (`NoUpdate | Available { version, notes }`).
- Polling cadence (replicates current behaviour).
- Reaction to `tauri-plugin-updater` events.

**Does NOT own**:
- Update notification UI (`components/update_notification`).
- The actual install/restart flow (delegated to the existing Tauri plugin via bridge).

### `components/`

**Owns**:
- `view!` macros / Leptos components for each screen and reusable widget.
- DOM event wiring (`on:click`, `on:input`, etc.) that feeds into managers / engine.

**Does NOT own**:
- Business state (consumed via signals, not held).
- Direct `invoke()` calls (always go through `bridge::commands`).

### `theme/`

**Owns**:
- `themes.rs` — generated by `tools/build-themes/` from `src/style/themes/*.css` (DO NOT EDIT manually).
- Theme application: setting a CSS variable / class on `document.documentElement` per the active theme.
- Follow-system-theme detection via `prefers-color-scheme` media query.

**Does NOT own**:
- The CSS source (under `src/style/themes/`; that's the source-of-truth contract per FR-021).
- Theme picker UI (`components/settings/theme`).

## Testing strategy and test-first markers

Per Principle V, failing-test commits precede implementation commits for stateful engines, manager state machines, persistence helpers, and time-keeping math. Per AGENTS.md §Test-first commit ordering, the diff must show RED first, then GREEN.

| Module | Test runner | Test-first? | Notes |
|---|---|---|---|
| `engine/timer.rs` | `cargo test` | **YES (RED-first)** | Behaviour tests for every transition rule from `src/core/pomodoro-timer.js`. Drift, smart-pause, max-session, manual entry. |
| `engine/activity_signal.rs` | `cargo test` | **YES (RED-first)** | Idle ↔ Active edge detection; window-folded reductions. |
| `engine/date_format.rs` | `cargo test` | **YES (RED-first)** | Named test `engine::date_format::tests::matches_js_to_date_string` iterates 366 dates and asserts `chrono_format(date) == known_js_format(date)` for a representative sample, pinning the `"%a %b %d %Y"` format so a future chrono change that breaks parity fails loud. |
| `bridge/commands.rs` | `wasm-bindgen-test` (mock-injected) | **YES (RED-first)** | One test per command verifying serde round-trip of args/returns + error variants. New commands extend `tauriMock.js` first per FR-010. |
| `bridge/error.rs` | `cargo test` + `wasm-bindgen-test` | **YES (RED-first)** | `bridge::error::tests::*` exercises every `BridgeError` variant's serde round-trip (Tauri-side serialise → Leptos-side deserialise; and the inverse for the Leptos-only `BridgeUnavailable` variant). |
| `bridge/availability.rs` | `wasm-bindgen-test` | **YES (RED-first)** | `window.__TAURI_INTERNALS__` presence detection; short-circuit return shapes. |
| `bridge/events.rs` | `wasm-bindgen-test` | **YES (RED-first)** | Typed payload deserialisation per event. |
| `bridge/storage.rs` (legacy localStorage migration) | `wasm-bindgen-test` | **YES (RED-first)** | One test per `import_legacy_*` command exercises the migration with mocked `localStorage` and asserts the matching Tauri command receives the expected payload (idempotent — second run is a no-op). |
| `managers/auth.rs` | `cargo test` (pure) + `wasm-bindgen-test` (DOM) | **YES (RED-first)** | State machine transitions: Unauth → SignIn → Authed; Authed → SignOut → Guest. The named test `managers/auth.rs::tests::imports_legacy_supabase_session_from_localstorage` covers the one-shot Supabase-token migration. |
| `managers/session.rs` | `cargo test` | **YES (RED-first)** | CRUD reductions; date-grouping invariants. |
| `managers/settings.rs` | `cargo test` | **YES (RED-first)** | Load/save round-trip; missing serde-default fields (mirrors `app_settings_missing_serde_default_fields_use_defaults` in `src-tauri/src/lib.rs:1241`); the named test `managers/settings::tests::migrates_hide_status_bar_to_status_bar_display` covers the legacy `hide_status_bar` → `StatusBarDisplay` migration. |
| `managers/navigation.rs` | `cargo test` | **YES (RED-first)** | View transition rules. |
| `managers/tag.rs` | `cargo test` | **YES (RED-first)** | CRUD reductions. |
| `managers/team.rs` | `cargo test` | YES (RED-first; small surface) | Parity-only; minimal logic. |
| `managers/update.rs` | `cargo test` + `wasm-bindgen-test` | **YES (RED-first)** | Update-info reductions; polling cadence rules. |
| `theme/loader.rs` | `wasm-bindgen-test` | NO — covered by visual regression | Per Principle V "test-first does NOT apply to ... theme loading". |
| `components/*` | Playwright e2e + visual regression | NO — covered by e2e | Per Principle V "test-first does NOT apply to UI rendering". |
| `app.rs` | Playwright e2e | NO — covered by e2e | Wiring only; no business logic to test in isolation. |
| `tools/build-themes/` | `cargo test` (input → output snapshot) | YES (RED-first) | Tooling correctness matters; output is committed-but-generated `themes.rs`. |

**Visual regression** is the integration acceptance test for UI; not test-first. The 14 baselines are pre-existing; the migration's job is to not break them.

**`tauriMock.js`-first ordering rule** (per FR-010 and Principle VI): adding any Tauri command means (1) extend `tests/e2e/fixtures/tauriMock.js` first with a default return for the new command, (2) add a failing `wasm-bindgen-test` (or e2e test) exercising the bridge wrapper, (3) land the real Rust call site. This applies to the three new commands introduced by this migration (`track_event`, `supabase_*` adapter family, `export_sessions_xlsx`) and to every future addition.

## CI gates

Reference `.agentex.yml`. Post-cutover stage definitions:

```yaml
qa:
  setup:
    - cargo fetch --locked
    - (cd tests/e2e && npm ci)
    - (cd tests/e2e && npx playwright install --with-deps chromium)

  test:
    - cargo test --workspace --frozen
    - (cd src && wasm-pack test --headless --chrome)   # or trunk's test runner; chosen by Phase 1

  lint:
    - cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic
    - cargo fmt --check
    # Enforce Principle I: engine is a pure state machine, no DOM access.
    - 'if grep -rE "web_sys|web-sys" src/src/engine/ ; then echo "ERROR: engine module references web-sys (Principle I)"; exit 1; fi'

  e2e:
    - (cd src && trunk build --release)
    - (cd tests/e2e && npx playwright test --grep-invert "@visual")

  visual:
    - (cd tests/e2e && npx playwright test visual-regression.spec.js)

  baseline-cap:
    # Mechanical CI gate: at most 2 visual-regression baseline PNGs may change in a single PR.
    - |
      count=$(git diff --name-only origin/main...HEAD | grep -c '^tests/e2e/__screenshots__/visual-regression/.*\.png$' || true)
      if [ "$count" -gt 2 ]; then
        echo "ERROR: $count baseline PNGs changed (max 2 per PR; see plan.md §Constitution Check IV)"
        exit 1
      fi
```

**Pre-commit hook** (husky-equivalent) post-cutover:
- `cargo fmt --check` on touched Rust files (workspace-wide if any Rust changed).
- `cargo clippy --workspace --frozen -- -D warnings` on PR (full pedantic on CI).
- Lockfile-drift check covers both `Cargo.lock` (vs. `Cargo.toml` workspace members) and `tests/e2e/package-lock.json` (vs. `tests/e2e/package.json`).
- `--no-verify` not used (Principle III).

Stage transitions in CI run sequentially; a failing earlier stage short-circuits later ones, except `visual` which always runs when `e2e` passes (and is the documented green-light gate per Principle IV). The `baseline-cap` stage runs in parallel with `visual` and fails the build at >2 changed baselines.

## Implementation phasing

Internal phasing for the implementation phase. The actual task breakdown is `/speckit-tasks`'s job — this section names the phases, their entry/exit criteria, and the test-first commit ordering they require.

### Audit method for test-first ordering

The cutover PR has ~250 commits with strict RED/GREEN ordering per Principle V. Reviewers don't audit every commit; the audit method is: (a) check that for every Principle-V-scope file, the commit history shows at least one commit pair where the test file has *failing* assertions before the implementation file is added, (b) sample 10% of commits per phase by running `git checkout <sha> && cargo test` to confirm the RED commit's tests fail and the GREEN commit's tests pass, and (c) confirm the diff sequence in `git log --reverse --pretty=format:'%h %s'` shows test-files-touched before impl-files-touched per scope. The cutover PR's description includes a one-line per-phase summary with the sample-able SHAs.

### Phase 0 — Workspace scaffold

**Entry**: clean branch `001-leptos-migration` post-spec.
**Exit**: workspace `Cargo.toml` with members `src`, `src-tauri`, `tools/build-themes`; empty Leptos crate (`presto-web`) that compiles to WASM via `trunk build`; Trunk hello-world page that Tauri can serve at `127.0.0.1:1420`. `tauri.conf.json` updated: `frontendDist = "../src/dist"`, `beforeDevCommand = "cd src && trunk serve --port 1420"`, `beforeBuildCommand = "cd src && trunk build --release"`, `devUrl = "http://127.0.0.1:1420"` per [research.md](./research.md) §1. Existing JS app still runs alongside (deletion is Phase 6).
**Test-first**: N/A (scaffolding).

### Phase 0.5 — Mock/handler reconciliation

**Entry**: Phase 0 complete.
**Exit**: `tests/e2e/fixtures/tauriMock.js` reconciled to today's surviving handler set in `src-tauri/src/lib.rs`:
- **Add** 8 mock entries for handler-registered commands that lack mocks today: `get_stats_history`, `reset_all_data`, `save_daily_stats`, `start_activity_monitoring`, `stop_activity_monitoring`, `update_activity_timeout`, `update_tray_icon`, `update_tray_menu`.
- **Remove** 4 stale mock-only entries that have no corresponding handler: `append_daily_stats`, `delete_all_data`, `load_history`, `open_url`.
- **Skip** the 10 commands deleted in Phase 6 (see [contracts/tauri-bridge.md](./contracts/tauri-bridge.md) §Deletions); they don't need mocks because the handlers go away.

**Test-first**: N/A (the mock is itself the test fixture; correctness is verified by the e2e suite continuing to pass).

This phase precedes Phase 1 so the new commands land on a clean, accurate mock baseline; the mock-first rule (FR-010, Principle VI) becomes meaningful only when the mock matches today's handler set.

### Phase 1 — Bridge module (test-first)

**Entry**: Phase 0.5 complete.
**Exit**: every surviving Tauri command in [contracts/tauri-bridge.md](./contracts/tauri-bridge.md) has a typed Leptos wrapper in `src/src/bridge/commands.rs`, with a passing test (RED commit precedes GREEN). The `BridgeError` enum is introduced both in `src-tauri/src/lib.rs` and in the Leptos crate; every surviving handler's `Result<T, String>` is mechanically rewritten to `Result<T, BridgeError>` with the mapping strategy from [research.md](./research.md) §13 (default to `BridgeError::Internal { msg: e.to_string() }`; tighten to `NotFound`/`InvalidArgument`/`NotAuthenticated` where the call site has semantic context). `update_tray_menu` and `update_tray_icon` tighten their stringly-typed enum-shaped args to `TimerMode`. `bridge/availability.rs`, `bridge/events.rs`, and `bridge/error.rs` complete. New permanent commands (`track_event`, four `supabase_*`, `export_sessions_xlsx`) and transition-only commands (`import_legacy_supabase_session`, six per-domain `import_legacy_*`) are added behind the mock-first rule (mock entry → RED test → GREEN impl).
**Test-first**: YES, per Principle V. Each command: failing-test commit → implementation commit. Each `BridgeError` variant: `bridge::error::tests::*` exercises the serde round-trip RED-first.

### Phase 2 — Engine port (test-first)

**Entry**: Phase 1 complete.
**Exit**: `engine/timer.rs` passes every behaviour test that existed for `src/core/pomodoro-timer.js`. RED-GREEN-REFACTOR commit per behaviour. `ActivitySignal` reduction complete and tested. `engine/date_format.rs` named test (`engine::date_format::tests::matches_js_to_date_string`) lands and passes — pinning the chrono format string `"%a %b %d %Y"` against `Date.toDateString()` parity (see [data-model.md](./data-model.md) `Session.date`).
**Test-first**: YES, RED-first per behaviour. SC-007 explicit.

### Phase 3 — Managers (test-first, one at a time)

**Entry**: Phase 2 complete.
**Exit**: each manager (`auth`, `session`, `settings`, `navigation`, `tag`, `team`, `update`) has its state machine in Rust with passing RED-first tests. Order recommended: `settings` → `navigation` → `tag` → `session` → `auth` → `update` → `team` (matches dependency order; `auth` later because it requires the new Supabase Tauri-side adapter to land first).

**Per-manager test-first highlights**:
- `managers/settings`: named test `migrates_hide_status_bar_to_status_bar_display` (legacy `hide_status_bar: bool` → `StatusBarDisplay::IconOnly | Default` per [data-model.md](./data-model.md) §"Settings legacy migration").
- `managers/auth`: named test `imports_legacy_supabase_session_from_localstorage` (one-shot Supabase-token migration via mocked `localStorage`; asserts the Tauri-side adapter receives the expected payload).
- One wasm-bindgen-test per `import_legacy_*` command in `bridge/storage.rs` (see §[Testing strategy](#testing-strategy-and-test-first-markers)).

**Test-first**: YES per manager.

### Phase 4 — Components (UI port, screen by screen)

**Entry**: Phase 3 complete.
**Exit**: every screen in `components/*` rendered by Leptos; `tauri dev` shows a working app indistinguishable from the JS build to the visual regression suite. Order recommended: Timer → Tasks → History → Calendar → Tag manager → Settings tabs (8) → Auth modal → Update notification → Team.
**Test-first**: NO (covered by e2e + visual regression per Principle V).

### Phase 5 — Theme system + assets

**Entry**: Phase 4 complete.
**Exit**: `tools/build-themes/` generates `src/src/theme/themes.rs` from `src/style/themes/*.css`; Trunk `[[hooks]]` invokes it on every build; `theme/loader.rs` applies themes; `remixicon` font + CSS vendored in `src/assets/icons/` and served by Trunk; visual regression baselines pass.
**Test-first**: `tools/build-themes` YES (snapshot-style); `theme/loader` covered by e2e.

### Phase 6 — Cleanup (cutover commit)

**Entry**: Phase 5 complete; visual regression suite passes against all 14 baselines without regeneration; e2e suite passes; full CI green.

**Deleted JS toolchain**: from repo root — `package.json`, `package-lock.json`, `node_modules/`, `vite.config.js`, `vitest.config.js`, `eslint.config.js`, `tsconfig.json`, `src/globals.d.ts`, `build-themes.js`. From `src/` — `main.js`, `managers/*.js`, `core/*.js`, `utils/*.js`, `config/storage-keys.js`. From `tests/` — `core/`, `managers/`, `utils/` Vitest specs (their behaviour is preserved by Phase 1–3 Rust tests).

**Deleted Tauri commands** (10 unused; per [contracts/tauri-bridge.md](./contracts/tauri-bridge.md) §Deletions and Principle VII): from `src-tauri/src/lib.rs` — `save_manual_session`, `delete_manual_session`, `get_manual_sessions_for_date`, `save_tags`, `load_session_tags`, `save_session_tags`, `unregister_global_shortcuts`, `show_window`, `set_dock_visibility`, `set_status_bar_visibility`. From `src-tauri/src/helpers.rs` — any helpers left dead by these removals (a follow-up `cargo +nightly udeps`-style scan ratifies).

**Deprecated by replacement**: `write_excel_file` removed (superseded by `export_sessions_xlsx`).

**Survives**: `tests/e2e/package.json`, `tests/e2e/package-lock.json` (scoped per [research.md](./research.md) §4); the 7 transition-only `import_legacy_*` Tauri commands (slated for removal one minor version after cutover, in a separate cleanup release).

**Test-first**: N/A (deletion). Re-run visual regression suite as the final cutover gate.

### Phase 7 — CI hardening

**Entry**: Phase 6 complete.
**Exit**: the `baseline-cap` CI stage from §[CI gates](#ci-gates) is wired into `.github/workflows/ci.yml` (or `.agentex.yml` per repo convention) and is exercised on a throwaway branch by intentionally re-capturing 3 baselines; the gate fails the build with the documented error. The `engine` `web-sys` grep in the `lint` stage is verified to fail-closed by introducing a temporary `web_sys::` reference under `src/src/engine/` and confirming the gate fails.
**Test-first**: the CI gates are themselves verified by these dry-runs (commits are created on a throwaway branch, gates fail as expected, branch is deleted).

### Pre-release validation checklist

Owner: maintainer (Konstantin). CI matrix is out of scope for this feature. The checklist runs once per merge of feature 001 (and once per major-version release thereafter):

1. Build the prior release (e.g., `v0.4.4`) installer for the local platform via `npm run build` on `main` pre-merge.
2. Install the prior release on a clean test profile (or wipe app-data dir).
3. Sign in (Supabase) and create at least one session, one task, one tag, two manual session entries. Toggle a non-default theme. Toggle off `auto_check_updates`. Toggle on a non-default `status_bar_display`.
4. Build the post-cutover build from `001-leptos-migration` HEAD via `cd src && trunk build --release && cargo tauri build`.
5. Trigger the update on the running prior release (or sideload the new bundle through Tauri's updater channel).
6. Confirm the updated app launches and:
   - Auth state preserved (still signed in).
   - All sessions, tasks, tags, manual sessions intact (compare counts and IDs).
   - Settings preserved (theme, `auto_check_updates`, `status_bar_display`).
   - localStorage migration ran exactly once (re-launch verifies idempotency: no duplicate sessions, no errors).

## Post-design Constitution Check

Re-checked after Phase 1 design (research.md, data-model.md, contracts/tauri-bridge.md, quickstart.md). Verdicts unchanged from §[Constitution Check](#constitution-check). No principle moved from PASS to VIOLATION. New evidence per principle:

- **I**: data-model.md confirms `TimerMode` is a Rust enum and `engine/` has no `web-sys` dependency declared (CI-enforced — see §[CI gates](#ci-gates)).
- **III**: contracts/tauri-bridge.md uses the typed `BridgeError` enum for every command's error variant; no `String` error escape hatch survives the cutover. FR-008's compile-time-mismatch promise is now backed by the wire-level type.
- **V**: §[Testing strategy](#testing-strategy-and-test-first-markers) explicitly enumerates RED-first scope. The audit method for the cutover PR's ~250 commits is documented in §[Implementation phasing](#implementation-phasing) "Audit method for test-first ordering".
- **VI**: contracts/tauri-bridge.md is the documented bridge contract; the mock-mirroring rule from FR-010 is reaffirmed at the bottom of that file. Phase 0.5 reconciles the existing mock to today's handler set before any new command is added.

## Complexity Tracking

> No Constitution Check violations require justification.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none) | — | — |
