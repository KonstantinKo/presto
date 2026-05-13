# Quickstart — Feature 005 (Multi-Locale UI With In-App Language Switcher)

Contributor's path to exercising the feature end-to-end, adding a new
translation key, adding a new locale, running the translation-completeness
check, and switching locale in the running app. Assumes a clean `005-i18n`
checkout; if `cargo` / `trunk` / `npx` are not on `PATH`, see the root
`AGENTS.md` and `CLAUDE.md`.

## Local build

```bash
# Frontend (Leptos + WASM via Trunk)
cd src
trunk build --release   # or: trunk serve  (dev mode, hot reload, http://localhost:1420)

# Backend (Tauri 2.x) — NOT runnable in CI / agentex worktrees (needs GUI deps).
# For local desktop runs only:
cargo tauri dev
```

A `trunk build --release` step is the cold-start sanity check that picks
up the new `src/locales/*.json` files via the `leptos_i18n::load_locales!()`
proc-macro at the crate root. The four catalogues are read at compile time
and embedded into the WASM binary — no separate `<link data-trunk
rel="copy">` is needed in `src/index.html`. After the build, the `dist/`
tree should NOT contain a `dist/locales/` subdirectory (catalogues are
compiled in, not served).

## Where message catalogues live

Four files, one per supported locale, at:

```text
src/locales/en.json    # Source-of-truth catalogue (Spec A13)
src/locales/de.json    # German translation
src/locales/it.json    # Italian translation
src/locales/tr.json    # Turkish translation
```

The locale list and the default locale are declared in `src/Cargo.toml`:

```toml
[package.metadata.leptos-i18n]
default = "en"
locales = ["en", "de", "it", "tr"]
```

### Catalogue file structure

Each catalogue is a JSON object whose top-level keys group related strings
by view-area (per-view namespaces). Example excerpt from `en.json`:

```json
{
  "timer": {
    "mode_focus": "Focus",
    "mode_break": "Break",
    "mode_long_break": "Long Break",
    "state_paused": "Paused",
    "state_overtime": "Overtime",
    "ctrl_reset": "Reset",
    "ctrl_start": "Start",
    "ctrl_pause": "Pause"
  },
  "settings": {
    "general": {
      "language_label": "Language",
      "focus_duration_label": "Focus Duration (minutes):"
    },
    "auto_save_ok": "Settings saved",
    "auto_save_err": "Failed to save settings"
  }
}
```

The same JSON path (e.g. `timer.mode_focus`) MUST exist in all four
locale files. The proc-macro fails the build (via the
`-D warnings`-promoted `deprecated` lint) on any divergence — see "How
to run the translation-completeness check" below.

### Why JSON (not YAML / FTL / TOML)

See `research.md` Decision 2. JSON is the library default with the
lowest editor-tool barrier for a hand-curated catalogue. YAML / JSON5
are available behind cargo features if a follow-up needs them.

## How to add a translation key

Three steps. All three must happen in the same commit to keep CI green.

### Step 1 — Add the key to `en.json` (source of truth)

```diff
 {
   "timer": {
     "mode_focus": "Focus",
+    "mode_overtime_warning": "Overtime — session continues",
     "mode_break": "Break"
   }
 }
```

### Step 2 — Add the key to `de.json`, `it.json`, `tr.json`

```diff
 // de.json
 {
   "timer": {
     "mode_focus": "Fokus",
+    "mode_overtime_warning": "Überzeit — Sitzung läuft weiter",
     "mode_break": "Pause"
   }
 }
```

(Repeat for `it.json` and `tr.json` with the appropriate translations.
Native-speaker review is desirable but the spec does not block on it —
per FR-029, machine-translation-driven generation is out of scope and
the catalogues are hand-curated.)

### Step 3 — Use the typed key in a view

```rust
use crate::i18n::*;

let i18n = use_i18n();
view! {
    <span class="overtime-banner">
        {t!(i18n, timer.mode_overtime_warning)}
    </span>
}
```

