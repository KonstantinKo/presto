# Feature Specification: Leptos Frontend Migration

**Feature Branch**: `001-leptos-migration`
**Created**: 2026-05-09
**Status**: Draft
**Input**: User description: "Hard cutover migration of the presto frontend from vanilla JavaScript + HTML + CSS + Vite + Vitest to Leptos (Rust + WASM) + Trunk + wasm-bindgen-test. Tauri 2.x backend (`src-tauri/`) is unchanged. Playwright e2e suite (17 specs) and the 14 visual regression baselines under `tests/e2e/__screenshots__/visual-regression/` stay; pixel-equivalence within 2% per `playwright.config.js` is the green-light gate for the cutover."

## User Scenarios & Testing *(mandatory)*

> A tech-stack migration is judged not by new end-user features but by **contractual guarantees the migration must preserve**. Each story below is a guarantee that the post-cutover build must satisfy in isolation; satisfying any single story is meaningful evidence the migration has not regressed that contract. Constitutional anchors are cited inline.

### User Story 1 - Existing user updates via the auto-updater and notices nothing (Priority: P1)

An existing presto user is running an installed prior version (any released `0.4.x`). The Tauri auto-updater pulls the new build. After the update, the user opens the app: timer, tasks, tags, settings, history, calendar, theme, keyboard shortcuts, and audio + system notifications all behave and look identical to before. Their previously-saved local data (sessions, tasks, tags, settings — including `presto-guest-mode` / `presto-auth-seen` localStorage flags) is preserved across the update.

**Why this priority**: This is the user-facing acceptance test of the entire migration. If existing users see broken state, missing data, or visibly different behaviour after the update, the cutover has failed regardless of how clean the new code is. Tied to the constitution's anchors **I. The Timer Is Sacred** (engine behaviour preserved bit-for-bit) and **IV. Visual Regression Is The UI Contract** (screens look identical within tolerance).

**Independent Test**: Install a prior `0.4.x` build with sample data (a few sessions, tasks, tags, custom theme, analytics opt-in toggled to a known state). Trigger an update to the post-cutover build via the auto-updater. Reopen the app. Verify each persisted artefact is present and unchanged, the active theme still applies, the analytics opt-in retains its state, and the timer starts and runs a full 25-minute cycle producing the same transition events as before.

**Acceptance Scenarios**:

1. **Given** a user with an installed prior version containing N completed sessions, M tasks across K tags, and a non-default theme selected, **When** the auto-updater applies the new build and the user reopens the app, **Then** all N sessions, M tasks, K tags, and the non-default theme remain selected and applied.
2. **Given** a user who has set `settings.analytics_enabled` to true (or to false) in the prior build, **When** the update is applied and the app reopens, **Then** the same analytics opt-in state is in effect — no implicit re-prompt and no silent re-default.
3. **Given** a user who started a session and let the OS suspend the foreground tab for 90 seconds during a pomodoro, **When** the OS resumes the tab in the post-cutover build, **Then** the timer reflects 90 seconds of elapsed real wall-clock time (drift compensation per Principle I), not 90 fewer seconds of advanced state.
4. **Given** a user in guest mode with `presto-guest-mode=true` and `presto-auth-seen=true` in localStorage prior to the update, **When** the post-cutover build starts, **Then** guest mode persists and the sign-in overlay is not re-shown.
5. **Given** a user pressing each global shortcut (Cmd/Ctrl+Alt+Space, Cmd/Ctrl+Alt+R, Cmd/Ctrl+Alt+S, Cmd/Ctrl+H, Space) post-update, **When** the shortcut fires, **Then** the same action occurs as in the prior build (start/pause toggle, reset, skip, hide, space-on-timer-screen, respectively), with no shortcut having been silently dropped or remapped.

---

### User Story 2 - Developer clones the repo and runs the full test suite to green (Priority: P1)

