# Research: Leptos Frontend Migration

**Phase**: 0 (Outline & Research)
**Feeds**: [plan.md](./plan.md), [data-model.md](./data-model.md), [contracts/tauri-bridge.md](./contracts/tauri-bridge.md), [quickstart.md](./quickstart.md)

This file resolves every `[NEEDS CLARIFICATION]` and `[BEST-GUESS PM DECISION]` marker carried forward from spec.md, plus the cross-cutting tooling decisions (Trunk vs. cargo-leptos; workspace conversion). Each section uses the Decision / Rationale / Alternatives format the spec-kit template prescribes.

---

## 1. Leptos crate location (resolves spec FR-023)

**Decision**: Repurpose `src/` as the Leptos crate root in a Cargo workspace. The directory layout is:

```text
Cargo.toml                # NEW: workspace root with members = ["src", "src-tauri", "tools/build-themes"]
src/                      # Leptos crate (workspace member); [package].name = "presto-web"
├── Cargo.toml            # NEW: Leptos crate manifest, [package].name = "presto-web"
├── Trunk.toml            # NEW: Trunk config; dist-dir is the default ("dist", resolves to src/dist/)
├── index.html            # EXISTS today; Trunk's entry HTML
├── style/                # CSS source, relocated from current src/styles/
├── assets/               # Static (icon font + brand)
├── public/               # If Trunk needs it for static-copy assets
└── src/                  # Rust source (main.rs, app.rs, modules)
src-tauri/                # UNCHANGED tree; [package].name = "presto" (bin) / "presto_lib" (lib) preserved
tools/build-themes/       # NEW: workspace member for theme code-gen binary; [package].name = "presto-build-themes"
tests/                    # E2E + visual regression unchanged
```

**Crate naming**: the Leptos crate is `presto-web` (avoids collision with `presto` the Tauri bin and `presto_lib` the Tauri lib). The theme code-gen binary is `presto-build-themes`. Workspace `[workspace.members] = ["src", "src-tauri", "tools/build-themes"]`.

**`Trunk.toml`**: `dist-dir` is left at Trunk's default — `dist/` next to `Trunk.toml` ⇒ resolves to `src/dist/`. No override needed; we cite this here so the `tauri.conf.json` paths below are unambiguous.

**`tauri.conf.json` post-migration changes** — the previous "lowest churn" framing was about *layout familiarity* (the served entry continues to live under `src/`), not about the config staying byte-identical. The config necessarily changes for any build-tool swap. Concretely:

| Field | Today (Vite) | Post-migration (Trunk) |
|---|---|---|
| `frontendDist` | `"../src"` | `"../src/dist"` |
| `beforeDevCommand` | empty (or vite) | `"cd src && trunk serve --port 1420"` |
| `beforeBuildCommand` | empty (or vite build) | `"cd src && trunk build --release"` |
| `devUrl` | `"http://127.0.0.1:1420"` | `"http://127.0.0.1:1420"` (unchanged — Trunk dev server pinned to 1420) |

**Rationale**: Lowest churn for `playwright.config.js`'s `baseURL: "http://127.0.0.1:1420"` (unchanged — we pin Trunk's dev port to match Vite's). The `src/src/` Rust-source nesting is awkward but is the standard Tauri+Leptos convention (Trunk reads `src/index.html`, the Rust source must live in `src/src/` to satisfy Cargo's default `[lib]`/`[bin]` layout). Renaming to a sibling `web/` or a workspace member `crates/web/` would force additional changes to `playwright.config.js`, the visual-regression-baseline path expectations, and the e2e fixtures — beyond the `tauri.conf.json` edits already required by the build-tool swap, which is unavoidable.

**Alternatives considered**:
- **Sibling `web/` directory**: cleaner separation from `src-tauri/`. Rejected: forces additional edits to `playwright.config.js`, the e2e setup scripts, and the asset path in `index.html` — beyond the `tauri.conf.json` edits already required by the build-tool swap.
- **Workspace member at `crates/web/`**: most idiomatic for a multi-crate Cargo repo. Rejected for the same reason as above plus higher learning curve for new contributors who already know `src-tauri/` is the backend.

**Constitutional anchors**: VII (No Upstream Compatibility Burden) — we can rename freely; choice is operational, not principled. IX (Lock Files Are First-Class) — workspace `Cargo.lock` is single source of truth post-cutover.