The `t!(i18n, timer.mode_overtime_warning)` call is **compile-time-checked**.
A typo `timer.mode_overtime_warring` fails `cargo build` with
`error[E0599]: no method named 'mode_overtime_warring'`. No stringly-typed
lookup path exists in the public API.

### Step 4 — Run the build to confirm

```bash
cargo build --workspace --frozen
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic
```

If the key is missing from any of the three non-English catalogues, the
clippy run fails with `error: use of deprecated function 'warnings::w0':
Missing key "timer.mode_overtime_warning" in locale "de"` (or `it` / `tr`).
Fix the catalogue and re-run.

## How to add a locale

Four steps. Adding a fifth locale (e.g. Spanish) past v1's four-locale
scope is its own follow-up feature per Spec FR-024 — but the mechanical
path is documented here for future reference.

### Step 1 — Add the variant to `presto_ipc::Locale`

```diff
 #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
 #[serde(rename_all = "lowercase")]
 pub enum Locale {
     #[default]
     En,
     De,
     It,
     Tr,
+    Es,
 }
```

### Step 2 — Add the locale to `Cargo.toml`'s metadata block

```diff
 [package.metadata.leptos-i18n]
 default = "en"
-locales = ["en", "de", "it", "tr"]
+locales = ["en", "de", "it", "tr", "es"]
```

### Step 3 — Create the new catalogue file

```bash
# Copy the source-of-truth as a starting point; translate every value.
cp src/locales/en.json src/locales/es.json
# Hand-translate each string in src/locales/es.json.
```

### Step 4 — Update the dropdown's native-self-name list

In `src/src/components/settings/general.rs`, add the new option:

```diff
 <select id="locale-selector" on:change=on_change ...>
     <option value="en">"English"</option>
     <option value="de">"Deutsch"</option>
     <option value="it">"Italiano"</option>
     <option value="tr">"Türkçe"</option>
+    <option value="es">"Español"</option>
 </select>
```

And extend the `match` arm in `on_change`'s body, plus the
`From<presto_ipc::Locale> for i18n::Locale` impl in `src/src/i18n.rs`,
plus the `match_two_letter_prefix` helper's match arms.

### Step 5 — Run the build

```bash
cargo build --workspace --frozen
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic
```

The `SurplusKey` warning fires if `es.json` has any keys not present in
`en.json`; the `MissingKey` warning fires if any `en.json` key is absent
from `es.json`. Both promote to `deprecated`-lint failures and fail CI
under `-D warnings`.

## How to run the translation-completeness check

The check is performed by the `leptos_i18n` proc-macro at build time and
surfaced via the `deprecated` lint. The presto CI invocation is:

```bash
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic
```

(per `.agentex.yml` `lint:` block — exact line). Run this locally before
push to catch missing / surplus keys.

### What a failure looks like

```text
error: use of deprecated function `warnings::w0`: Missing key
       "settings.general.new_label" in locale "de"
   --> src/src/lib.rs:42:1
    |
 42 | leptos_i18n::load_locales!();
    | ----------------------------- in this macro invocation
    |
    = note: `-D deprecated` implied by `-D warnings`
```

### Why no bespoke script?

The proc-macro already performs the key-set difference check at expansion
time and emits a diagnostic. Adding a bespoke
`scripts/check-translation-completeness.sh` would duplicate that work
with its own bug surface (path globs, JSON parsing, key-set extraction).
The existing CI clippy gate is sufficient — see `research.md` Decision 3.

## How to switch locale in the running app

### Via the Settings → General dropdown (the canonical user path)

1. Launch the app: `cargo tauri dev` (on a desktop with the GUI deps
   installed) or `trunk serve` (in browser, for visual debugging only —
   the Tauri-side persistence won't fire under Trunk's dev server,
   per the bridge-availability shim in `src/src/bridge/`).
2. Open Settings (sidebar nav, gear icon or `Settings` tooltip).
3. The General tab is the default. The new "Language" row sits above
   the timer-durations section.
4. Click the `#locale-selector` dropdown and pick `Deutsch` (or
   `Italiano` / `Türkçe`).