A developer or maintainer clones the post-cutover branch and runs the full local quality pipeline. `cargo build --frozen` succeeds for both `src-tauri/` and the new Leptos crate. `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` passes with no warnings on the Leptos crate (matching backend posture per Principle III). `cargo fmt --check` passes. `cargo test` (Rust unit tests) passes. `wasm-bindgen-test` (DOM-coupled tests) passes. `npx playwright test` (all 17 e2e specs) passes. `npx playwright test tests/e2e/visual-regression.spec.js` passes against the existing 14 baselines within the `playwright.config.js` 2% pixel-ratio tolerance — no baseline regenerated as part of the migration.

**Why this priority**: This is the maintainer-facing acceptance test. The whole point of the migration (per **III. Type Safety Over Defensive Code**) is that the new toolchain is *stricter and simpler*; a quality pipeline that is harder to run, slower, or that requires baseline rewrites would invert the value proposition. This story also operationalises **IV. Visual Regression Is The UI Contract** as the green-light gate, **V. Test-First For Stateful Engines** (the timer engine and manager state machines have failing tests that precede their Rust implementations), and **IX. Lock Files Are First-Class** (`cargo build --frozen` not `cargo build`).

**Independent Test**: On a clean clone, run each command above in order and verify each exits with status 0 and produces no warnings. The visual regression run is the canonical gate — a passing visual diff against the unchanged baselines is what makes the migration verifiable.

**Acceptance Scenarios**:

1. **Given** the post-cutover branch checked out fresh, **When** a developer runs `cargo build --frozen` from the repo root for both crates, **Then** both succeed with no warnings and no lockfile drift.
2. **Given** the Leptos crate, **When** `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` runs against it, **Then** it exits 0 with zero warnings and any `#[allow(...)]` attributes carry an inline justification comment per Principle III.
3. **Given** the existing 14 PNG baselines under `tests/e2e/__screenshots__/visual-regression/`, **When** `npx playwright test tests/e2e/visual-regression.spec.js` runs against the post-cutover dev server, **Then** every screenshot passes within the 2% tolerance — none of the 14 baselines was regenerated as part of this migration.
4. **Given** the 17 Playwright e2e specs, **When** they run against the Leptos build served by the dev server with `tauriMock.js` installed, **Then** all 17 pass without modification to the spec file UI-interaction code (only `tauriMock.js` may have changed, and only if the bridge surface evolved).
5. **Given** the timer engine in its new Rust home, **When** `cargo test` runs the engine's behaviour tests (drift compensation, smart-pause activity gate, max-session cap, transition events), **Then** every test that existed for the JS engine has an equivalent passing Rust test — and those Rust tests were committed before the engine implementation per **V. Test-First For Stateful Engines**.

---

### User Story 3 - Developer adds a Tauri command and the type system catches a mismatch (Priority: P2)

A developer adds a new Tauri command in `src-tauri/src/lib.rs` (typed Rust handler with a defined argument struct, return type, and error type). They wire a Leptos call site that invokes it. They get one of the parameter types wrong on the call site — say they pass an `i32` where the handler expects a `u32`, or they decode the response into a struct with a renamed field. The build fails at compile time with a clear type error pointing at the call-site mismatch — not at runtime, and not via a JSDoc-only signal.

**Why this priority**: This story validates the *forward-looking* benefit of the migration, the one that justified incurring the cutover cost (per **VI. The Tauri Boundary Is Stable** + **III. Type Safety Over Defensive Code**). Boundary drift moves from a runtime panic class to a compile error class. The story is P2 (not P1) because it concerns post-migration developer ergonomics, not user-visible cutover acceptance.

**Independent Test**: Pick any existing Tauri command. Add a deliberate type mismatch on the Leptos call site (wrong scalar width, missing field on the response, or swapping argument order). Confirm `cargo build --frozen` fails with a compile error pointing at the call-site line, with no need to run the app or any test.

**Acceptance Scenarios**:

1. **Given** a Tauri command with a known argument and return shape, **When** a Leptos call site passes an argument whose type differs from the handler's declared type, **Then** the Leptos crate fails to compile with a localised error.
2. **Given** the same Tauri command, **When** the handler's argument struct gains a new required field but the call site still constructs the prior shape, **Then** the Leptos crate fails to compile.
3. **Given** that the bridge mock at `tests/e2e/fixtures/tauriMock.js` is the test surface for the bridge per **VI. The Tauri Boundary Is Stable** and **V. Test-First For Stateful Engines**, **When** a new Tauri command is added, **Then** the mock is extended first (with a default return for the new command), then the failing test is added, then the real call site lands — in that order.