---

## 2. Build tool — Trunk vs. cargo-leptos

**Decision**: **Trunk**. Tauri's official Leptos integration recommends Trunk for CSR-only desktop apps; cargo-leptos is the right choice when you also need server functions or SSR.

**Rationale**: presto is a single-window CSR-only desktop app with no server side, no streaming, and no fullstack story. cargo-leptos's value-add is its server-fn machinery, SSR pre-rendering, and hot-reload of both client and server bundles in lockstep — none of which we use. Trunk gives us the smaller, simpler subset: WASM build + asset pipeline + dev server + pre-build hooks (which we use for the theme code-gen). Less surface area, less to learn, fewer failure modes.

**Trade-off detail (the 1–2 paragraphs requested)**:

cargo-leptos is the right tool when (a) you need server functions to call Rust on a server from a Leptos component without writing a separate API contract, (b) you want SSR or hydration with a streaming HTML pre-render, or (c) you want one tool to orchestrate a multi-binary client+server build. Cost: a tighter coupling to Leptos's view-engine internals, a bigger learning curve for contributors who haven't seen the cargo-leptos build model, and friction integrating a non-Leptos build tool (in our case, the Tauri host).

Trunk is the right tool when (a) you have a single CSR WASM frontend, (b) you want a build experience close to Vite's mental model (entry HTML + asset pipeline + dev server), and (c) the frontend talks to a Rust host (Tauri) via the host's IPC, not via Leptos server functions. Cost: server-side Leptos features are unavailable, but we don't use them. presto matches Trunk's sweet spot.

**Alternatives considered**:
- **cargo-leptos**: rejected as above.
- **wasm-pack + custom orchestrator**: rejected — Trunk already provides the asset pipeline and dev server we'd otherwise hand-roll.

**Constitutional anchors**: III (Type Safety Over Defensive Code) — Trunk doesn't add a new defensive layer; the type-safety story stays in Rust+Leptos itself. VIII (Spec-Driven Feature Flow) — the choice is documented here so it's not litigated downstream.

---

## 3. Workspace conversion

**Decision**: Convert today's single-crate-at-`src-tauri/` repo to a Cargo workspace with three members: `src/` (Leptos), `src-tauri/` (existing), and `tools/build-themes/` (theme code-gen binary).

**Rationale**: A workspace gives us a single `Cargo.lock`, a single `cargo build --workspace --frozen` for the test stage, and clean dependency sharing between members (e.g., the `tools/build-themes` binary may depend on the same `serde` version as the Leptos crate).

**Conversion steps** (executed in Phase 0 of the implementation phasing):

1. Add a workspace `Cargo.toml` at the repo root with `[workspace]` declaring members `["src", "src-tauri", "tools/build-themes"]`.
2. Create `src/Cargo.toml` for the Leptos crate (initially empty / hello-world).
3. Create `tools/build-themes/Cargo.toml` for the code-gen binary.
4. Move `src-tauri/Cargo.lock` to repo root as `Cargo.lock`. Run `cargo build --workspace --frozen` to verify the transition is clean.
5. Update `.gitignore` to ensure only the workspace-root `Cargo.lock` is tracked (and any per-member `target/` is ignored).

**Backward compatibility**: `src-tauri/Cargo.toml` stays a workspace member; its existing `[package]`, `[lib]`, `[dependencies]` blocks are unchanged. The only delta is that workspace-level lints + dependency-version resolution now apply.

**Alternatives considered**:
- **Keep two separate crates with separate `Cargo.lock`s**: rejected — duplicates lockfile discipline (Principle IX), doubles CI cache invalidation, and complicates the new `tools/build-themes` crate's home.

**Constitutional anchors**: IX (Lock Files Are First-Class) — workspace consolidates to one `Cargo.lock`. III — workspace-level `[lints]` table propagates the pedantic posture to the Leptos crate.

---

## 4. Playwright install path (resolves spec edge case "Existing pinned playwright install path")

**Decision**: Scoped `tests/e2e/package.json` pinning **only** `@playwright/test`. CI runs `(cd tests/e2e && npm ci && npx playwright install --with-deps chromium && npx playwright test)`. The repo root has no `package.json`, no `node_modules/`. The lockfile principle (IX) survives — `tests/e2e/package-lock.json` is committed and authoritative for that scope.

