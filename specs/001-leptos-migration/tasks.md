# Tasks: Leptos Frontend Migration

**Input**: Design documents from `/specs/001-leptos-migration/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-bridge.md

**Tests**: REQUIRED for Principle V scope (engine, manager state machines, persistence helpers, time-keeping math, BridgeError, bridge wrappers). Per AGENTS.md §Test-first commit ordering, the failing-test commit MUST precede the implementation commit; a single combined commit is rejected.

**Organization**: Tasks follow `plan.md` §Implementation phasing (Phase 0 → Phase 7). The plan's phasing is decided and is **not** redesigned here; spec User Stories are mapped via the `[USn]` story label on each task. See §Story label mapping below.

## Format: `[ID] [T:RED|T:GREEN] [P?] [Story] Description — done-signal`

- `[T:RED]` — failing-test commit (Principle V scope only; see plan.md §"Audit method for test-first ordering").
- `[T:GREEN]` — implementation commit; the corresponding RED task's tests must pass after this lands.
- Mock-only Phase 0.5 changes are `[T:GREEN]` only (the e2e suite is the integration test; no per-mock RED).
- `[P]` — parallelisable (different files, no dependencies on incomplete tasks in the same batch).
- Story label `[USn]` maps to spec.md user stories: `[US1]` cutover acceptance, `[US2]` quality pipeline green, `[US3]` typed bridge compile-time check, `[US4]` visual regression in steady state.
- Each task has a runnable **done-signal** following an em-dash.

## Story label mapping

| User Story | Priority | Label | Anchor task scope |
|---|---|---|---|
| US1: User updates via auto-updater, notices nothing | P1 | `[US1]` | Engine, managers (auth, session, settings), persistence wrappers, legacy-localStorage migration, theme, components, tray |
| US2: Developer clones, full quality pipeline goes green | P1 | `[US2]` | Workspace scaffold, lints/format, CI gates, e2e + visual regression survival, lockfile discipline |
| US3: Typed bridge catches mismatch at compile time | P2 | `[US3]` | BridgeError introduction, every bridge command wrapper, mock reconciliation |
| US4: Visual regression catches drift in steady state | P3 | `[US4]` | baseline-cap CI gate, engine `web-sys` grep gate |

## Multi-agent batching note

248 tasks across 9 phases (Phase 0, 0.5, 1, 2, 3, 4, 5, 6, 7); per-agent soft cap is 50 (per `/manage-feature` step 12). Phase totals: Phase 0 = 8; Phase 0.5 = 14; Phase 1 = 97 (sub-phase breakdown: 1A=7, 1B=2, 1C=52, 1D=15, 1E=17, 1F=2, 1G=2); Phase 2 = 27; Phase 3 = 42; Phase 4 = 30; Phase 5 = 7; Phase 6 = 18; Phase 7 = 5. Sum: 8 + 14 + 97 + 27 + 42 + 30 + 7 + 18 + 5 = 248.

Implementation batches across ~12–15 subagents at ~15–20 tasks each, sliced along **sub-phase** boundaries (since that's how Phase 1 actually slices) to preserve commit ordering: A=Phase 0 (8); B=Phase 0.5 (14); C0=Phase 1A BridgeError + closed-domain enum tightening (7; gates C1–C5); C1=Phase 1B BridgeAvailable (2); C2=Phase 1C surviving-command wrappers (52); C3=Phase 1D new permanent commands (15); C4=Phase 1E transition-only `import_legacy_*` (17); C5=Phase 1F events (2) + Phase 1G `BridgeAvailable` short-circuit wiring (2) (4 total); D=Phase 2 engine (27); E=Phase 3 managers (42); F=Phase 4 components (30); G=Phase 5 theme + assets (7); H=Phase 6 cleanup (18; visual regression is the gate); I=Phase 7 CI hardening (5).

## Constitution Check

Per principle, Roman numeral + name (per plan-template.md):

- **I. The Timer Is Sacred** — PASS. Engine tasks T120–T145 (Phase 2) all RED-first; `web-sys` forbidden in engine (grep gate T246–T247).
- **II. Local-First, Privacy-Default** — PASS. `track_event` opt-in Rust-side (T086); Supabase guest mode preserved (T175–T182); PII never logged.
- **III. Type Safety Over Defensive Code** — PASS. `BridgeError` typed enum (T023–T027); clippy pedantic posture (T002, T240).
- **IV. Visual Regression Is The UI Contract** — PASS. baseline-cap CI gate (T244–T245); final visual regression run (T242) with 0 baselines re-captured.
- **V. Test-First For Stateful Engines** — PASS. Every Principle V-scope module has paired `[T:RED]` / `[T:GREEN]` tasks (engine T120–T145, managers T147–T188, BridgeError T023–T024, bridge wrappers T032–T119, theme code-gen T219–T220).
- **VI. The Tauri Boundary Is Stable** — PASS. Mock-first rule at Phase 0.5 (T009–T022) and Phase 1 (T084, T087, T096, T099); only `invoke()` + `listen()` IPC.
- **VII. No Upstream Compatibility Burden** — PASS. 10 unused commands deleted (T226–T234).
- **VIII. Spec-Driven Feature Flow** — PASS. tasks.md generated from plan.md.
- **IX. Lock Files Are First-Class** — PASS. Workspace `Cargo.lock` (T001, T007); root `package-lock.json` deleted (T237); lockfile-drift hook (T248).

**Violations**: 0. Complexity tracking: N/A.

---

## Phase 0 — Workspace scaffold (Setup)

**Goal**: Cargo workspace, Trunk hello-world, `tauri.conf.json` paths, CI green baseline.
**Test-first**: N/A (scaffolding).

- [ ] T001 [US2] Create workspace `Cargo.toml` at repo root with `[workspace] members = ["src", "src-tauri", "tools/build-themes"]` and `[workspace.lints.clippy] pedantic = "deny"`. (Note: this overrides constitution III's "warn-now" line; the post-merge constitution revision will make pedantic-deny the formal stance per user directive 2026-05-09.) — done-signal: `cargo metadata --format-version 1 --no-deps | jq '.workspace_members | length'` returns 3; `cargo clippy -p presto_lib -- -W clippy::pedantic` and `cargo clippy -p presto-web --target wasm32-unknown-unknown -- -W clippy::pedantic` both honour the workspace `deny` posture (clippy treats pedantic warnings as errors on both crates).
- [ ] T002 [P] [US2] Create `src/Cargo.toml` for the Leptos crate (`[package].name = "presto-web"`, `crate-type = ["cdylib","rlib"]`, `[dependencies] leptos = { version = "0.7", features = ["csr"] }`, `wasm-bindgen`, `web-sys`, `js-sys`, `serde`, `serde-wasm-bindgen`, `gloo-storage`, `thiserror`, `chrono`) — done-signal: `cargo check -p presto-web --target wasm32-unknown-unknown` returns 0.
- [ ] T003 [P] [US2] Create `tools/build-themes/Cargo.toml` (`[package].name = "presto-build-themes"`) and a stub `tools/build-themes/src/main.rs` that writes a one-line placeholder to `src/src/theme/themes.rs` — done-signal: `cargo run -p presto-build-themes` returns 0; output file exists.
- [ ] T004 [US2] Add `src/Trunk.toml` with default `dist-dir = "dist"` and a `[[hooks]]` entry `pre_build = "cargo run -p presto-build-themes"`; create `src/index.html` Trunk entry — done-signal: `(cd src && trunk build)` returns 0; `src/dist/index.html` exists.
- [ ] T005 [US2] Add `src/src/main.rs` (`leptos::mount_to_body(|| view! { <p>"hello"</p> })`) and `src/src/app.rs` placeholder; the JS app under `src/main.js` is left in place (deletion is Phase 6) — done-signal: `(cd src && trunk build --release)` returns 0; the WASM bundle is in `src/dist/`.
- [ ] T006 [US2] Update `src-tauri/tauri.conf.json`: `frontendDist = "../src/dist"`, `beforeDevCommand = "cd src && trunk serve --port 1420"`, `beforeBuildCommand = "cd src && trunk build --release"`, `devUrl = "http://127.0.0.1:1420"` — done-signal: `cargo tauri build --no-bundle` returns 0; the bundle includes the Trunk-built `dist/`.
- [ ] T007 [US2] Move `src-tauri/Cargo.lock` to repo root as `Cargo.lock`; update `.gitignore` to track only the workspace-root `Cargo.lock` and ignore each member's `target/` — done-signal: `cargo build --workspace --frozen` returns 0; `git ls-files | grep -c '^Cargo.lock$'` returns 1.
- [ ] T008 [US2] Add a CI baseline job to `.agentex.yml` `qa.setup` that runs `cargo fetch --locked` and `(cd tests/e2e && npm ci)` — done-signal: a throwaway-branch CI run exits 0 on the new setup stage.

**Checkpoint**: workspace builds; Trunk hello-world serves; Tauri builds against `src/dist/`. Phase 0.5 may begin.

---

## Phase 0.5 — Mock/handler reconciliation (Foundational)

**Goal**: Reconcile `tests/e2e/fixtures/tauriMock.js` to today's surviving handler set in `src-tauri/src/lib.rs` before any new commands or wrappers are added.
**Test-first**: N/A — the mock is a test fixture; correctness is verified by the e2e suite.

### Add 8 missing mock entries

- [ ] T009 [P] [US3] Add `case "get_stats_history":` returning `[]` to `tests/e2e/fixtures/tauriMock.js` — done-signal: `(cd tests/e2e && npx playwright test --grep history)` returns 0.
- [ ] T010 [P] [US3] Add `case "reset_all_data":` returning `undefined` to `tests/e2e/fixtures/tauriMock.js` — done-signal: `(cd tests/e2e && npx playwright test --grep reset)` returns 0.
- [ ] T011 [P] [US3] Add `case "save_daily_stats":` returning `undefined` to `tests/e2e/fixtures/tauriMock.js` — done-signal: e2e suite continues to pass.
- [ ] T012 [P] [US3] Add `case "start_activity_monitoring":` returning `undefined` to `tests/e2e/fixtures/tauriMock.js` — done-signal: e2e suite continues to pass.
- [ ] T013 [P] [US3] Add `case "stop_activity_monitoring":` returning `undefined` to `tests/e2e/fixtures/tauriMock.js` — done-signal: e2e suite continues to pass.
- [ ] T014 [P] [US3] Add `case "update_activity_timeout":` returning `undefined` to `tests/e2e/fixtures/tauriMock.js` — done-signal: e2e suite continues to pass.
- [ ] T015 [P] [US3] Add `case "update_tray_icon":` returning `undefined` to `tests/e2e/fixtures/tauriMock.js` — done-signal: e2e suite continues to pass.
- [ ] T016 [P] [US3] Add `case "update_tray_menu":` returning `undefined` to `tests/e2e/fixtures/tauriMock.js` — done-signal: e2e suite continues to pass.

### Remove 4 stale mock-only entries

- [ ] T017 [P] [US3] Remove stale `case "append_daily_stats":` from `tests/e2e/fixtures/tauriMock.js` — done-signal: full e2e suite (`npx playwright test`) returns 0.
- [ ] T018 [P] [US3] Remove stale `case "delete_all_data":` from `tests/e2e/fixtures/tauriMock.js` — done-signal: e2e suite returns 0.
- [ ] T019 [P] [US3] Remove stale `case "load_history":` from `tests/e2e/fixtures/tauriMock.js` — done-signal: e2e suite returns 0.
- [ ] T020 [P] [US3] Remove stale `case "open_url":` from `tests/e2e/fixtures/tauriMock.js` — done-signal: e2e suite returns 0.

### Drift gate + verification

- [x] T021 [US3] Add a CI step to `.agentex.yml` (or `scripts/check-mock-drift.sh`) that greps `src-tauri/src/lib.rs` for `#[tauri::command]` names and asserts each name appears as a `case` in `tauriMock.js`, and inverse — done-signal: the script exits 0 on the reconciled HEAD; intentionally renaming a case in a throwaway branch makes it exit 1.
- [ ] T022 [US3] Run the full e2e + visual regression suite as the verification of the reconciled mock — done-signal: `(cd tests/e2e && npx playwright test)` returns 0; `(cd tests/e2e && npx playwright test visual-regression.spec.js)` returns 0 with 0 baselines re-captured.