---

### User Story 4 - Developer modifies a screen and the visual regression suite catches drift (Priority: P3)

A developer changes the UI of an existing screen — say, adjusts the spacing of the settings panel or modifies a button's hover state. They run `npx playwright test tests/e2e/visual-regression.spec.js` locally. The relevant baseline diffs flag the change. If the change was unintended, the developer fixes the code. If the change was intended, the developer regenerates only the affected baseline(s) with `--update-snapshots`, visually reviews the new PNG(s), and commits them in the same PR with a one-line note explaining the visual change per Principle IV.

**Why this priority**: This story validates that the visual regression contract continues to function in steady state after the migration — that **IV. Visual Regression Is The UI Contract** is preserved end-to-end, not just at the cutover gate. P3 because it's the routine post-migration workflow, not a cutover blocker.

**Independent Test**: Make any deliberate visual change to one screen on the post-cutover branch. Run the visual regression suite. Confirm the diff fails on exactly the affected baseline(s) and no others. Regenerate only those baselines, confirm the new PNGs reflect the intended change, and commit with a PR note.

**Acceptance Scenarios**:

1. **Given** a deliberate single-screen visual change, **When** the visual regression suite runs, **Then** only baselines for that screen flag a diff — no spurious diffs on unrelated screens (no global font/AA shift bleeding into other baselines).
2. **Given** an intended visual change with regenerated baselines, **When** the PR lands, **Then** the diff includes exactly the regenerated PNGs and a one-line note in the PR description explaining the visual change per Principle IV.
3. **Given** an unintentional visual diff (e.g., a CSS rule was clobbered as collateral of a refactor), **When** the developer sees the failure, **Then** the failing-actual / failing-expected / diff PNGs in the test output make the regression diagnosable without needing to re-run the prior build.

---

### Edge Cases