**Rationale**: Cargo cannot run a chromium browser. Playwright's e2e + visual regression suite is the cutover gate (SC-001 / SC-002 / Principle IV) and there is no Rust replacement. The smallest possible JS surface area post-cutover is a single scoped `package.json` in `tests/e2e/` with one dependency. Everything else (theme code-gen, type-checking, formatting, linting) is replaced by Rust tooling.

**Alternatives considered**:
- **Keep root `package.json` with only `@playwright/test`**: rejected — leaves a misleading "this is a JS project" signal at the repo root and a `node_modules/` near the Rust workspace.
- **Run Playwright via a Tauri command or Rust binary**: rejected — Playwright has no Rust binding; running it via `Command::new("npx")` from Rust is identical to `cd tests/e2e && npx ...` but more code.
- **Drop e2e + visual regression entirely**: rejected — that defeats Principle IV (the green-light gate).

**Constitutional anchors**: IX — the surviving scoped lockfile is "first-class" within its scope. IV — preserves the gate.

---

## 5. Aptabase analytics SDK replacement (resolves spec edge case)

**Decision**: A thin Rust-side adapter via `tauri-plugin-aptabase` (already in `src-tauri/Cargo.toml:59`). New Tauri command `track_event(name: String, props: Option<HashMap<String, Value>>)` checked against `settings.analytics_enabled` server-side using the existing `are_analytics_enabled` helper at `src-tauri/src/lib.rs:141`. Leptos invokes via `invoke()`.

**Rationale**: We already use the Rust plugin; the JS shim `@aptabase/tauri` is just a wrapper that forwards to it via `invoke()`. Removing the shim and exposing the wrapped action as a first-class command (`track_event`) is no functional change at the Rust call sites that already call `app.track_event(...)` (e.g., `lib.rs:369`, `lib.rs:394`, `lib.rs:640`); it just exposes one new entry point for Leptos. The opt-in toggle lives at the Rust call site and is never bypassed.

**Alternatives considered**:
- **Re-export the JS shim from the WASM bundle**: rejected — drags JS interop overhead into every analytics call and double-checks the opt-in (once on the Rust side, once on the JS side).
- **Leptos calls Aptabase HTTP API directly**: rejected — bypasses the plugin, requires duplicating JWT/auth setup, and creates a second analytics code path to maintain.

**Constitutional anchors**: II (Local-First, Privacy-Default) — opt-in checked Rust-side, never bypassed. VI (The Tauri Boundary Is Stable) — single new command, documented in contracts/.

---

## 6. Supabase auth SDK replacement (resolves spec edge case)

**Decision**: A Rust-side adapter using direct REST + JWT in a thin module under `src-tauri/src/auth/` (or alongside `helpers.rs`). Leptos calls Tauri commands `supabase_sign_in_with_password`, `supabase_sign_out`, `supabase_get_session`, `supabase_refresh_session`. The JS Supabase SDK is removed entirely. A one-shot transition migration moves any in-flight JS-era Supabase token from `localStorage` into the Rust-managed app-data store on first post-cutover launch.

**Rationale**: Avoids dragging supabase-js (and its websocket realtime client) into a WASM bundle. The Tauri host already has HTTP capability; using the plugin's `tauri::http` (or a small `reqwest` dep) for the four auth REST endpoints is straightforward. Token storage uses the app-data directory (Rust-managed, on-disk JSON). Guest mode — first-class per Principle II — is unaffected because it's a localStorage flag, not a Supabase concept.

**Auth surface kept narrow**:
- `supabase_sign_in_with_password(email, password) → Result<AuthSession, BridgeError>`
- `supabase_sign_out(refresh_token) → Result<(), BridgeError>`
- `supabase_get_session() → Result<Option<AuthSession>, BridgeError>`
- `supabase_refresh_session(refresh_token) → Result<AuthSession, BridgeError>`

The OAuth flow keeps the existing `start_oauth_server` Tauri command (returns the OAuth port; unchanged from today).

### One-shot Supabase token migration (cutover-PR scope)

The existing JS SDK persists the active session at `window.localStorage.getItem("sb-<project-ref>-auth-token")` as a JSON object containing `access_token`, `refresh_token`, `expires_at`, and `user`. The cutover build cannot rely on the user re-authenticating — that would break FR-007's auto-updater promise for any signed-in user.

End-to-end transition design:

1. On first launch of the post-cutover build, `bridge::storage::migrate_legacy_localstorage()` runs from `app.rs` startup.
2. For the Supabase slice: it reads `window.localStorage.getItem("sb-<project-ref>-auth-token")` via `web-sys::window().local_storage()`. If absent, short-circuit.
3. If present, parse into `SupabaseSessionPayload` (see [data-model.md](./data-model.md)).
4. Idempotency gate: short-circuit and clear the localStorage key if the Rust-side store already has a session (`supabase_get_session` returns `Some`).
5. Otherwise, hand the payload to the new Tauri command `import_legacy_supabase_session(payload: SupabaseSessionPayload) -> Result<(), BridgeError>`. The Rust-side adapter validates the payload (well-formed JWT? `expires_at` parses?) and persists it to the app-data dir using the same shape `supabase_get_session` returns.
6. On successful persist, clear the localStorage key (best-effort; leave it on failure so a follow-up launch can retry).

Steady state: one minor version after cutover, the migration code path is dead-on-arrival (any user who has launched the cutover build has already had their token migrated). The migration entry point and the `import_legacy_supabase_session` command are removed in a follow-up cleanup release. Coverage during the cutover-PR window: a `wasm-bindgen-test` (`managers/auth.rs::tests::imports_legacy_supabase_session_from_localstorage`) exercises the migration path with a mocked `localStorage` and asserts the Rust-side adapter receives the expected payload.

**Alternatives considered**:
- **Rust supabase-rs official SDK**: rejected — limited coverage of auth flows (per spec edge case); REST + JWT is more direct and doesn't add a heavy dependency for our narrow surface.
- **Keep supabase-js as a JS shim under WASM**: rejected — defeats the point of the migration and creates two SDKs to maintain.
- **Force re-authentication on first post-cutover launch**: rejected — breaks FR-007's auto-updater promise for signed-in users.

**Constitutional anchors**: II — guest mode preserved; auth is opt-in. VI — JS↔Rust boundary stays narrow (4 permanent commands + 1 transition-only command, the latter with a defined sunset).

---

## 7. Theme code-gen path (resolves spec edge case "Theme code-gen path")

**Decision**: A Trunk pre-build hook (`[[hooks]]` in `Trunk.toml`) that runs the binary at `tools/build-themes/`. The binary scans `src/style/themes/*.css`, parses out the metadata header from each, and emits `src/src/theme/themes.rs` enumerating discovered themes as a Rust module. This replaces `build-themes.js`.

**Rationale**: Same source-of-truth contract (CSS files in `src/style/themes/`), Rust-only toolchain. The current `build-themes.js` parses CSS file headers to extract theme name + display label; the Rust port does the same. `tools/build-themes/` is a workspace member so it runs under `cargo build --workspace --frozen` for free. Trunk's `[[hooks]]` runs it before every build, matching today's `predev`/`prebuild` npm-script behaviour.

**Alternatives considered**:
- **`build.rs` in the Leptos crate**: rejected — `build.rs` runs on every `cargo build`, including incremental builds where themes haven't changed; Trunk's pre-build hook scopes correctly to the Trunk pipeline.
- **Hand-write `themes.rs` once and remove the code-gen**: rejected — defeats FR-021's "drop a CSS file in `art/`, get a selectable theme" contract.
- **Procedural macro**: rejected — overkill; debug-loops on macros are slow.

**Constitutional anchors**: I — theme code-gen is not engine-adjacent; III — generated Rust is type-checked. VIII — change is documented.

---

## 8. `xlsx` replacement for export (resolves spec edge case)

**Decision**: Replace JS `xlsx` with `rust_xlsxwriter` (write-only, lean — sufficient for export). Wrap in a Tauri command `export_sessions_xlsx(path: String, sessions: Vec<ExportSession>) → Result<(), BridgeError>`. JS `xlsx` package removed.

**Rationale**: We only ever write `.xlsx` files (export); we never read them. `rust_xlsxwriter` is write-only, well-maintained, and several MB lighter than the read+write alternative `umya-spreadsheet`. The existing `write_excel_file` command (`lib.rs:1102`) currently takes a base64-encoded blob from the JS `xlsx` library and writes it to disk; under the new design, the Tauri command builds the workbook itself and writes it directly. Same user-visible behaviour; less data crossing the bridge.