**Checkpoint**: mock matches today's handler set. Phase 1 may add new commands.

---

## Phase 1 — Bridge module (test-first)

**Goal**: every surviving Tauri command has a typed Leptos wrapper in `src/src/bridge/commands.rs`; every command (existing + new) returns `Result<T, BridgeError>`. The 6 new permanent commands (`track_event`, four `supabase_*`, `export_sessions_xlsx`) and 7 transition-only `import_legacy_*` commands land behind the mock-first rule.

**Test-first**: YES — RED commit precedes GREEN per Principle V.

### Phase 1A — `BridgeError` + closed-domain enum tightening (7 tasks)

- [x] T023 [T:RED] [US3] Write `src/src/bridge/error.rs` `tests::*` exercising serde round-trip for every `BridgeError` variant (`BridgeUnavailable`, `NotAuthenticated`, `InvalidArgument { field, reason }`, `NotFound { resource }`, `SerdeRoundtrip { command, error }`, `Internal { msg }`); externally-tagged JSON (`{"kind":"invalid_argument","field":"email","reason":"empty"}`); commit RED. The same RED commit also lands the Tauri-side `bridge_error_serde_roundtrip` test in `src-tauri/src/lib.rs` (mirrors the externally-tagged JSON shape from the Leptos side); both tests fail with the same shape mismatch — done-signal: `cargo test -p presto-web bridge::error::tests` AND `cargo test -p presto_lib bridge_error_serde_roundtrip` both exit non-zero (no impl yet on either side).
- [x] T024 [T:GREEN] [US3] Implement `BridgeError` in `src/src/bridge/error.rs` mirroring data-model.md §`BridgeError`; `#[serde(tag = "kind", rename_all = "snake_case")]` — done-signal: `cargo test -p presto-web bridge::error::tests` returns 0.
- [x] T025 [T:GREEN] [US3] Mirror `BridgeError` in `src-tauri/src/lib.rs` with the identical serde derive (the Tauri-side test from T023 passes AS A CONSEQUENCE of this implementation, not as a freshly-introduced assertion) — done-signal: `cargo test -p presto_lib bridge_error_serde_roundtrip` returns 0.
- [x] T026 [US3] Mechanically rewrite every `.map_err(|e| e.to_string())` call site in `src-tauri/src/lib.rs` to `BridgeError::Internal { msg: e.to_string() }`, defaulting to `Internal` and tightening to `NotFound` / `InvalidArgument` / `NotAuthenticated` where the call site has semantic context (per research.md §13) — done-signal: `cargo build --workspace --frozen` returns 0; no `Result<_, String>` survives in `#[tauri::command]` handler signatures.
- [x] T027 [US3] Tighten `update_tray_menu` `current_mode: String → TimerMode` and `update_tray_icon` `session_mode: String → TimerMode` (per data-model.md §`TimerMode`); update the existing handlers and any call sites — done-signal: `cargo build --workspace --frozen` returns 0.
- [x] T028 [T:RED] [US3] Write `src/src/bridge/types.rs` `tests::session_type_serde_roundtrip` exercising serde round-trip for every `SessionType` variant (`Focus`, `Break`, `LongBreak`, `Custom`); on-disk wire form is camelCase strings (`"focus"`, `"break"`, `"longBreak"`, `"custom"`) per data-model.md §`SessionType`; the test fixture deserialises a JSON object literally containing `"session_type": "longBreak"` and asserts it round-trips to `SessionType::LongBreak` and back; commit RED — done-signal: `cargo test -p presto-web bridge::types::tests::session_type_serde_roundtrip` exits non-zero (no impl yet). _(Implementation note: filename is `bridge/session_type.rs` rather than `bridge/types.rs` per the implementation dispatch — keeps the module focused on one type.)_
- [x] T029 [T:GREEN] [US3] Implement `SessionType` enum in `src/src/bridge/types.rs` (mirror in `src-tauri/src/lib.rs`) per data-model.md §`SessionType` with `#[serde(rename_all = "camelCase")]`; tighten `ManualSession.session_type: String → SessionType` on both sides; update the test fixtures at `src-tauri/src/lib.rs:1370,1388` and `src-tauri/src/helpers.rs:681` to construct `SessionType::Focus` / `SessionType::Break` instead of `"focus".to_string()` / `"break".to_string()` — done-signal: `cargo test -p presto-web bridge::types::tests::session_type_serde_roundtrip` returns 0; `cargo build --workspace --frozen` returns 0; no `session_type: String` survives in `ManualSession` on either side of the bridge.