- **Skipped-version updater**: A user who skipped several `0.4.x` releases opens the post-cutover build for the first time. The local app-data format from any prior version they touched must still be readable; if a format migration is needed, it runs once on first post-update launch, is idempotent, and never deletes user data on failure (it logs and falls back to the prior format).
- **localStorage flags survive the migration**: `presto-guest-mode` and `presto-auth-seen` are read by the Leptos build via `web-sys` (window.localStorage). The keys, values, and write timing match what the JS build did. A user who was in guest mode pre-update is in guest mode post-update with no re-prompt.
- **Sub-pixel rendering drift**: The visual regression baselines were captured against the JS UI. If the Leptos UI legitimately renders one pixel differently for a font-kerning, sub-pixel-AA, or rasteriser-version reason, the 2% tolerance per `playwright.config.js` absorbs it. **[BEST-GUESS PM DECISION]**: if a *specific* baseline genuinely needs an updated capture (not absorbable inside tolerance), update that one baseline once with a one-line PR note. Re-capturing all 14 baselines as part of the migration is not allowed — that defeats Principle IV.
- **Aptabase analytics SDK replacement**: Aptabase ships a Tauri plugin (`@aptabase/tauri` JS shim + `tauri-plugin-aptabase` Rust crate). Post-migration the JS shim is gone. **[BEST-GUESS PM DECISION]**: call the existing Rust-side plugin directly from Leptos via a new thin Tauri command (e.g., `track_event`) that wraps `tauri-plugin-aptabase`. The opt-in toggle (`settings.analytics_enabled`) is checked at the Rust call site — never bypassed.
- **Supabase auth SDK replacement**: Supabase ships an official Rust SDK with limited coverage. **[BEST-GUESS PM DECISION]**: keep the auth surface narrow — the existing flows (sign in, sign out, session refresh, OAuth callback) talk to Supabase via direct REST + JWT from a Tauri-side adapter, and the Leptos frontend invokes that adapter through Tauri commands. This keeps the boundary discipline of Principle VI and avoids dragging a JS auth client into a WASM context.
- **Theme code-gen path**: `build-themes.js` runs as `predev` / `prebuild` npm scripts today, generating a JS module from CSS files in `art/`. **[BEST-GUESS PM DECISION]**: replace with a Trunk pre-build hook (or `build.rs` if more convenient) that produces the Rust equivalent. The "CSS files in `art/` are the source of truth" contract is preserved; the consuming code is regenerated, not hand-written. `/speckit-plan` picks the exact mechanism.
- **OAuth deep link / `tauri-plugin-oauth` callback**: The OAuth plugin is Rust-side already; the JS bridge surface is small (open URL, listen for callback). Post-migration the listener is a Leptos `use_event` over the same Tauri event channel — no IPC mechanism change per Principle VI.
- **Existing pinned playwright install path**: Today `@playwright/test` is in `package.json` devDependencies. Post-cutover root `package.json` is deleted. **[BEST-GUESS PM DECISION]**: replacement is a scoped `package.json` under `tests/e2e/` that pins `@playwright/test` only, plus a documented `npm ci` in that directory before running e2e. (Cargo cannot run `chromium`; the e2e scope still needs an npm shell.)
- **`xlsx` dependency** (used today for export): `xlsx` is a JS library. **[BEST-GUESS PM DECISION]**: replace with a Rust crate exposing a Tauri command that returns a generated `.xlsx` byte buffer; the Leptos frontend hands the buffer to the existing dialog/save flow. The existing user-visible export action is unchanged.
- **`remixicon` icon font**: shipped as static font + CSS today. **[BEST-GUESS PM DECISION]**: vendor the same font + CSS files under the Leptos `assets/` directory; Trunk serves them. No code-level dependency.
- **Activity-monitoring + smart-pause**: today the timer's smart-pause hooks DOM events (`mousemove`, `keydown`, `visibilitychange`). The post-migration Leptos engine reads these from the same DOM via `web-sys` listeners; **the engine never reads from the DOM** per Principle I — the listeners feed into a normalised `ActivitySignal` stream that the engine consumes.
- **Failure-mode rollback**: if the post-cutover build is shipped and a class of users sees a regression we did not catch, the recovery is `tauri-updater`'s normal-channel patch release. **No explicit dual-build coexistence is supported** — this is a hard cutover (per VISION.md and the feature brief).

## Requirements *(mandatory)*

### Functional Requirements

#### Behavioural parity (constitutional anchors I, IV)

- **FR-001**: The post-cutover timer engine MUST reproduce the JS engine's externally observable behaviour bit-for-bit: same transition events, same drift compensation under OS suspend / background-throttle, same smart-pause activity gate, same max-session cap, same manual-session-entry path through the engine.
- **FR-002**: The engine MUST be a pure state machine with no DOM reads — DOM-sourced signals (activity, visibility) MUST flow into the engine via a normalised input stream defined in the Rust layer.
- **FR-003**: All 14 PNG baselines under `tests/e2e/__screenshots__/visual-regression/` MUST pass without regeneration as part of this migration, within the 2% tolerance defined in `playwright.config.js`. A baseline regenerated as collateral of the migration is a migration failure.
- **FR-004**: All 17 Playwright e2e specs MUST pass without modification to their UI-interaction code. The only acceptable modifications are to `tests/e2e/fixtures/tauriMock.js` (and only if the underlying Tauri bridge surface evolved) and to test fixtures that explicitly seed pre-navigation state.

#### Persistence and update path (constitutional anchor II — referenced for context only since the spec scope is the cutover, not the local-first model)

- **FR-005**: Existing local user data (sessions, tasks, tags, settings, theme selection, analytics opt-in state) persisted by any released `0.4.x` build MUST be readable by the post-cutover build with no manual user action. If a format migration is required, it MUST run once on first launch, be idempotent, and on failure MUST log the error and preserve the original data unmodified.
- **FR-006**: The `presto-guest-mode` and `presto-auth-seen` localStorage flags MUST persist across the migration. A user in guest mode pre-update MUST remain in guest mode post-update with no sign-in overlay re-prompt.
- **FR-007**: The Tauri auto-updater path (signed-release pull → install → restart) MUST verify successfully against the post-cutover build during pre-release validation. End-to-end means: a clean install of a prior release, a triggered update, and a confirmed running post-cutover build with all FR-005 / FR-006 guarantees met.

