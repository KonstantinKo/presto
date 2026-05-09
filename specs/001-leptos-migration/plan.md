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
8. [Implementation phasing](#implementation-phasing)
9. [Post-design Constitution Check](#post-design-constitution-check)
10. [Complexity Tracking](#complexity-tracking)

## Summary

Hard-cutover migration of the presto frontend from vanilla JS + Vite + Vitest to **Leptos (CSR + WASM) + Trunk + wasm-bindgen-test**. The Tauri 2.x backend (`src-tauri/`) is unchanged in scope — only the bridge surface gets adapter commands for Aptabase, Supabase, and `xlsx` export, replacing JS shims that disappear with the WASM swap. The 17 Playwright e2e specs and 14 chromium-linux PNG visual regression baselines under `tests/e2e/__screenshots__/visual-regression/` are the green-light gate; pixel-equivalence within 2% (`playwright.config.js`) is the cutover acceptance test.

The repo becomes a Cargo workspace: `src/` is repurposed as the Leptos crate root (lowest churn for `tauri.conf.json` `frontendDist` and `playwright.config.js` baseURL), `src-tauri/` stays. Build tool is **Trunk** (not cargo-leptos — presto is CSR-only single-window desktop, and SSR/server-fn machinery is overkill). Repo-root `package.json` is deleted; the only surviving npm scope is `tests/e2e/` with `@playwright/test` pinned. Testing is `cargo test` + `wasm-bindgen-test` + Playwright; no Vitest, no happy-dom. Detailed decisions in [research.md](./research.md); module ownership in §[Modules](#modules); the typed bridge surface in [contracts/tauri-bridge.md](./contracts/tauri-bridge.md); a contributor onboarding path in [quickstart.md](./quickstart.md).

## Technical Context

**Language/Version**: Rust 1.83+ (workspace `edition = "2021"`); WASM target `wasm32-unknown-unknown` for the Leptos crate; Rust on the Tauri side is unchanged.
**Primary Dependencies**: `leptos = "0.7"` (CSR feature), `wasm-bindgen`, `web-sys`, `js-sys`, `serde` + `serde-wasm-bindgen` (for `invoke()` payloads), `gloo-storage` (localStorage wrapper). Backend deps unchanged. Build deps (Leptos crate): Trunk (workstation tool, not a Cargo dep). Replacements: `rust_xlsxwriter` (write-only `.xlsx`) replaces JS `xlsx`; theme code-gen via a workspace binary `tools/build-themes/` invoked by Trunk pre-build hook replaces `build-themes.js`.
**Storage**: Tauri app-data directory (authoritative; unchanged). `localStorage` for the bounded `presto-guest-mode` / `presto-auth-seen` flags only. No format change to on-disk state per FR-005.
**Testing**: `cargo test --workspace --frozen` for pure-logic Rust; `wasm-bindgen-test` for DOM-coupled Leptos modules; Playwright e2e (17 specs) + visual regression suite (14 baselines, 2% tolerance) unchanged.
**Target Platform**: macOS, Linux, Windows desktops. Single-window app, CSR only. No mobile, no server.
**Project Type**: Desktop app (Tauri host + Leptos WebView frontend). Cargo workspace with two members.
**Performance Goals**: Time-to-first-paint and timer-tick smoothness must not regress observably from the JS baseline. The visual regression suite is the proxy gate — anything visibly slower would cause a screenshot mismatch (transition animations have `animations: "disabled"`, but layout-shift symptoms would still surface). No new perf budget; preserve current.
**Constraints**: Visual regression suite must pass against the existing 14 baselines without regenerating them; ≤2 baselines may be re-captured for legitimate sub-pixel rendering drift with explicit per-baseline justification (>2 = escalate). All Tauri command shapes preserved exactly. Local data format unchanged. Auto-updater path from any released `0.4.x` to the post-cutover build must succeed.
**Scale/Scope**: Single-user desktop app. Estimated Leptos LOC after migration: ~6–9k (matching the ~10.7k JS LOC in `src/main.js` + managers + core, minus glue code that disappears under typed bridge wrappers). 30 Tauri commands enumerated in the contracts file. ~14 UI screens; ~7 manager state machines.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked at end after Phase 1 design — see §[Post-design Constitution Check](#post-design-constitution-check).*

Per-principle verdicts. Cited by Roman numeral + name.

- **I. The Timer Is Sacred** — **PASS**. The post-migration timer engine (`src/src/engine/`) is a pure Rust state machine with no `web-sys` imports; DOM-sourced activity signals enter via a normalised `ActivitySignal` stream defined in the bridge layer. Drift compensation, smart-pause activity gate, max-session cap, and manual-session-entry path all flow through the same engine. Behavioural-parity tests precede implementation (Principle V).
- **II. Local-First, Privacy-Default** — **PASS**. No new network egress. Aptabase opt-in is checked Rust-side at the call site (`are_analytics_enabled` already exists in `src-tauri/src/lib.rs:141`); the new `track_event` command preserves that gate. Supabase moves from a JS SDK to a Rust REST adapter; it remains opt-in (guest mode unaffected). PII discipline preserved. `localStorage` flags `presto-guest-mode` and `presto-auth-seen` survive the migration via `gloo-storage`.
- **III. Type Safety Over Defensive Code** — **PASS**. Leptos crate configured with `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` matching backend posture. Closed domains (timer mode, session type, sound notification variant, manager-state enums) are Rust sum types per FR-013. `#[allow(...)]` requires inline justification. Validation lives at the bridge boundary (deserialising `invoke()` returns) only.
- **IV. Visual Regression Is The UI Contract** — **PASS**. The 14 baselines under `tests/e2e/__screenshots__/visual-regression/` are unchanged in this feature. Tolerance stays at 2% per `playwright.config.js`. Up to 2 baselines may be re-captured for legitimate sub-pixel rendering drift with one-line PR justification per Edge Case 11; >2 escalates. Re-capturing all 14 is forbidden (PM decision §Decisions, item 11).
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
src/                                # NEW: Leptos crate root (repurposed; old JS deleted in Phase 6)
├── Cargo.toml                      # NEW: Leptos crate manifest (clippy::pedantic = deny)
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
    ├── Cargo.toml
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
- Migration-on-first-launch path (FR-005): if the loaded JSON shape is missing fields, fill in `Default` values and write back; idempotent.

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
| `bridge/commands.rs` | `wasm-bindgen-test` (mock-injected) | **YES (RED-first)** | One test per command verifying serde round-trip of args/returns + error variants. New commands extend `tauriMock.js` first per FR-010. |
| `bridge/availability.rs` | `wasm-bindgen-test` | **YES (RED-first)** | `window.__TAURI_INTERNALS__` presence detection; short-circuit return shapes. |
| `bridge/events.rs` | `wasm-bindgen-test` | **YES (RED-first)** | Typed payload deserialisation per event. |
| `managers/auth.rs` | `cargo test` (pure) + `wasm-bindgen-test` (DOM) | **YES (RED-first)** | State machine transitions: Unauth → SignIn → Authed; Authed → SignOut → Guest. |
| `managers/session.rs` | `cargo test` | **YES (RED-first)** | CRUD reductions; date-grouping invariants. |
| `managers/settings.rs` | `cargo test` | **YES (RED-first)** | Load/save round-trip; migration of missing serde-default fields (mirrors `app_settings_missing_serde_default_fields_use_defaults` test in `src-tauri/src/lib.rs:1241`). |
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

  e2e:
    - (cd src && trunk build --release)
    - (cd tests/e2e && npx playwright test --grep-invert "@visual")

  visual:
    - (cd tests/e2e && npx playwright test visual-regression.spec.js)
```

**Pre-commit hook** (husky-equivalent) post-cutover:
- `cargo fmt --check` on touched Rust files (workspace-wide if any Rust changed).
- `cargo clippy --workspace --frozen -- -D warnings` on PR (full pedantic on CI).
- Lockfile-drift check covers both `Cargo.lock` (vs. `Cargo.toml` workspace members) and `tests/e2e/package-lock.json` (vs. `tests/e2e/package.json`).
- `--no-verify` not used (Principle III).

Stage transitions in CI run sequentially; a failing earlier stage short-circuits later ones, except `visual` which always runs when `e2e` passes (and is the documented green-light gate per Principle IV).

## Implementation phasing

Internal phasing for the implementation phase. The actual task breakdown is `/speckit-tasks`'s job — this section names the phases, their entry/exit criteria, and the test-first commit ordering they require.

### Phase 0 — Workspace scaffold

**Entry**: clean branch `001-leptos-migration` post-spec.
**Exit**: workspace `Cargo.toml` with members `src`, `src-tauri`, `tools/build-themes`; empty Leptos crate that compiles to WASM via `trunk build`; Trunk hello-world page that Tauri can serve at `127.0.0.1:1420`. Existing JS app still runs alongside (deletion is Phase 6).
**Test-first**: N/A (scaffolding).

### Phase 1 — Bridge module (test-first)

**Entry**: Phase 0 complete.
**Exit**: every Tauri command in [contracts/tauri-bridge.md](./contracts/tauri-bridge.md) has a typed Leptos wrapper in `src/src/bridge/commands.rs`, with a passing test (RED commit precedes GREEN). `bridge/availability.rs` and `bridge/events.rs` complete. `tauriMock.js` already covers all current commands; if the contract enumeration surfaces unmocked commands, they get mocked here first (per FR-010 ordering).
**Test-first**: YES, per Principle V. Each command: failing-test commit → implementation commit.

### Phase 2 — Engine port (test-first)

**Entry**: Phase 1 complete.
**Exit**: `engine/timer.rs` passes every behaviour test that existed for `src/core/pomodoro-timer.js`. RED-GREEN-REFACTOR commit per behaviour. `ActivitySignal` reduction complete and tested.
**Test-first**: YES, RED-first per behaviour. SC-007 explicit.

### Phase 3 — Managers (test-first, one at a time)

**Entry**: Phase 2 complete.
**Exit**: each manager (`auth`, `session`, `settings`, `navigation`, `tag`, `team`, `update`) has its state machine in Rust with passing RED-first tests. Order recommended: `settings` → `navigation` → `tag` → `session` → `auth` → `update` → `team` (matches dependency order; `auth` later because it requires the new Supabase Tauri-side adapter to land first).
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
**Exit**: deleted from repo root: `package.json`, `package-lock.json`, `node_modules/`, `vite.config.js`, `vitest.config.js`, `eslint.config.js`, `tsconfig.json`, `src/globals.d.ts`, `build-themes.js`. Deleted from `src/`: `main.js`, `managers/*.js`, `core/*.js`, `utils/*.js`. Deleted from `tests/`: `core/`, `managers/`, `utils/` Vitest specs (their behaviour is preserved by Phase 1–3 Rust tests). Survives: `tests/e2e/package.json`, `tests/e2e/package-lock.json` (scoped per [research.md](./research.md) §4).
**Test-first**: N/A (deletion). Re-run visual regression suite as the final cutover gate.

## Post-design Constitution Check

Re-checked after Phase 1 design (research.md, data-model.md, contracts/tauri-bridge.md, quickstart.md). Verdicts unchanged from §[Constitution Check](#constitution-check). No principle moved from PASS to VIOLATION. New evidence per principle:

- **I**: data-model.md confirms `TimerMode` is a Rust enum and `engine/` has no `web-sys` dependency declared.
- **III**: contracts/tauri-bridge.md uses sum types for command error variants; no `String` error escape hatch beyond the existing `Result<T, String>` shapes inherited from `src-tauri/src/lib.rs` (preserved for cutover-period parity; tightening to typed errors is a follow-up per the constitution's "no scope-creep" guidance, not a violation).
- **V**: §[Testing strategy](#testing-strategy-and-test-first-markers) explicitly enumerates RED-first scope.
- **VI**: contracts/tauri-bridge.md is the documented bridge contract; the mock-mirroring rule from FR-010 is reaffirmed at the bottom of that file.

## Complexity Tracking

> No Constitution Check violations require justification.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none) | — | — |