### Phase 1B — `BridgeAvailable` + bridge module skeleton

- [ ] T030 [T:RED] [US3] Write `src/src/bridge/availability.rs` `tests::*` (wasm-bindgen-test): `BridgeAvailable::Available` when `window.__TAURI_INTERNALS__` is present; `Absent` otherwise; commit RED — done-signal: `(cd src && wasm-pack test --node)` for `bridge::availability::tests` returns non-zero.
- [ ] T031 [T:GREEN] [US3] Implement `BridgeAvailable` enum + `bridge_available()` reading `window.__TAURI_INTERNALS__` once — done-signal: wasm-bindgen-test returns 0.

### Phase 1C — Per-surviving-command wrappers (RED + GREEN per command — 26 commands × 2 = 52 tasks)

> Each pair: T:RED writes a `wasm-bindgen-test` for the wrapper (uses the existing mock entry from Phase 0.5; asserts serde shape of args/return + `BridgeError` round-trip). T:GREEN adds the wrapper in `src/src/bridge/commands.rs`. Done-signal for each pair: `(cd src && wasm-pack test --node bridge::commands::tests::<cmd>)` non-zero (RED) → 0 (GREEN). All wrappers signed per contracts/tauri-bridge.md.

#### Persistence — sessions (4 commands → 8 tasks)

- [ ] T032 [T:RED] [US1] `bridge::commands::tests::save_session_data_round_trip` in `src/src/bridge/commands.rs` — done-signal: wasm-bindgen-test fails compilation or assertion.
- [ ] T033 [T:GREEN] [US1] Implement `pub async fn save_session_data(session: Session) -> Result<(), BridgeError>` — done-signal: test passes.
- [ ] T034 [T:RED] [US1] `bridge::commands::tests::load_session_data_round_trip` — done-signal: fails.
- [ ] T035 [T:GREEN] [US1] Implement `pub async fn load_session_data() -> Result<Option<Session>, BridgeError>` — done-signal: passes.
- [ ] T036 [T:RED] [US1] `bridge::commands::tests::get_stats_history_round_trip` — done-signal: fails.
- [ ] T037 [T:GREEN] [US1] Implement `pub async fn get_stats_history() -> Result<Vec<Session>, BridgeError>` — done-signal: passes.
- [ ] T038 [T:RED] [US1] `bridge::commands::tests::save_daily_stats_round_trip` — done-signal: fails.
- [ ] T039 [T:GREEN] [US1] Implement `pub async fn save_daily_stats(session: Session) -> Result<(), BridgeError>` — done-signal: passes.

#### Persistence — tasks (2 commands → 4 tasks)

- [ ] T040 [T:RED] [US1] `bridge::commands::tests::save_tasks_round_trip` — done-signal: fails.
- [ ] T041 [T:GREEN] [US1] Implement `pub async fn save_tasks(tasks: Vec<Task>) -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T042 [T:RED] [US1] `bridge::commands::tests::load_tasks_round_trip` — done-signal: fails.
- [ ] T043 [T:GREEN] [US1] Implement `pub async fn load_tasks() -> Result<Vec<Task>, BridgeError>` — done-signal: passes.

#### Persistence — manual sessions (2 commands → 4 tasks)

- [ ] T044 [T:RED] [US1] `bridge::commands::tests::save_manual_sessions_round_trip` — done-signal: fails.
- [ ] T045 [T:GREEN] [US1] Implement `pub async fn save_manual_sessions(sessions: Vec<ManualSession>) -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T046 [T:RED] [US1] `bridge::commands::tests::load_manual_sessions_round_trip` — done-signal: fails.
- [ ] T047 [T:GREEN] [US1] Implement `pub async fn load_manual_sessions() -> Result<Vec<ManualSession>, BridgeError>` — done-signal: passes.

#### Persistence — tags (4 commands → 8 tasks)

- [ ] T048 [T:RED] [US1] `bridge::commands::tests::load_tags_round_trip` — done-signal: fails.
- [ ] T049 [T:GREEN] [US1] Implement `pub async fn load_tags() -> Result<Vec<Tag>, BridgeError>` — done-signal: passes.
- [ ] T050 [T:RED] [US1] `bridge::commands::tests::save_tag_round_trip` — done-signal: fails.
- [ ] T051 [T:GREEN] [US1] Implement `pub async fn save_tag(tag: Tag) -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T052 [T:RED] [US1] `bridge::commands::tests::delete_tag_round_trip` — done-signal: fails.
- [ ] T053 [T:GREEN] [US1] Implement `pub async fn delete_tag(tag_id: String) -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T054 [T:RED] [US1] `bridge::commands::tests::add_session_tag_round_trip` — done-signal: fails.
- [ ] T055 [T:GREEN] [US1] Implement `pub async fn add_session_tag(session_tag: SessionTag) -> Result<(), BridgeError>` — done-signal: passes.

#### Settings & lifecycle (3 commands → 6 tasks)

- [ ] T056 [T:RED] [US1] `bridge::commands::tests::save_settings_round_trip` — done-signal: fails.
- [ ] T057 [T:GREEN] [US1] Implement `pub async fn save_settings(settings: Settings) -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T058 [T:RED] [US1] `bridge::commands::tests::load_settings_round_trip` (covers the legacy `hide_status_bar` fallback path via mock fixture returning the legacy shape; asserts deserialised `status_bar_display`) — done-signal: fails.
- [ ] T059 [T:GREEN] [US1] Implement `pub async fn load_settings() -> Result<Settings, BridgeError>` — done-signal: passes.
- [ ] T060 [T:RED] [US1] `bridge::commands::tests::reset_all_data_round_trip` — done-signal: fails.
- [ ] T061 [T:GREEN] [US1] Implement `pub async fn reset_all_data() -> Result<(), BridgeError>` — done-signal: passes.

#### Global shortcuts (1 command → 2 tasks)

- [ ] T062 [T:RED] [US1] `bridge::commands::tests::register_global_shortcuts_round_trip` — done-signal: fails.
- [ ] T063 [T:GREEN] [US1] Implement `pub async fn register_global_shortcuts(shortcuts: ShortcutSettings) -> Result<(), BridgeError>` — done-signal: passes.

#### Activity monitoring (3 commands → 6 tasks)

- [ ] T064 [T:RED] [US1] `bridge::commands::tests::start_activity_monitoring_round_trip` — done-signal: fails.
- [ ] T065 [T:GREEN] [US1] Implement `pub async fn start_activity_monitoring(timeout_seconds: u64) -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T066 [T:RED] [US1] `bridge::commands::tests::stop_activity_monitoring_round_trip` — done-signal: fails.
- [ ] T067 [T:GREEN] [US1] Implement `pub async fn stop_activity_monitoring() -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T068 [T:RED] [US1] `bridge::commands::tests::update_activity_timeout_round_trip` — done-signal: fails.
- [ ] T069 [T:GREEN] [US1] Implement `pub async fn update_activity_timeout(timeout_seconds: u64) -> Result<(), BridgeError>` — done-signal: passes.

#### Autostart (3 commands → 6 tasks)

- [ ] T070 [T:RED] [US1] `bridge::commands::tests::enable_autostart_round_trip` — done-signal: fails.
- [ ] T071 [T:GREEN] [US1] Implement `pub async fn enable_autostart() -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T072 [T:RED] [US1] `bridge::commands::tests::disable_autostart_round_trip` — done-signal: fails.
- [ ] T073 [T:GREEN] [US1] Implement `pub async fn disable_autostart() -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T074 [T:RED] [US1] `bridge::commands::tests::is_autostart_enabled_round_trip` (asserts `Absent` short-circuits to `Ok(false)`) — done-signal: fails.
- [ ] T075 [T:GREEN] [US1] Implement `pub async fn is_autostart_enabled() -> Result<bool, BridgeError>` — done-signal: passes.

#### Window & tray (2 commands → 4 tasks)

- [ ] T076 [T:RED] [US1] `bridge::commands::tests::update_tray_icon_round_trip` (asserts `TimerMode` enum serialises as camelCase) — done-signal: fails.
- [ ] T077 [T:GREEN] [US1] Implement `pub async fn update_tray_icon(args: UpdateTrayIconArgs) -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T078 [T:RED] [US1] `bridge::commands::tests::update_tray_menu_round_trip` — done-signal: fails.
- [ ] T079 [T:GREEN] [US1] Implement `pub async fn update_tray_menu(is_running: bool, is_paused: bool, current_mode: TimerMode) -> Result<(), BridgeError>` — done-signal: passes.