#### Type-safe Tauri boundary (constitutional anchors III, VI)

- **FR-008**: Every Tauri command currently exposed in `src-tauri/src/lib.rs` MUST be reachable from the Leptos frontend via type-checked argument and return shapes. Drift between the Rust handler signature and the Leptos call site MUST be a compile error in the Leptos crate, not a runtime failure.
- **FR-009**: The frontend MUST gracefully short-circuit when `window.__TAURI_INTERNALS__` is unavailable (e.g., the pure dev server with no Tauri host). Code paths that invoke Tauri MUST not panic in that environment; they MUST short-circuit, mock-respond, or display a degraded state per the existing JS behaviour.
- **FR-010**: `tests/e2e/fixtures/tauriMock.js` MUST mirror every Tauri command reachable from the Leptos frontend. Adding a Tauri command MUST update the mock first, then the failing test, then the real call site, in that order.
- **FR-011**: The IPC channel MUST remain exclusively `invoke()` for commands and `listen()` for events. No new postMessage protocols, no raw window globals, no plugin-specific channels — per Principle VI.

#### Toolchain and quality gates (constitutional anchors III, V, IX)

- **FR-012**: The Leptos crate MUST be configured with `cargo clippy --all-targets -- -D warnings -W clippy::pedantic`. Pedantic warnings MUST be visible (unsuppressed) at PR time. `#[allow(...)]` attributes MUST carry an inline justification comment.
- **FR-013**: Closed domains in the post-cutover frontend (timer mode, session type, sound notification variant, manager state — each manager: auth, session, settings, navigation, tag, team, update) MUST be Rust enums (sum types). String-typed or open-enum representations of these domains are forbidden in the new code.
- **FR-014**: Failing tests MUST precede implementation for: the timer engine, manager state machines (auth, session, settings, navigation, tag, team, update), and Tauri-backed persistence helpers. UI rendering and view wiring are exempt — those are exercised by the e2e + visual regression suites per Principle V.
- **FR-015**: Post-cutover, root `package.json`, `package-lock.json`, `node_modules/`, `vite.config.js`, `vitest.config.js`, `eslint.config.js`, `tsconfig.json`, and `globals.d.ts` MUST be deleted from the repo root. The single exception: a scoped `package.json` under `tests/e2e/` retains `@playwright/test` as the only npm dependency. `Cargo.lock` becomes the single repo-root lock file per Principle IX.
- **FR-016**: All `tests/{core,managers,utils}/` Vitest specs MUST be deleted in the cutover commit. Their behaviour MUST be re-expressed as Rust `#[cfg(test)]` unit tests (for non-DOM logic) or `wasm-bindgen-test` tests (for DOM-coupled logic) before the JS specs are removed.

#### Out-of-scope guards (constitutional anchor VII referenced for context)

- **FR-017**: This feature MUST NOT add new user-facing features. New settings, new screens, new shortcuts, new commands beyond what is required by the migration itself (e.g., a `track_event` command wrapping `tauri-plugin-aptabase`, an `xlsx_export` command replacing the JS `xlsx` dependency) are out of scope and MUST be deferred to follow-up issues.
- **FR-018**: This feature MUST NOT switch the analytics or auth provider. Aptabase remains the analytics provider, with the same opt-in toggle and the same default-off posture per Principle II. Supabase remains the optional auth provider, with guest mode first-class per Principle II.
- **FR-019**: This feature MUST NOT add a Tauri Mobile target, "smart" pomodoro suggestions / AI features, or multi-user / cloud-only mode. Out-of-scope candidates surfaced during planning are filed as wontfix follow-ups, not silently scope-crept into this PR.

#### Non-functional preserved (constitutional anchor I implicitly)