**Alternatives considered**:
- **`umya-spreadsheet`** (read+write): rejected — heavier; we don't need read.
- **Keep `write_excel_file` and have the Leptos crate compute the bytes via `rust_xlsxwriter` then base64-encode them for the bridge**: rejected — pointless serialisation.
- **Drop the export feature**: out-of-scope-removal; would need a separate spec.

**Constitutional anchors**: VI — single new command. VIII — change documented.

---

## 9. `remixicon` icon font (resolves spec edge case)

**Decision**: Vendor the `remixicon` font + CSS files into `src/assets/icons/`. Trunk's asset pipeline copies them. The existing icon class names (e.g., `ri-briefcase-line`, `ri-brain-line`) stay, so e2e selectors keep working.

**Rationale**: `remixicon` is shipped as static font + CSS today; there's no JS code path that depends on the npm package. Vendoring is a copy-paste of the static assets out of `node_modules/remixicon/fonts/` and the CSS file into `src/assets/icons/`. No code-level dependency.

**Alternatives considered**:
- **Reference via `npm` + Vite-served static**: not applicable post-cutover (`node_modules/` is deleted).
- **Use a Rust icon crate or inline SVGs**: rejected — would force re-authoring every UI site that uses `<i class="ri-...">`, multiplying the e2e selector churn.

**Constitutional anchors**: IV — preserves visual regression by not changing rendering. VII — vendoring is fine; we're not maintaining upstream compatibility.

---

## 10. Activity-monitoring + smart-pause (resolves spec edge case)

**Decision**: Same architecture as today — Rust-side `ActivityMonitor` in `src-tauri/src/lib.rs:30-308` emits Tauri events (`user-activity`, `user-inactivity` on macOS; DOM-based fallback on Linux/Windows). Leptos subscribes via `bridge::events::listen("user-activity", …)` and `listen("user-inactivity", …)`. The `engine/activity_signal.rs` module folds these (plus DOM `mousemove`/`keydown`/`visibilitychange` events read via `web-sys` listeners feeding into the same signal) into a single `ActivitySignal` stream consumed by `engine/timer.rs`.

**Rationale**: The macOS code path (`Self::get_system_idle_time` via CGEventSource) is the only platform-specific bit; on other platforms the JS frontend currently hooks DOM events. Post-migration the listeners are Leptos-side `web-sys` listeners feeding the same normalised signal — but **the engine never reads from the DOM** per Principle I; the listeners are a bridge concern that produces a typed `ActivitySignal` enum.

**Alternatives considered**:
- **Move all activity detection Rust-side**: rejected — DOM `mousemove`/`keydown`/`visibilitychange` are observable only from the WebView (where the user is actually clicking), not from the Tauri host process; the JS-side hook is already where it needs to be.
- **Drop smart-pause**: out-of-scope-removal.

**Constitutional anchors**: I — engine remains pure; bridge produces normalised signal. V — `ActivitySignal` reduction is test-first.

---

## 11. Sub-pixel rendering drift policy (resolves spec edge case "Sub-pixel rendering drift")

**Decision**: The 2% pixel-ratio tolerance in `playwright.config.js` is a **CI-flake budget** for cross-platform AA noise (e.g., font hinting on chromium-linux), not a drift allowance for migration changes. The cutover's expected baseline diff is 0% — same fonts, same DOM structure, same CSS, served by the same WebView. **Re-captures during the cutover PR are 0 by default; up to 2 with explicit per-PR justification.** A baseline that fails the visual-regression suite is either a regression (fix the code) or a doc-flagged intentional change (re-capture + one-line PR note + reviewer ratification). Re-capturing >2 baselines escalates to a constitution-amendment-or-spec-revision discussion.

**Rationale**: Conflating the tolerance and the re-capture allowance turns the safety net into a soft-cap. Distinguishing them keeps the gate sharp: tolerance absorbs platform-dependent AA noise that's not a code change, while the re-capture ceiling enforces that the migration is a no-op visually.

**Alternatives considered**:
- **Loosen tolerance to 5%**: rejected — that's a constitution amendment per Principle IV's tolerance rule, not a per-feature decision.
- **Re-capture all 14**: rejected — defeats Principle IV.

**Constitutional anchors**: IV — explicit per-baseline ceiling preserves the contract.

---

## 12. Failure-mode rollback (resolves spec edge case "Failure-mode rollback")