#### Export (1 command → 2 tasks; deprecated by `export_sessions_xlsx`, removed in Phase 6)

- [ ] T080 [T:RED] [US1] `bridge::commands::tests::write_excel_file_round_trip` — done-signal: fails.
- [ ] T081 [T:GREEN] [US1] Implement `pub async fn write_excel_file(path: String, data: String) -> Result<(), BridgeError>` (kept for cutover-period parity) — done-signal: passes.

#### OAuth (1 command → 2 tasks)

- [ ] T082 [T:RED] [US1] `bridge::commands::tests::start_oauth_server_round_trip` — done-signal: fails.
- [ ] T083 [T:GREEN] [US1] Implement `pub async fn start_oauth_server() -> Result<u16, BridgeError>` — done-signal: passes.

### Phase 1D — New permanent commands (6 commands × {mock entry, RED, GREEN} = 18 tasks)

- [ ] T084 [US1] Add `case "track_event":` returning `undefined` to `tauriMock.js` (mock-first per FR-010) — done-signal: e2e suite continues to pass.
- [ ] T085 [T:RED] [US1] `bridge::commands::tests::track_event_round_trip` — done-signal: fails.
- [ ] T086 [T:GREEN] [US1] Implement Tauri handler `track_event(name, props)` in `src-tauri/src/lib.rs` checking `are_analytics_enabled` Rust-side; add Leptos wrapper `pub async fn track_event(name: &str, props: Option<HashMap<String, serde_json::Value>>) -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T087 [US1] Add four `case "supabase_*":` mock entries (`sign_in_with_password`, `sign_out`, `get_session`, `refresh_session`) to `tauriMock.js` per contracts/tauri-bridge.md mock examples — done-signal: e2e suite continues to pass.
- [ ] T088 [T:RED] [US1] `bridge::commands::tests::supabase_sign_in_with_password_round_trip` — done-signal: fails.
- [ ] T089 [T:GREEN] [US1] Implement Tauri handler `supabase_sign_in_with_password(email, password) -> Result<AuthSession, BridgeError>` (REST adapter under `src-tauri/src/auth/mod.rs`) and Leptos wrapper — done-signal: passes.
- [ ] T090 [T:RED] [US1] `bridge::commands::tests::supabase_sign_out_round_trip` — done-signal: fails.
- [ ] T091 [T:GREEN] [US1] Implement Tauri handler + Leptos wrapper for `supabase_sign_out(refresh_token) -> Result<(), BridgeError>` — done-signal: passes.
- [ ] T092 [T:RED] [US1] `bridge::commands::tests::supabase_get_session_round_trip` — done-signal: fails.
- [ ] T093 [T:GREEN] [US1] Implement Tauri handler + Leptos wrapper for `supabase_get_session() -> Result<Option<AuthSession>, BridgeError>` (reads from app-data dir) — done-signal: passes.
- [ ] T094 [T:RED] [US1] `bridge::commands::tests::supabase_refresh_session_round_trip` — done-signal: fails.
- [ ] T095 [T:GREEN] [US1] Implement Tauri handler + Leptos wrapper for `supabase_refresh_session(refresh_token) -> Result<AuthSession, BridgeError>` — done-signal: passes.
- [ ] T096 [US1] Add `case "export_sessions_xlsx":` returning `undefined` to `tauriMock.js` — done-signal: e2e suite continues to pass.
- [ ] T097 [T:RED] [US1] `bridge::commands::tests::export_sessions_xlsx_round_trip` — done-signal: fails.
- [ ] T098 [T:GREEN] [US1] Implement Tauri handler `export_sessions_xlsx(path, sessions)` using `rust_xlsxwriter` and Leptos wrapper — done-signal: passes; an integration test writes a workbook to a temp file and asserts file existence + non-zero size.

### Phase 1E — Transition-only `import_legacy_*` commands (7 commands × {mock entry, RED bridge::storage test, GREEN handler+wrapper} = 21 tasks)

> Each command has a wasm-bindgen-test in `src/src/bridge/storage.rs` (managers/auth.rs for the Supabase one; see plan.md §Testing strategy) that exercises the migration with mocked `localStorage` and asserts the matching Tauri command receives the expected payload (idempotent — second run is a no-op).

- [x] T099 [US1] Add 7 `case "import_legacy_*":` returning `undefined` mock entries (`supabase_session`, `settings`, `history`, `tasks`, `tags`, `manual_sessions`, `user_state`) to `tauriMock.js` — done-signal: e2e suite continues to pass.
- [x] T100 [T:RED] [US1] `bridge::storage::tests::imports_legacy_settings` — done-signal: fails.
- [x] T101 [T:GREEN] [US1] Implement Tauri handler `import_legacy_settings(payload) -> Result<(), BridgeError>` (idempotent skip if `AppSettings` already on disk) and Leptos `bridge::storage` reader for `pomodoro-settings` / `theme-preference` / `timer-theme-preference` / `presto_auto_check_updates` localStorage keys — done-signal: passes.
- [x] T102 [T:RED] [US1] `bridge::storage::tests::imports_legacy_history` — done-signal: fails.
- [x] T103 [T:GREEN] [US1] Implement Tauri handler `import_legacy_history(payload)` + Leptos reader for `pomodoro-history` — done-signal: passes.
- [x] T104 [T:RED] [US1] `bridge::storage::tests::imports_legacy_tasks` — done-signal: fails.
- [x] T105 [T:GREEN] [US1] Implement Tauri handler `import_legacy_tasks(payload)` + Leptos reader for `pomodoro-tasks` — done-signal: passes.
- [x] T106 [T:RED] [US1] `bridge::storage::tests::imports_legacy_tags` — done-signal: fails.
- [x] T107 [T:GREEN] [US1] Implement Tauri handler `import_legacy_tags(payload)` + Leptos reader for `presto-tags` — done-signal: passes.
- [x] T108 [T:RED] [US1] `bridge::storage::tests::imports_legacy_manual_sessions` — done-signal: fails.
- [x] T109 [T:GREEN] [US1] Implement Tauri handler `import_legacy_manual_sessions(payload)` + Leptos reader for `presto_manual_sessions` — done-signal: passes.
- [x] T110 [T:RED] [US1] `bridge::storage::tests::imports_legacy_user_state` — done-signal: fails.
- [x] T111 [T:GREEN] [US1] Implement Tauri handler `import_legacy_user_state(payload)` + Leptos reader for `presto-guest-mode` / `presto-auth-seen` / `presto-skipped-versions` / `pomodoro-session` — done-signal: passes.
- [x] T112 [T:RED] [US1] `managers/auth::tests::imports_legacy_supabase_session_from_localstorage` (per plan.md §Testing strategy named test) — done-signal: fails.
- [x] T113 [T:GREEN] [US1] Implement Tauri handler `import_legacy_supabase_session(payload: SupabaseSessionPayload)` and Leptos reader for `sb-<project-ref>-auth-token` localStorage key; idempotency gate per research.md §6 step 4 — done-signal: passes.
- [x] T114 [T:RED] [US1] `bridge::storage::tests::migrate_legacy_localstorage_idempotent` (full entry-point coverage; second-launch is a no-op; partial-failure preserves the localStorage key) — done-signal: fails.
- [x] T115 [T:GREEN] [US1] Implement single Leptos entry point `bridge::storage::migrate_legacy_localstorage()` that runs from `app.rs` startup, dispatching to each per-domain reader, idempotent — done-signal: passes.

### Phase 1F — Events module (`bridge::events`)

- [ ] T116 [T:RED] [US3] `bridge::events::tests::*` for each of the 10 listed events (E1–E10 in contracts/tauri-bridge.md): typed payload deserialisation per event, including the updater-plugin event (E10) — done-signal: fails.
- [ ] T117 [T:GREEN] [US3] Implement `bridge::events::listen_user_activity`, `listen_user_inactivity`, `listen_global_shortcut`, `listen_shortcuts_updated`, `listen_oauth_callback`, `listen_tray_start_session`, `listen_tray_pause`, `listen_tray_skip`, `listen_tray_cancel`, plus updater-plugin events (E10) — done-signal: passes.

### Phase 1G — Wire `BridgeAvailable` short-circuit into every wrapper

- [ ] T118 [T:RED] [US1] `bridge::commands::tests::short_circuits_when_bridge_absent` — every wrapper returns the documented sentinel (empty `Vec`, `false`, `Ok(())`) when `BridgeAvailable::Absent` — done-signal: fails.
- [ ] T119 [T:GREEN] [US1] Wire the `BridgeAvailable` check into all 26 surviving + 6 permanent + 7 transition wrappers — done-signal: passes.

**Checkpoint**: every command in contracts/tauri-bridge.md has a typed wrapper, every error is `BridgeError`, every command short-circuits gracefully when the bridge is absent. Phase 2 may begin.

---

## Phase 2 — Engine port (test-first)

**Goal**: `engine/timer.rs` reproduces the JS timer engine's external behaviour bit-for-bit; `ActivitySignal` reduction lands; `engine::date_format` pins the chrono format.
**Test-first**: YES per behaviour, RED-first per Principle V (FR-014, SC-007).

### Engine — TimerState transitions (mode + session)

- [x] T120 [T:RED] [US1] `engine::timer::tests::starts_in_focus_mode` — done-signal: fails.
- [x] T121 [T:GREEN] [US1] Implement `TimerState::new()` and the `Idle → Focus` transition — done-signal: passes.
- [x] T122 [T:RED] [US1] `engine::timer::tests::focus_completes_after_25min_emits_pomodoro_completed` — done-signal: fails.
- [x] T123 [T:GREEN] [US1] Implement focus-complete transition + `pomodoroCompleted` event emission — done-signal: passes.
- [x] T124 [T:RED] [US1] `engine::timer::tests::break_after_focus` — done-signal: fails.
- [x] T125 [T:GREEN] [US1] Implement `Focus → Break` transition — done-signal: passes.
- [x] T126 [T:RED] [US1] `engine::timer::tests::long_break_after_4_focus_sessions` — done-signal: fails.
- [x] T127 [T:GREEN] [US1] Implement `Focus → LongBreak` after 4 cycles — done-signal: passes.

### Engine — Drift compensation

- [x] T128 [T:RED] [US1] `engine::timer::tests::drift_compensation_recovers_90s_of_os_suspend` (SC-005, AS-1.3) — done-signal: fails.
- [x] T129 [T:GREEN] [US1] Implement wall-clock-anchored elapsed computation in `TimerState::tick(now_ms)` — done-signal: passes.

### Engine — Smart-pause activity gate

- [x] T130 [T:RED] [US1] `engine::activity_signal::tests::idle_active_edge_detection` — done-signal: fails.
- [x] T131 [T:GREEN] [US1] Implement `ActivitySignal` reduction (Idle ↔ Active edge detection; mid-state events folded) — done-signal: passes.
- [x] T132 [T:RED] [US1] `engine::timer::tests::smart_pause_pauses_after_inactive_timeout` — done-signal: fails.
- [x] T133 [T:GREEN] [US1] Wire `ActivitySignal` consumption into `TimerState::tick` for smart-pause — done-signal: passes.
- [x] T134 [T:RED] [US1] `engine::timer::tests::smart_pause_resumes_on_activity` — done-signal: fails.
- [x] T135 [T:GREEN] [US1] Implement smart-pause resume path — done-signal: passes.

### Engine — Max-session cap

- [x] T136 [T:RED] [US1] `engine::timer::tests::max_session_cap_stops_at_total_sessions` — done-signal: fails.
- [x] T137 [T:GREEN] [US1] Implement max-session-cap stop transition — done-signal: passes.

### Engine — Manual session entry

- [x] T138 [T:RED] [US1] `engine::timer::tests::manual_session_entry_routes_through_engine` (Principle I rule "manual session entry must go through the same engine path as live sessions") — done-signal: fails.
- [x] T139 [T:GREEN] [US1] Implement `TimerState::record_manual_session(session)` path — done-signal: passes.

### Engine — Reset / Skip

- [x] T140 [T:RED] [US1] `engine::timer::tests::reset_returns_to_initial_state` — done-signal: fails.
- [x] T141 [T:GREEN] [US1] Implement reset transition — done-signal: passes.
- [x] T142 [T:RED] [US1] `engine::timer::tests::skip_advances_to_next_mode_without_emitting_completed` — done-signal: fails.
- [x] T143 [T:GREEN] [US1] Implement skip transition — done-signal: passes.

### Engine — Date format pinning

- [x] T144 [T:RED] [US1] `engine::date_format::tests::matches_js_to_date_string` — iterates 366 dates and asserts `chrono_format(d) == js_to_date_string(d)`; chrono format string `"%a %b %d %Y"` per data-model.md §`Session.date` — done-signal: fails.
- [x] T145 [T:GREEN] [US1] Implement `engine::date_format::format_session_date(timestamp_ms) -> String` — done-signal: passes.

### Engine — `web-sys` purity gate (CI-coupled to T224)

- [x] T146 [US1] Run the engine `web-sys` grep gate locally on a clean Phase 2 HEAD — done-signal: `if grep -rE "web_sys|web-sys" src/src/engine/ ; then exit 1; fi` exits 0.

**Checkpoint**: every behaviour rule from `src/core/pomodoro-timer.js` has a passing Rust test. SC-007 satisfied.

---

## Phase 3 — Managers (test-first, in dependency order)

**Goal**: each manager (`auth`, `session`, `settings`, `navigation`, `tag`, `team`, `update`) has a Rust state machine with passing RED-first tests. Order recommended: `settings` → `navigation` → `tag` → `session` → `auth` → `update` → `team`.

**Test-first**: YES per manager.

### managers/settings (10 tasks)

- [x] T147 [T:RED] [US1] `managers::settings::tests::load_returns_default_when_missing_file` — done-signal: fails.
- [x] T148 [T:GREEN] [US1] Implement `Settings::load()` calling `bridge::commands::load_settings` and falling back to default — done-signal: passes.
- [x] T149 [T:RED] [US1] `managers::settings::tests::missing_serde_default_fields_use_defaults` (mirrors `app_settings_missing_serde_default_fields_use_defaults` in `src-tauri/src/lib.rs:1241`) — done-signal: fails.
- [x] T150 [T:GREEN] [US1] Implement field-level `#[serde(default = "...")]` defaults on `Settings` and nested types — done-signal: passes.
- [x] T151 [T:RED] [US1] `managers::settings::tests::migrates_hide_status_bar_to_status_bar_display` (per plan.md §Testing strategy named test; covers all five cases in data-model.md §Settings legacy migration: `hide_status_bar:true → IconOnly`, `hide_status_bar:false → Default`, `status_bar_display: "icon-only" present → IconOnly` (kebab-case round-trip from a pre-cutover JS-era settings JSON fixture), `status_bar_display: "default" present → Default`, neither → `Default`) — done-signal: fails.
- [x] T152 [T:GREEN] [US1] Implement `deserialize_status_bar_display_with_legacy_fallback` custom deserializer — done-signal: passes.
- [x] T153 [T:RED] [US1] `managers::settings::tests::save_writes_full_shape_drops_legacy_field` — done-signal: fails.
- [x] T154 [T:GREEN] [US1] Implement `Settings::save()` round-trip writing only the new shape — done-signal: passes.
- [x] T155 [T:RED] [US1] `managers::settings::tests::idempotent_missing_field_migration_writes_back` (FR-005) — done-signal: fails.
- [x] T156 [T:GREEN] [US1] Implement load → fill defaults → write-back idempotent path — done-signal: passes.