- **FR-020**: All keyboard shortcuts (Cmd/Ctrl+Alt+Space, Cmd/Ctrl+Alt+R, Cmd/Ctrl+Alt+S, Cmd/Ctrl+H, Space) MUST continue to fire the same actions through the same global-shortcut plugin. No shortcut may be silently dropped, remapped, or made platform-specific where it was previously cross-platform.
- **FR-021**: The theme system MUST continue to treat the CSS files under `art/` as the source of truth. `build-themes.js` MAY be replaced by a Trunk pre-build step or `build.rs`, but the contract (drop a CSS file in `art/`, get a selectable theme) MUST hold.
- **FR-022**: The `art/` directory contents (theme CSS, brand assets) MUST NOT be modified as part of this feature beyond what is mechanically required to feed the Trunk pre-build pipeline (e.g., adding a manifest file in `art/` is allowed; rewriting CSS is not).

#### Open question (limited per skill spec)

- **FR-023**: The post-cutover Leptos crate's home directory in the repo (e.g., reusing `src/` after deleting all JS, or a sibling `web/`, or a workspace member like `crates/web/`) is [NEEDS CLARIFICATION: location of the Leptos crate within the repo — `src/` repurposed (lowest churn for `playwright.config.js` baseURL since dev port is the same) vs. a sibling `web/` directory (clearer separation from `src-tauri/`) vs. a `crates/web/` workspace member (most idiomatic for a Cargo multi-crate repo)]. PM lean: reuse `src/` after a clean wipe, since `playwright.config.js` baseURL and Tauri's dev-server-URL config both already point there; the Leptos crate's `Cargo.toml` lives next to `src/index.html` and Trunk serves from that location. Resolution belongs in `/speckit-plan`, not `/speckit-specify`.

### Key Entities

> Tech-stack migration; the entities are the boundary contracts the migration preserves, not new domain objects.

- **Tauri command surface**: the set of `#[tauri::command]` handlers in `src-tauri/src/lib.rs` (and any Rust-side plugin commands they delegate to). Identified by command name; characterised by argument shape, return shape, and error shape. Post-migration: each command has a corresponding type-checked Leptos call site.
- **Tauri event channel**: events emitted by the Rust side (e.g., updater events, OAuth callback events). Identified by event name; characterised by payload shape. Post-migration: each subscribed event has a corresponding Leptos listener with a typed payload.
- **Local persistence record**: the on-disk shape of sessions, tasks, tags, settings, theme selection, and analytics opt-in. Authoritative store is the Tauri app-data directory. localStorage holds the bounded subset (`presto-guest-mode`, `presto-auth-seen`) used in non-Tauri contexts and read on first launch. Migration MUST NOT alter on-disk shape; if it does (truly unavoidable), a one-shot idempotent migration at first launch is required (per FR-005).
- **Visual regression baseline set**: the 14 chromium-linux PNGs under `tests/e2e/__screenshots__/visual-regression/`. Identified by filename; the contract is pixel-equivalent rendering of the corresponding screen by the post-cutover build, within `playwright.config.js` tolerance.
- **Quality pipeline**: the ordered set of commands a developer or CI runs to gate a change (`cargo build --frozen` × 2 crates, `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` × 2 crates, `cargo fmt --check`, `cargo test`, `wasm-bindgen-test`, `npx playwright test`, `npx playwright test tests/e2e/visual-regression.spec.js`). Post-migration this set replaces the JS pipeline (`eslint`, `prettier --check`, `tsc --noEmit`, `vitest run`).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the 14 visual regression baselines pass against the post-cutover build with no baseline regenerated as part of the migration. Pixel-ratio diff stays within the 2% tolerance defined in `playwright.config.js`.
- **SC-002**: 100% of the 17 Playwright e2e specs pass without modification to UI-interaction code in spec files. Modifications restricted to `tests/e2e/fixtures/tauriMock.js` and pre-navigation fixtures only.
- **SC-003**: 0 baseline regenerations land in the cutover PR. (Amendment to a single baseline for a documented reason after the cutover lands is governed by Principle IV but is out of scope for this feature.)
- **SC-004**: 100% of Tauri commands currently reachable from the JS frontend are reachable from the Leptos frontend with type-checked argument and return shapes. A deliberate type mismatch on a call site fails `cargo build --frozen` at compile time.
- **SC-005**: An existing user on a prior `0.4.x` release applies the auto-update and finds 100% of their local data (sessions, tasks, tags, settings, theme, analytics opt-in, guest-mode flags) preserved. 0 data-loss incidents in pre-release validation.
- **SC-006**: The post-cutover Leptos crate reports 0 `clippy --all-targets -- -D warnings -W clippy::pedantic` warnings on a fresh build. Every `#[allow(...)]` carries an inline justification comment.
- **SC-007**: 100% of the timer engine's behaviour-level tests have an equivalent Rust test that lands *before* the engine's Rust implementation (commits ordered: failing tests → implementation), per Principle V.
- **SC-008**: Repo-root files removed in the cutover commit: `package.json`, `package-lock.json`, `node_modules/`, `vite.config.js`, `vitest.config.js`, `eslint.config.js`, `tsconfig.json`, `globals.d.ts`, all `tests/{core,managers,utils}/` Vitest specs. Only `Cargo.lock` remains as a repo-root lock file; the sole surviving npm `package.json` lives under `tests/e2e/` and pins `@playwright/test` only.
- **SC-009**: All 5 documented global keyboard shortcuts (Cmd/Ctrl+Alt+Space, Cmd/Ctrl+Alt+R, Cmd/Ctrl+Alt+S, Cmd/Ctrl+H, Space) work post-cutover with the same actions as pre-cutover. 0 silently-dropped shortcuts.
- **SC-010**: A new Tauri command added post-cutover requires updating `tauriMock.js` first, then the failing test, then the real call site — measured by reviewing the cutover-PR's first 3 follow-up PRs that add commands. 100% of those follow-ups follow the order.
- **SC-011**: First-launch format-migration path (if needed at all per FR-005) runs in under 2 seconds for a user with 1 year of historical sessions (~1000 sessions) and is idempotent across repeated cold starts.