5. The UI re-renders in the picked locale **immediately** — no toast,
   no restart, no reload prompt. The Settings page itself flips too:
   the surrounding "Language" label becomes `Sprache` (de) / `Lingua`
   (it) / `Dil` (tr); the dropdown options stay `English / Deutsch /
   Italiano / Türkçe` (native self-names; never re-translated per
   Spec FR-015 / Story 1 AC 4).

### Via direct `settings.json` edit (debug path)

```bash
# Locate the Tauri app-data directory (platform-specific):
# - Linux:   ~/.local/share/com.koss-konstantin.presto/
# - macOS:   ~/Library/Application Support/com.koss-konstantin.presto/
# - Windows: %APPDATA%\com.koss-konstantin.presto\
# Edit settings.json:
{
  "appearance": {
    "theme": "auto",
    "timer_theme": "espresso",
    "locale": "de"
  },
  ...
}
# Relaunch the app — boot-time resolution reads the new value.
```

### Via the dev console (Trunk serve / e2e mock)

The Tauri bridge is absent under `trunk serve`; locale changes via the
dropdown won't persist. The dropdown's `on:change` handler still
updates the in-memory `settings.appearance.locale` value, and the
library's locale signal still flips — so view re-rendering still
fires. Useful for visual debugging.

## How to run the new tests

### Three RED-first IPC round-trip tests (`cargo test`)

These are the test-first failing tests that precede the implementation
commit (see `plan.md` Phase 0):

```bash
# From the repository root:
cargo test --workspace --frozen \
    -p presto-ipc \
    -- \
    locale_legacy_field_defaults_en \
    locale_round_trip \
    locale_serialises_lowercase
```

Or run the whole `settings.rs` test module to see them alongside the
existing legacy fixtures:

```bash
cargo test --workspace --frozen -p presto-ipc settings::tests
```

### Locale-resolution unit test (FR-023)

```bash
# From the repository root:
cargo test --workspace --frozen \
    -p presto-web \
    -- \
    resolve_initial_locale
```

Covers the five branches of the FR-009 precedence chain (persisted-wins,
OS-detection-wins, no-supported-match-fallback, empty-os-fallback,
multi-language-list-first-match-wins). The function is pure — it takes
the persisted value and the OS-detected language strings as parameters,
returning the resolved `Locale` — so it runs under `cargo test` without
a DOM. The actual `web_sys::window().navigator().languages` read happens
in `src/src/app.rs`'s boot path and is e2e-covered, not unit-tested.

### New e2e flow (Settings → General locale switcher)

```bash
# From the repository root:
cd tests/e2e
npx playwright test settings-general.spec.js --reporter=line
```

The new flow exercises:
- Open Settings → General.
- Verify the new `#locale-selector` row is present above the
  timer-durations section.