### managers/navigation (4 tasks)

- [x] T157 [T:RED] [US1] `managers::navigation::tests::initial_view_is_timer` and `tests::any_view_to_any_view_transition_allowed` — done-signal: fails.
- [x] T158 [T:GREEN] [US1] Implement `NavView` + `SettingsTab` enums and `Navigation::transition_to(view)` — done-signal: passes.
- [x] T159 [T:RED] [US1] `managers::navigation::tests::settings_tab_transitions_preserve_selected_tab` — done-signal: fails.
- [x] T160 [T:GREEN] [US1] Implement settings-tab nested transition — done-signal: passes.

### managers/tag (6 tasks)

- [x] T161 [T:RED] [US1] `managers::tag::tests::create_returns_new_tag_with_id` — done-signal: fails.
- [x] T162 [T:GREEN] [US1] Implement `Tag::create` calling `bridge::commands::save_tag` — done-signal: passes.
- [x] T163 [T:RED] [US1] `managers::tag::tests::delete_removes_from_list` — done-signal: fails.
- [x] T164 [T:GREEN] [US1] Implement `Tag::delete` calling `bridge::commands::delete_tag` — done-signal: passes.
- [x] T165 [T:RED] [US1] `managers::tag::tests::list_reduction_handles_loaded_set` — done-signal: fails.
- [x] T166 [T:GREEN] [US1] Implement `Tag::list_reduction` — done-signal: passes.