## Assumptions

- **A1 — Hard cutover, single PR**: This feature is one PR, one feature, one landing. There is no flag-day dual-build coexistence and no feature-flag gating "old vs new" frontend.
- **A2 — Backend frozen**: All Rust code under `src-tauri/` is unchanged scope-wise. New Tauri commands solely *to enable* the migration (e.g., `track_event` wrapping `tauri-plugin-aptabase`, an `xlsx_export` command replacing the JS `xlsx` dependency) are in-scope; all other backend changes are deferred follow-ups per FR-019.
- **A3 — Visual regression is the gate**: The cutover ships only when the 14 baselines pass within tolerance. If the gate fails, the migration is fixed; baselines are not re-captured to silence the gate.
- **A4 — Test-first applies to engines + state machines, not UI plumbing**: Per Principle V, failing tests precede the timer engine, manager state machines, and persistence helpers. UI rendering, view wiring, and theme loading are exempt — exercised by the e2e + visual regression suites.
- **A5 — Aptabase via thin Tauri command, Supabase via Tauri-side adapter**: Per the edge case PM decisions above. These are best-guess routes; `/speckit-plan` may refine them but the boundary discipline (no JS auth/analytics client in the WASM context) is fixed.
- **A6 — Theme code-gen path replaced, contract preserved**: `build-themes.js` is replaced by a Trunk pre-build step or `build.rs`. CSS files in `art/` remain the source of truth.
- **A7 — Playwright stays npm**: `@playwright/test` survives in a scoped `tests/e2e/package.json`. Cargo cannot host the chromium runner; an npm shell scoped to e2e is the smallest possible JS surface area post-cutover.
- **A8 — chromium-linux baselines only**: Per `tests/e2e/CLAUDE.md`. Local diffs on macOS / Windows that pass on CI are trusted; the linux baselines are canonical.
- **A9 — Existing user base is current**: Updater testing covers the most-recent `0.4.x` releases. Users on much older builds (pre-`0.4.0`) are not a target for this cutover; they are expected to upgrade through the normal updater chain.
- **A10 — `--no-verify` is unused**: Per CLAUDE.md and Principle III. If a hook fails during this feature's implementation, the underlying issue is fixed; the hook is not bypassed.
- **A11 — `xlsx` and `remixicon` replaced as described**: `xlsx` JS lib → Tauri-side Rust crate behind a new command; `remixicon` font + CSS vendored under Leptos `assets/`. These are best-guess routes for `/speckit-plan` to confirm.