**Decision**: Hard cutover, no dual-build coexistence. If the merged build is broken in a way that escapes CI + the visual regression suite, recovery is a normal-channel patch release through the Tauri auto-updater. No "incremental rollback" — that's what the spec/plan/visual-regression discipline is FOR.

**Rationale**: Per VISION.md and the feature brief, this is a hard cutover. A dual-build ("old frontend + new frontend behind a flag") would (a) double the CI matrix, (b) require keeping two `tauriMock.js` shapes, (c) create two failure modes a user might be on. The visual regression suite + e2e suite are the gates; if they pass, the migration is judged successful. If a regression escapes both, the response is the same as any other regression: patch release.

**Alternatives considered**:
- **Dual-build behind a feature flag**: rejected as above.
- **Beta channel for the cutover release**: not in scope of this feature; the existing release-channel discipline (semver patch via `tauri-plugin-updater`) is sufficient.

**Constitutional anchors**: VII (No Upstream Compatibility Burden) — we owe current users a working app, not a rollback button. IV — visual regression is the gate that prevents this from happening.

---

## 13. Error handling — typed `BridgeError` enum

**Decision**: Replace today's `Result<T, String>` shape on every Tauri command with `Result<T, BridgeError>`, where `BridgeError` is a serde-tagged Rust enum with variants `BridgeUnavailable`, `NotAuthenticated`, `InvalidArgument { field, reason }`, `NotFound { resource }`, `SerdeRoundtrip { command, error }`, and `Internal { msg }`. The full type definition lives in [data-model.md](./data-model.md) §`BridgeError`; the per-command error column in [contracts/tauri-bridge.md](./contracts/tauri-bridge.md) reflects the typed shape.

**Rationale**: Spec FR-008 promises that a Leptos call site mismatching the Rust handler's signature is a compile error. A `String` error channel can't deliver that — every error becomes a free-form string that the call site must parse to discriminate. A typed enum with externally-tagged JSON makes the discrimination structural, the way FR-008 implies. This is a scope addition vs. the original plan, which marked typed errors out of scope; the rationale is that FR-008 is load-bearing and the original deferral effectively kicked the type-safety story out of this feature, undermining the motivating principle.

**Serde shape**: externally-tagged (`#[serde(tag = "kind", rename_all = "snake_case")]`). Wire example: `{"kind":"invalid_argument","field":"email","reason":"empty"}`. The simplest cross-language shape — every error JSON object carries a discriminator and the variant fields alongside.

**Mapping strategy** (Tauri-side, applied in the cutover commit, mechanical search-and-replace in `src-tauri/src/lib.rs`): every `.map_err(|e| e.to_string())` call site is rewritten to map into a `BridgeError` variant. Default mapping is `BridgeError::Internal { msg: e.to_string() }`. Where the existing handler already discriminates (e.g., a missing-row branch), the rewrite uses `NotFound`. Where it validates an argument and returns a `String` error, the rewrite uses `InvalidArgument`. The Supabase commands use `NotAuthenticated` for the no-active-session path. Coverage: a `bridge::error::tests::*` suite exercises every variant's serde round-trip (Tauri-side serialise → Leptos-side deserialise, and back).

**Stringly-typed boundary args, tightened in the same pass**: `update_tray_menu`'s `current_mode: String` becomes `current_mode: TimerMode`; `update_tray_icon`'s `session_mode: String` likewise tightens. Other arguments in the surviving handler set are scanned once during the cutover; any other stringly-typed enum-shaped argument is tightened to mirror the Leptos-side wrapper (no asymmetric types across the bridge).

**Alternatives considered**:
- **Keep `Result<T, String>` for cutover; tighten in a follow-up**: rejected — leaves FR-008 unfulfilled at merge time and creates a wider follow-up than the cutover-PR cost would have been.
- **Internally-tagged `#[serde(tag = "kind", content = "data")]`**: rejected — a unit variant like `NotAuthenticated` becomes `{"kind":"not_authenticated","data":null}`, which is noisier than the externally-tagged `{"kind":"not_authenticated"}`.

**Constitutional anchors**: III (Type Safety Over Defensive Code) — typed sum type closes the error domain. VI (The Tauri Boundary Is Stable) — a one-time, mechanical tightening of the existing surface, not a redesign.

---

## 14. One-shot legacy localStorage migration