### managers/session (8 tasks)

- [x] T167 [T:RED] [US1] `managers::session::tests::manual_session_create_round_trips_via_bridge` — done-signal: fails.
- [x] T168 [T:GREEN] [US1] Implement `Session::create_manual` calling `bridge::commands::save_manual_sessions` — done-signal: passes.
- [x] T169 [T:RED] [US1] `managers::session::tests::manual_session_update_replaces_by_id` — done-signal: fails.
- [x] T170 [T:GREEN] [US1] Implement `Session::update_manual` — done-signal: passes.
- [x] T171 [T:RED] [US1] `managers::session::tests::manual_session_delete_removes_by_id` — done-signal: fails.
- [x] T172 [T:GREEN] [US1] Implement `Session::delete_manual` (via bulk re-save with the entry omitted, matching the deleted `delete_manual_session` JS path) — done-signal: passes.
- [x] T173 [T:RED] [US1] `managers::session::tests::list_by_date_groups_correctly` (date grouping uses `engine::date_format`) — done-signal: fails.
- [x] T174 [T:GREEN] [US1] Implement `Session::list_by_date` — done-signal: passes.

### managers/auth (8 tasks)

- [ ] T175 [T:RED] [US1] `managers::auth::tests::initial_state_guest_when_localstorage_flag_set` — done-signal: fails.
- [ ] T176 [T:GREEN] [US1] Implement `AuthState::init()` reading `presto-guest-mode` localStorage flag and `bridge::commands::supabase_get_session` — done-signal: passes.
- [ ] T177 [T:RED] [US1] `managers::auth::tests::sign_in_transition_unauthenticated_to_signed_in` — done-signal: fails.
- [ ] T178 [T:GREEN] [US1] Implement `AuthState::sign_in_with_password` transition — done-signal: passes.
- [ ] T179 [T:RED] [US1] `managers::auth::tests::sign_out_transition_signed_in_to_unauthenticated` — done-signal: fails.
- [ ] T180 [T:GREEN] [US1] Implement `AuthState::sign_out` transition — done-signal: passes.
- [ ] T181 [T:RED] [US1] `managers::auth::tests::continue_as_guest_writes_localstorage_flag` — done-signal: fails.
- [ ] T182 [T:GREEN] [US1] Implement `AuthState::continue_as_guest` writing `presto-guest-mode = "true"` — done-signal: passes.

### managers/update (4 tasks)

- [ ] T183 [T:RED] [US1] `managers::update::tests::updateinfo_no_update_default` — done-signal: fails.
- [ ] T184 [T:GREEN] [US1] Implement `UpdateInfo` enum + `Update::poll()` calling the updater plugin via bridge — done-signal: passes.
- [ ] T185 [T:RED] [US1] `managers::update::tests::polling_cadence_matches_jsbaseline` — done-signal: fails.
- [ ] T186 [T:GREEN] [US1] Implement polling-cadence loop — done-signal: passes.

### managers/team (3 tasks)

- [ ] T187 [T:RED] [US1] `managers::team::tests::demo_fixture_loads` (parity-only; small surface) — done-signal: fails.
- [ ] T188 [T:GREEN] [US1] Implement `Team::load_demo_fixture` matching `team-manager.js` parity — done-signal: passes.

**Checkpoint**: all 7 managers' state machines are implemented; `cargo test --workspace --frozen` is green.

---

## Phase 4 — Components (UI port; e2e + visual regression covered)

**Goal**: every screen rendered by Leptos; `tauri dev` shows a working app indistinguishable from the JS build to the visual regression suite. Order recommended: Timer → Tasks → History → Calendar → Tag manager → Settings tabs (8) → Auth modal → Update notification → Team.

**Test-first**: NO (covered by e2e + visual regression per Principle V).

> Per-component pattern: skeleton (component file with `view!` macro and consumed signals) → behaviour wiring (`on:click` / `on:input` etc. to managers) → integration check via `(cd src && trunk build)` and a focused e2e spec run.

### Timer view (3 tasks)

- [ ] T189 [P] [US1] Skeleton `src/src/components/timer_view.rs` consuming `TimerState` signal — done-signal: `(cd src && trunk build)` returns 0.
- [ ] T190 [US1] Wire start/pause/reset/skip buttons to `engine::timer::Timer` — done-signal: e2e `(cd tests/e2e && npx playwright test timer.spec.js)` returns 0.
- [ ] T191 [US1] Visual regression check for Timer screen — done-signal: `(cd tests/e2e && npx playwright test visual-regression.spec.js --grep timer)` returns 0; 0 baselines re-captured.

### Tasks view (3 tasks)

- [ ] T192 [P] [US1] Skeleton `src/src/components/task_list.rs` — done-signal: `(cd src && trunk build)` returns 0.
- [ ] T193 [US1] Wire task add/complete/delete to `managers::session` — done-signal: e2e `tasks.spec.js` returns 0.
- [ ] T194 [US1] Visual regression check for Tasks screen — done-signal: visual regression returns 0.

### History view (3 tasks)

- [ ] T195 [P] [US1] Skeleton `src/src/components/history.rs` consuming session-history signal — done-signal: `trunk build` returns 0.
- [ ] T196 [US1] Wire history filter + grouping — done-signal: e2e `history.spec.js` returns 0.
- [ ] T197 [US1] Visual regression check — done-signal: visual regression returns 0.

### Calendar view (3 tasks)

- [ ] T198 [P] [US1] Skeleton `src/src/components/calendar.rs` — done-signal: `trunk build` returns 0.
- [ ] T199 [US1] Wire calendar date selection to `managers::session::list_by_date` — done-signal: e2e `calendar.spec.js` returns 0.
- [ ] T200 [US1] Visual regression check — done-signal: visual regression returns 0.

### Tag manager (2 tasks)

- [ ] T201 [P] [US1] Skeleton `src/src/components/tag_manager.rs` — done-signal: `trunk build` returns 0.
- [ ] T202 [US1] Wire tag CRUD; visual regression check — done-signal: e2e `tags.spec.js` returns 0; visual regression returns 0.

### Settings tabs (8 tabs × 2 tasks = 16 tasks)

- [ ] T203 [P] [US1] Skeleton `src/src/components/settings/general.rs`; wire to `managers::settings`; visual regression check — done-signal: visual regression returns 0.
- [ ] T204 [P] [US1] Skeleton `src/src/components/settings/shortcuts.rs`; wire; visual regression — done-signal: visual regression returns 0.
- [ ] T205 [P] [US1] Skeleton `src/src/components/settings/notifications.rs`; wire; visual regression — done-signal: visual regression returns 0.
- [ ] T206 [P] [US1] Skeleton `src/src/components/settings/automation.rs`; wire; visual regression — done-signal: visual regression returns 0.
- [ ] T207 [P] [US1] Skeleton `src/src/components/settings/advanced.rs`; wire; visual regression — done-signal: visual regression returns 0.
- [ ] T208 [P] [US1] Skeleton `src/src/components/settings/goals.rs`; wire; visual regression — done-signal: visual regression returns 0.
- [ ] T209 [P] [US1] Skeleton `src/src/components/settings/theme.rs`; wire to `theme::loader`; visual regression — done-signal: visual regression returns 0.
- [ ] T210 [P] [US1] Skeleton `src/src/components/settings/updates.rs`; wire to `managers::update`; visual regression — done-signal: visual regression returns 0.