- Pick `Deutsch` from the dropdown.
- Assert the surrounding label changes to `Sprache`.
- Assert a non-Settings string in another tab (e.g. the Notifications
  tab's "Auto-start timer" label) renders in German.
- Pick `English` from the dropdown.
- Assert the strings revert.

## How to regenerate the affected baseline

Exactly one baseline regenerates:
`tests/e2e/__screenshots__/visual-regression/settings-general-chromium-linux.png`.

```bash
cd tests/e2e
npx playwright test visual-regression.spec.js \
    --update-snapshots \
    --grep "settings-general"
```

After regeneration, review the diff visually against the per-baseline
justification (below) and commit the regenerated baseline. The PR
description MUST include the per-baseline note verbatim.

### Per-baseline justification (paste verbatim into the PR description)

> `settings-general-chromium-linux.png`: Language dropdown row added
> above the timer-durations section, four native-self-name options
> (English / Deutsch / Italiano / Türkçe). No other layout change.

### Sidebar mask still in effect

The feature 003 sidebar-mask posture (`mask: [page.locator(".sidebar")]`
on non-sidebar baselines) remains active. This feature does NOT change
the sidebar — sidebar nav tooltips ARE localised but the sidebar's
visible chrome (icons only by default) is the same in every locale, so
the masked region doesn't shift. Per FR-021 / Spec Story 3 AC 3, any
diff on the timer / statistics / daily / tag-manager / update-notification
baselines is treated as a regression to fix in code, not absorbed into
the baseline.

## Full local gate sweep (pre-PR)

Mirror what CI will run:

```bash
# Lints + formatting (translation-completeness check runs HERE via -D warnings)
cargo fmt --check
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic

# Unit + integration tests
cargo test --workspace --frozen

# Frontend build (cold; verifies the load_locales! macro reads all four files)
cd src && trunk build --release && cd ..

# wasm-bindgen-tests
cd src && wasm-pack test --node && cd ..

# E2E + visual regression
cd tests/e2e && npx playwright test --reporter=line && cd ../..

# Mock-drift gate (no-op for this feature; sanity check)
bash scripts/check-mock-drift.sh

# Engine-purity gate (zero new web_sys references under src/src/engine/)
bash scripts/check-engine-purity.sh

# Lockfile-drift gate (one new runtime dep — Cargo.lock MUST be staged)
git rev-parse origin/main >/dev/null 2>&1 || git fetch --no-tags --prune origin main
LOCKFILE_BASE_REF=origin/main bash scripts/check-lockfile-drift.sh
```

All gates should exit zero. If any fail, fix forward (do NOT
`--no-verify`).

## Smoke-test the locale-switching behaviour (manual)

Headless chromium can't observe the full multi-view re-render in a
single screenshot; for the PR-time smoke test, run `cargo tauri dev`
on a desktop and walk:

1. Open Settings → General. Confirm the `Language` row sits above the
   timer-durations section.
2. Pick `Deutsch`. Within the same render tick, confirm:
   - The Language label changes to `Sprache`.
   - Every other Settings tab label changes (e.g. `Notifications` →
     `Benachrichtigungen`).
3. Close Settings; navigate to the timer view. Confirm the mode badge,
   control button labels, and any state suffix render in German.
4. Navigate to Statistics. Confirm period tabs (`Daily` → `Täglich` /
   `Weekly` → `Wöchentlich`), tile labels, and "No data" empty-state
   strings render in German.
5. Navigate to Daily. Confirm `Daily Overview` → `Tagesübersicht` and
   the `Today's Sessions` / `No sessions completed` strings render in
   German. The `chrono`-formatted date strings (e.g. `Mon, May 13`)
   stay in English (Spec Clarifications 2026-05-13 / FR-025 / A8).
6. Open the tag picker. Confirm the `New tag…` placeholder and
   `Choose tag` header render in German.
7. Switch to `Italiano` and `Türkçe` in turn; spot-check each view.
8. Switch back to `English`. Confirm the original strings restore.
9. Quit and relaunch the app. Confirm the last-picked locale persists
   (the boot-time resolution reads `settings.appearance.locale`).

If any step deviates from Spec Acceptance Scenarios, file a bug —
don't ship.

## Smoke-test OS-locale detection (manual, P3 path)

This requires a fresh install (no `settings.json` yet) on a machine
whose OS locale is non-English. Easiest path: rename your local
`settings.json` to `settings.json.bak`, set `LANG=de_DE.UTF-8` in
the shell, and launch the app. The cold-start path should:

1. Read `settings.appearance.locale` → falls back to `Locale::En`
   (the file doesn't exist; `Settings::default()` returns).
2. The library's `<I18nContextProvider initial_locale=None>` path
   triggers OS detection via `leptos-use::use_locales` →
   `navigator.languages` returns the platform-mapped value (e.g.
   `["de-DE", "de", "en-US"]`).
3. The first matching supported prefix wins → `Locale::De`.
4. The UI renders in German on first paint.
5. Picking a different locale from the dropdown writes the picked
   value to `settings.appearance.locale`, which the existing
   debounced autosave Effect at `src/src/app.rs:215+` persists.

Restore your `settings.json.bak` after testing.