**Decision**: A single Leptos-side entry point `bridge::storage::migrate_legacy_localstorage()` runs on first post-cutover launch, reads each preserved JS-era localStorage key, and hands the parsed payload to a per-domain Tauri command (`import_legacy_settings`, `import_legacy_history`, `import_legacy_tasks`, `import_legacy_tags`, `import_legacy_manual_sessions`, `import_legacy_user_state`, `import_legacy_supabase_session`). Each command is idempotent: if the corresponding Rust-side store has data, the import is skipped. After successful migration, the localStorage key is cleared (best-effort).

**Rationale**: The JS era persists 14 keys via `src/config/storage-keys.js`. The cutover replaces every consumer of those keys with Rust-managed app-data persistence. Without a migration, every released-`0.4.x` user would lose their tasks, history, tags, manual sessions, and settings on the first post-cutover launch — breaking FR-007's auto-updater promise. A one-shot migration with a defined sunset (one minor version) preserves user data while keeping the surviving steady-state Tauri surface clean.

**Per-key disposition**: see [data-model.md](./data-model.md) §"Legacy localStorage migration" for the full table. Two of the 14 keys are dropped: `pomodoro-stats` (vestigial — only ever cleared on reset, never read elsewhere) and `presto_force_update_test` (test-only; no production code path). The remaining 12 are migrated.

**Idempotency story**: every `import_legacy_*` Tauri command checks the corresponding Rust-side store before importing. If data exists, it returns `Ok(())` without writing. The Leptos entry point clears the localStorage key whether or not the import wrote — clearing is best-effort. A second-launch of the cutover build is a no-op: every key is already gone or skipped. Any partial-failure state (e.g., import failed mid-way) preserves the localStorage key, so the next launch retries.

**Sunset**: the migration entry point and all `import_legacy_*` commands are slated for removal one minor version after cutover. The follow-up cleanup release deletes the entry point, the per-domain commands, and the corresponding mock entries from `tauriMock.js`. By that minor's ship date, every active user has had their data migrated on at least one prior launch.

**Coverage**: per-import wasm-bindgen-test exercises the migration with mocked `localStorage` and asserts the matching Tauri command receives the expected payload (named in plan.md §Testing strategy).

**Alternatives considered**:
- **No migration; require user to re-create state**: rejected — breaks FR-007.
- **Persist the legacy localStorage keys forever**: rejected — leaves a 14-key migration burden in every future build, defeating the point of moving to Rust-managed storage.
- **Bundle the migration with an explicit user prompt**: rejected — a silent, idempotent migration is the smallest UX change that preserves data.

**Constitutional anchors**: II — guest mode flag preserved across the migration. VII — sunset window means no permanent compatibility burden.

---

## Cross-cutting summary

| Topic | Decision (one line) |
|---|---|
| Crate location | `src/` repurposed as `presto-web`; workspace root |
| Build tool | Trunk (CSR-only); `Trunk.toml` dist-dir at default (`src/dist/`); `tauri.conf.json` `frontendDist`/`beforeDevCommand`/`beforeBuildCommand` updated |
| Testing | `cargo test` + `wasm-bindgen-test` + Playwright; no Vitest |
| Playwright npm | Scoped `tests/e2e/package.json`, `@playwright/test` only |
| Aptabase | Rust adapter + new `track_event` Tauri command |
| Supabase | Rust REST adapter + four `supabase_*` Tauri commands; one-shot localStorage token migration via `import_legacy_supabase_session` (transition-only) |
| Theme code-gen | Trunk hook → `tools/build-themes/` binary → `themes.rs` |
| Xlsx | `rust_xlsxwriter` + new `export_sessions_xlsx` Tauri command |
| Remixicon | Vendored under `src/assets/icons/` |
| Activity monitoring | Same Rust-side architecture; Leptos folds events into `ActivitySignal` |
| Baseline re-capture | 0 default; ≤2 with justification; >2 escalates. Tolerance is a flake budget, not a drift allowance |
| Rollback | Patch release via auto-updater; no dual-build |
| Error handling | Typed `BridgeError` enum on every command; serde-tagged JSON; FR-008 promise backed |
| Legacy localStorage migration | One-shot entry point + per-domain `import_legacy_*` Tauri commands; idempotent; transition-only (sunset one minor after cutover) |

All spec markers are resolved; the plan proceeds to Phase 1.