### Auth modal + update notification + team (3 components × 2 tasks = 6 tasks)

- [ ] T211 [P] [US1] Skeleton `src/src/components/auth_modal.rs`; wire to `managers::auth` (sign-in / continue-as-guest paths); e2e `auth.spec.js` returns 0 — done-signal: e2e returns 0.
- [ ] T212 [US1] Visual regression check for Auth modal — done-signal: returns 0.
- [ ] T213 [P] [US1] Skeleton `src/src/components/update_notification.rs`; wire to `managers::update` `UpdateInfo::Available` signal — done-signal: `trunk build` returns 0.
- [ ] T214 [US1] Visual regression check for Update notification — done-signal: returns 0.
- [ ] T215 [P] [US1] Skeleton + wiring for `src/src/components/team.rs` (parity-only) — done-signal: `trunk build` returns 0.
- [ ] T216 [US1] Visual regression check for Team screen — done-signal: returns 0.

### Top-level wiring + degraded-mode display

- [ ] T217 [US1] Wire `app.rs` `<App/>` root: dispatch `NavView` over the components above; subscribe to global-shortcut events; mount `bridge::storage::migrate_legacy_localstorage()` on startup — done-signal: `cargo tauri dev` launches; `(cd tests/e2e && npx playwright test)` (full e2e suite) returns 0.
- [ ] T218 [US1] Render degraded-mode UI when `BridgeAvailable::Absent` (Phase 4 ratifies the Phase 1G short-circuit at the visual level) — done-signal: pure-Trunk-dev-server load (`(cd src && trunk serve)`) renders without panic.

**Checkpoint**: full UI rendered in Leptos; full e2e suite + visual regression suite pass against the Leptos build with 0 baselines re-captured.

---

## Phase 5 — Theme system + assets

**Goal**: theme code-gen replaces `build-themes.js`; remixicon assets vendored; visual regression unbroken.
**Test-first**: `tools/build-themes` YES (snapshot-style); `theme/loader` covered by e2e.

- [ ] T219 [T:RED] [US1] `tools/build-themes/src/main.rs::tests::generates_themes_rs_snapshot` — given a fixture `art/themes/foo.css` with a metadata header, asserts the emitted `themes.rs` contains the expected enum variant — done-signal: `cargo test -p presto-build-themes` exits non-zero.
- [ ] T220 [T:GREEN] [US1] Implement `tools/build-themes/src/main.rs` reading `src/style/themes/*.css` and emitting `src/src/theme/themes.rs` — done-signal: test passes; `cargo run -p presto-build-themes` emits a non-empty `themes.rs`.
- [ ] T221 [P] [US1] Relocate CSS source from current `src/styles/` to `src/style/` (preserving `themes/` subdirectory contents per FR-022; only the directory location moves) — done-signal: `(cd src && trunk build)` returns 0.
- [ ] T222 [P] [US1] Vendor `remixicon` font + CSS into `src/assets/icons/` (copy `node_modules/remixicon/fonts/` and the CSS file) — done-signal: `(cd src && trunk build)` returns 0; `src/dist/assets/icons/` populated.
- [ ] T223 [US1] Implement `src/src/theme/loader.rs` applying themes via `document.documentElement` class + persistence via `bridge::commands::save_settings` — done-signal: `(cd tests/e2e && npx playwright test --grep theme)` returns 0.
- [ ] T224 [US1] Wire follow-system-theme detection via `prefers-color-scheme` media query — done-signal: e2e `theme.spec.js` returns 0; visual regression returns 0.
- [ ] T225 [US1] Run full visual regression suite as Phase 5 gate — done-signal: `(cd tests/e2e && npx playwright test visual-regression.spec.js)` returns 0; 0 baselines re-captured.

**Checkpoint**: theme system fully Rust-driven; visual regression still passes.

---

## Phase 6 — Cleanup (cutover commit)

**Goal**: delete the JS toolchain, the unused Tauri commands, and the redundant `write_excel_file`. Visual regression is the final gate.
**Test-first**: N/A (deletion).

### Delete unused Tauri commands (10 tasks; per contracts/tauri-bridge.md §Deletions)

- [ ] T226 [P] [US2] Delete `save_manual_session` from `src-tauri/src/lib.rs` (handler + helper if any) — done-signal: `cargo build --workspace --frozen` returns 0.
- [ ] T227 [P] [US2] Delete `delete_manual_session` from `src-tauri/src/lib.rs` — done-signal: build returns 0.
- [ ] T228 [P] [US2] Delete `get_manual_sessions_for_date` from `src-tauri/src/lib.rs` — done-signal: build returns 0.
- [ ] T229 [P] [US2] Delete `save_tags` (bulk-write helper) from `src-tauri/src/lib.rs` — done-signal: build returns 0.
- [ ] T230 [P] [US2] Delete `load_session_tags` from `src-tauri/src/lib.rs` — done-signal: build returns 0.
- [ ] T231 [P] [US2] Delete `save_session_tags` (bulk-write helper) from `src-tauri/src/lib.rs` — done-signal: build returns 0.
- [ ] T232 [P] [US2] Delete `unregister_global_shortcuts` from `src-tauri/src/lib.rs` — done-signal: build returns 0.
- [ ] T233 [P] [US2] Delete `show_window` from `src-tauri/src/lib.rs` — done-signal: build returns 0.
- [ ] T234 [P] [US2] Delete `set_dock_visibility` and `set_status_bar_visibility` (both macOS-only, superseded by `status_bar_display`) — done-signal: build returns 0.

### Delete redundant `write_excel_file` (now superseded by `export_sessions_xlsx`)

- [ ] T235 [US2] Delete `write_excel_file` Tauri handler + Leptos wrapper + mock entry; remove dead helpers — done-signal: `cargo build --workspace --frozen` returns 0; `tauriMock.js` no longer references `write_excel_file`.

### Delete dead helpers

- [ ] T236 [US2] Run `cargo +nightly udeps`-style scan on `src-tauri/src/helpers.rs` and delete any function left dead by T226–T235 — done-signal: `cargo build --workspace --frozen` returns 0; `cargo clippy --workspace -- -D warnings` is clean (no `#[allow(dead_code)]` survivors).

### Delete root JS toolchain (per FR-015 / SC-008)

- [ ] T237 [US2] Delete `package.json`, `package-lock.json`, `node_modules/`, `vite.config.js`, `vitest.config.js`, `eslint.config.js`, `tsconfig.json`, `src/globals.d.ts`, `build-themes.js` from repo root — done-signal: `git ls-files | grep -E '^(package(-lock)?\\.json|vite\\.config\\.js|vitest\\.config\\.js|eslint\\.config\\.js|tsconfig\\.json|build-themes\\.js)$'` is empty; `git ls-files src/globals.d.ts` is empty.

### Delete JS sources

- [ ] T238 [US2] Delete `src/main.js`, `src/managers/*.js`, `src/core/*.js`, `src/utils/*.js`, `src/config/storage-keys.js` — done-signal: `git ls-files src/ | grep -E '\\.js$'` is empty (only `.rs`, `.html`, `.css` remain).

### Delete Vitest specs (per FR-016)

- [ ] T239 [US2] Delete `tests/core/`, `tests/managers/`, `tests/utils/` Vitest spec directories — done-signal: `git ls-files tests/ | grep -E '\\.test\\.js$|^tests/(core|managers|utils)/'` is empty.

### Final cutover gates

- [ ] T240 [US2] Run `cargo build --workspace --frozen` and `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` — done-signal: both exit 0; 0 warnings (SC-006).
- [ ] T241 [US1] Run full e2e suite — done-signal: `(cd tests/e2e && npm ci && npx playwright test)` returns 0 (SC-002).
- [ ] T242 [US1] Run visual regression suite as the **final cutover gate** — done-signal: `(cd tests/e2e && npx playwright test visual-regression.spec.js)` returns 0; 0 baselines re-captured (SC-001, SC-003).
- [ ] T243 [US1] Run quickstart.md commands end-to-end — done-signal: every command in `specs/001-leptos-migration/quickstart.md` exits 0.

**Checkpoint**: cutover commit is a deletion-only commit; all gates green.

---

## Phase 7 — CI hardening

**Goal**: `baseline-cap` CI gate wired and dry-run-tested; engine `web-sys` grep gate wired and dry-run-tested.

- [x] T244 [US4] Wire `baseline-cap` stage from plan.md §CI gates into `.agentex.yml` (or `.github/workflows/ci.yml` per repo convention); the stage greps for changed `tests/e2e/__screenshots__/visual-regression/*.png` files in the PR diff and fails at >2 — done-signal: throwaway-branch run on a PR with 0 changed baselines exits 0.
- [ ] T245 [US4] Verify `baseline-cap` fail-closed by intentionally re-capturing 3 baselines on a throwaway branch — done-signal: CI exits 1 with the documented error message; throwaway branch is then deleted.
- [ ] T246 [US1] Wire engine `web-sys` grep gate from plan.md §CI gates into `.agentex.yml` `qa.lint` — done-signal: clean HEAD's lint stage exits 0.
- [ ] T247 [US1] Verify engine `web-sys` grep gate fail-closed by introducing a temporary `web_sys::` reference under `src/src/engine/` on a throwaway branch — done-signal: lint stage exits 1 with the documented error message; throwaway branch deleted.
- [ ] T248 [US2] Update pre-commit hook (husky-equivalent) to scan both `Cargo.lock` (vs. workspace `Cargo.toml`) and `tests/e2e/package-lock.json` (vs. `tests/e2e/package.json`) for drift per Principle IX — done-signal: a deliberate `Cargo.toml`-without-`Cargo.lock` change in a throwaway branch is rejected by the hook.

**Checkpoint**: every CI guard documented in plan.md §CI gates is live and fail-closed-verified.

---

## Dependencies & execution order

### Phase Dependencies

- **Phase 0**: no deps; can start immediately.
- **Phase 0.5**: depends on Phase 0 (workspace) but is otherwise self-contained; foundational for Phase 1 (mock-first rule becomes meaningful only when the mock matches the handler set).
- **Phase 1A (BridgeError)**: blocks all other Phase 1 work (every wrapper signature uses `BridgeError`).
- **Phase 1B (BridgeAvailable)**: parallel to 1A; blocks 1G.
- **Phase 1C–1F**: depend on 1A; 1C wrappers are parallel by domain.
- **Phase 1G (`BridgeAvailable` wiring)**: depends on 1B + 1C.
- **Phase 2 (engine)**: depends on Phase 1 (some `bridge::events::listen_*` is needed for the activity signal source, but the engine itself only consumes `ActivitySignal`, which is constructible in tests).
- **Phase 3 (managers)**: depends on Phase 1 (every manager calls bridge wrappers); recommended in-phase order `settings → navigation → tag → session → auth → update → team`.
- **Phase 4 (components)**: depends on Phase 3 (components consume manager signals).
- **Phase 5 (theme + assets)**: depends on Phase 4 (settings/theme tab is wired in Phase 4 but uses `theme::loader` from Phase 5).
- **Phase 6 (cleanup)**: depends on Phase 5 + visual regression green; this is the cutover commit.
- **Phase 7 (CI hardening)**: depends on Phase 6.

### Within each Principle V phase

- T:RED MUST land in a commit before T:GREEN; squash-merge of the cutover PR is allowed but the unsquashed sequence MUST appear in the PR's commit log (per AGENTS.md §Test-first commit ordering).
- A single combined RED+GREEN commit is rejected.

### Parallel opportunities

- Phase 0.5: T009–T020 are all `[P]` (different `case` blocks in the same file but no semantic overlap; the agent batches them in a single edit).
- Phase 1C: each per-command pair (T032/T033, T034/T035, …) is independent of every other pair — parallelisable across sub-batches by domain.
- Phase 4 components: every skeleton task is `[P]`; per-component skeleton/wire/visual sequence is sequential within the component but parallel across components.
- Phase 6 deletions: T226–T234 are all `[P]` (same file, no semantic overlap; one agent does them in a single edit).

---

## Coverage matrix — Functional Requirements

| FR | Covering tasks |
|---|---|
| FR-001 Engine bit-for-bit parity | T120–T145 |
| FR-002 Engine pure / normalised input | T130–T135, T146 |
| FR-003 14 baselines pass, no regen | T225, T242 |
| FR-004 17 e2e specs pass | T241, T243 |
| FR-005 Local data readable, idempotent migration | T099–T115, T147–T156 |
| FR-006 Guest-mode flags persist | T110–T111, T175–T176, T181–T182 |
| FR-007 Auto-updater path | T243 + plan.md §Pre-release validation checklist |
| FR-008 Compile-time mismatch | T023–T027, T032–T119, T240 |
| FR-009 BridgeAvailable short-circuit | T030–T031, T118–T119, T218 |
| FR-010 tauriMock mirror; mock-first | T009–T021, T084, T087, T096, T099 |
| FR-011 IPC invoke+listen only | T116–T117 events; T032–T119 commands |
| FR-012 clippy pedantic | T002, T240 |
| FR-013 Closed domains as enums | T024 (BridgeError), T158 (NavView/SettingsTab), T175 (AuthState), T151 (StatusBarDisplay), T027 (TimerMode), T028–T029 (SessionType); FR-013's "sound notification variant" wording dropped per spec.md amendment (the on-disk shape is `notifications.sound_notifications: bool` — a single toggle, not a variant set) |
| FR-014 RED-first scope | every `[T:RED]`: T023, T030, T032–T144 (every odd in 1C/D/E), T147–T187 (odd), T219 |
| FR-015 Repo-root JS toolchain deleted | T237 |
| FR-016 Vitest specs deleted | T239 |
| FR-017 No new user-facing features | (negative; ratified) |
| FR-018 Aptabase + Supabase preserved | T086, T088–T095 |
| FR-019 No mobile / AI / multi-user | (negative; ratified) |
| FR-020 All 5 global shortcuts | T063, T116–T117, T217 |
| FR-021 Theme contract preserved | T219–T225 |
| FR-022 `art/` not modified beyond mechanical | T221 (relocates path only) |
| FR-023 Resolved at plan-time | plan.md §Project Structure |

## Coverage matrix — Success Criteria

| SC | Covering tasks |
|---|---|
| SC-001 14 baselines pass within tolerance | T225, T242 |
| SC-002 17 e2e specs pass | T241 |
| SC-003 0 baseline regenerations | T242, T244–T245 |
| SC-004 All Tauri commands type-checked | T023–T027, T032–T119, T240 |
| SC-005 100% local data preserved | T099–T115, T147–T156, T243 |
| SC-006 0 clippy pedantic warnings | T240 |
| SC-007 Engine RED-first ordering | T120–T145 |
| SC-008 Repo-root JS toolchain deleted | T237, T239 |
| SC-009 All 5 global shortcuts work | T063, T217 |
| SC-010 Mock-first for follow-up commands | T021 (drift gate) |
| SC-011 First-launch migration <2s | T114–T115; benchmark at T243 |

---

## Implementation strategy

Hard cutover, single PR. MVP is the entire feature — US1 + US2 are mutually-blocking acceptance gates. Per `/manage-feature` step 12, the implementation phase batches across ~12–15 subagents at ~15–20 tasks each. Phase-boundary gates: Principle V batches require `[T:RED]` precedes `[T:GREEN]` in `git log --reverse`; Phase 6 requires visual regression returns 0 with 0 baselines re-captured; Phase 7 requires CI gates verified fail-closed on a throwaway branch.

The per-PR commit log MUST show RED-first ordering for every Principle V scope file (per AGENTS.md §Test-first commit ordering); squash-merge of the cutover PR is permitted, but the unsquashed sequence is the audit surface.

## Notes

- `[P]` = different files, no incomplete-task dependencies.
- `[T:RED]` / `[T:GREEN]` markers cited in plan.md §"Audit method for test-first ordering"; sample-based audit (10% of commits per phase).
- US labels (US1/US2/US3/US4) for traceability; migration judged against all four jointly.
- A `[T:RED]` description must name the test and the behaviour it asserts (`<test name>` + `<one-line behaviour>`); a vaguer formulation is rejected at superb-review.
- 0 visual regression baselines re-captured in the cutover PR. Re-capture >2 escalates per Principle IV.
