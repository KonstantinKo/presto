# Implementation Plan: Multi-Locale UI With In-App Language Switcher

**Branch**: `005-i18n` | **Date**: 2026-05-13 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification at `specs/005-i18n/spec.md`

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

A single user-facing capability: in-app UI locale switching across the
entire visible string surface, with four bundled locales (English,
German, Italian, Turkish). Settings → General gains one new control row
— a `#locale-selector` dropdown placed above the timer-durations
section, with four native-self-name options. `AppearanceSettings` (in
`crates/presto-ipc/src/settings.rs`) evolves with one
`#[serde(default)]` field (`locale: Option<Locale>`) and one new lowercase enum
`Locale` (four variants `En` / `De` / `It` / `Tr`), mirroring the
existing `theme` field's lowercase wire-shape convention at
`crates/presto-ipc/src/settings.rs:120-123` rather than the kebab-case
`AmbientSoundType` pattern (two-letter ISO-639-1 codes have no internal
word boundary). Library pick — **`leptos_i18n` at `v0.5.11`** — is the
only `0.5.x` release compatible with `leptos = "0.7"` (the current
project pin at `src/Cargo.toml:20`); the `0.6.x` series requires
`leptos = "0.8"`. Rejected alternatives: `leptos-fluent` (stringly-typed
`tr!("key")` API; missing-key checking is debug-mode-only opt-in;
incompatible with FR-005 + FR-006). Rationale and verified facts in
[research.md](./research.md).

Catalogue location: `src/locales/<locale>.json` (4 files: `en`, `de`,
`it`, `tr`). Source-of-truth is `en.json` per Spec A13. Format is JSON
per [research.md Decision 2](./research.md#decision-2--catalogue-file-format-json-library-default).
Catalogues compiled into the WASM binary via `leptos_i18n::load_locales!()`
proc-macro — no runtime fetch, no on-disk catalogue outside the WASM
bundle (Spec FR-008 / FR-019). Locale resolution at boot: persisted
`appearance.locale` if `Some(_)` → OS detection via
`leptos-use::use_locales` (`navigator.languages` reader, transitively
pulled by `leptos_i18n`) → fall back to `Locale::En` (Spec FR-009).
The OS-detection path is built into the library; the presto wiring
short-circuits to it only when the persisted value is `None`
(Spec User Story 4 AC 3 / FR-011).

Locale switching is signal-batched: the dropdown's `on:change` handler
writes to `settings.appearance.locale` (the IPC signal), and a small
`Effect` in `src/src/i18n.rs` (new module) mirrors the change into the
library's `i18n.set_locale(...)` so every `t!(i18n, ...)` call site
re-renders in the same Leptos reactive tick (Spec FR-007 / FR-012 /
SC-007 / Edge Case "mixed-locale frame avoidance"). The existing
debounced settings-autosave Effect at `src/src/app.rs:215+` picks up
the change and persists it; no bespoke save-button, no toast specific
to the language change (Spec FR-016).

No new Tauri commands (Spec FR-018 / SC-012). One new IPC field
(`AppearanceSettings.locale`) flows through the existing
`save_settings` / `load_settings` round trip. One new runtime crate
dependency (`leptos_i18n = "=0.5.11"`, exact-pinned) — `Cargo.lock` updates in lockstep
(Principle IX). One visual-regression baseline regenerates
(`settings-general-chromium-linux.png`) carrying a one-line
PR-description note (Principle IV / Spec FR-021 / SC-009).

Detail in [research.md](./research.md), [data-model.md](./data-model.md),
[contracts/components.md](./contracts/components.md),
[quickstart.md](./quickstart.md).

## Technical Context

**Language/Version**: Rust 1.83+; `wasm32-unknown-unknown` target for
the Leptos crate; backend Rust unchanged. No version bump from feature
004's baseline.

**Primary Dependencies**: One new runtime dependency —
`leptos_i18n = { version = "=0.5.11", default-features = false, features =
["csr", "json_files", "icu_compiled_data"] }` added to `src/Cargo.toml`.
Exact-pinned: only `0.5.11` is verified against `leptos = "0.7"`. Upgrade
only when a new `0.5.x` is explicitly retested (Fix C).
Transitive dependencies brought in by this addition: `leptos_i18n_macro`
(proc-macro subcrate), `leptos-use` (for `use_locales` OS-detection
helper, with the `use_locales` and `use_cookie` features active in the
library's own manifest — the `use_cookie` activation is transitive and
unused by presto), `icu_locid` (BCP-47 parsing), `codee`,
`typed-builder`, `default-struct-builder`, and their respective
transitives (estimated ~12–18 additional crates; all pure-Rust, no
`cc`-compiled C dependencies, no platform-specific gating). Backend
deps unchanged (the IPC `Locale` enum lives in `crates/presto-ipc` and
is `cfg_attr`-gated for `specta::Type` like the rest of the IPC types
— the backend doesn't depend on `leptos_i18n`).

**Storage**: Tauri app-data directory; unchanged. `settings.json`
evolves at the field level only inside the `appearance` block (one new
field, `#[serde(default)]`). Legacy records without the new field
deserialise to `None`. The first cold-start after upgrade runs the
resolver, which produces a concrete `Locale`; the centralized
settings-persistence Effect then writes `Some(Locale)` back to disk,
so subsequent loads short-circuit OS detection (per FR-011). No new
on-disk files; the four locale catalogues are not user data — they are
vendored read-only resources compiled into the WASM binary.

**Testing**: `cargo test --workspace --frozen` for the three new IPC
round-trip tests (`locale_legacy_field_defaults_none`, `locale_round_trip`,
`locale_serialises_lowercase`) in `crates/presto-ipc/src/settings.rs::tests`
plus the locale-resolution unit test (`resolve_initial_locale` in
`src/src/i18n.rs::tests`); Playwright e2e for the Settings UI plumbing
(new selector `#locale-selector` in `tests/e2e/settings-general.spec.js`);
visual regression for the one affected baseline
(`settings-general-chromium-linux.png`). The translation-completeness
gate (FR-006) is enforced by `cargo clippy -- -D warnings` via the
proc-macro's `#[deprecated]` warning promotion — no separate test
runner.

**Target Platform**: macOS, Linux, Windows desktops (CSR-only
single-window Tauri 2.x). The Tauri WebView varies per platform —
WKWebView on macOS, WebView2 (Chromium / Edge) on Windows, WebKitGTK on
Linux. All three execute the `leptos_i18n`-generated WASM identically;
the library has no platform-specific surface (no workers, no IndexedDB,
no `fetch` calls — all four catalogues are bundled into the WASM
binary at compile time).

**Performance Goals**: No regression. Locale switching is a
constant-time reactive-signal flip (Spec FR-007); all four catalogues
are resident in WASM memory after init (no async loading step, no
network fetch). WASM bundle delta is estimated at ~50–100 KB
(`leptos_i18n` generated code + four locale catalogues' string tables,
ICU CLDR data off by default). No specific SC measures bundle size;
SC-008 measures network-egress code paths, not WASM bytes.

**Constraints**: Strict static analysis stays green (Principles III + X).
The translation-completeness gate (Spec FR-006) leans on
`cargo clippy --workspace --all-targets --frozen -- -D warnings -W
clippy::pedantic` (per `.agentex.yml` `lint:`) — the proc-macro's
`MissingKey` warning emits a `#[deprecated]` annotation on a generated
fn call that the `-D warnings` flag promotes to a hard build error.
**No additional CI script is needed.** The engine-purity grep gate
(`scripts/check-engine-purity.sh`) must stay at zero `web_sys`
references under `src/src/engine/`; this feature only touches UI-side
code (`src/src/components/`, `src/src/app.rs`, the new
`src/src/i18n.rs`) and IPC types (`crates/presto-ipc/`), so the gate
stays green by construction. The baseline-cap gate stays at the
default — one baseline regenerates (`settings-general-chromium-linux.png`).

**Scale/Scope**: One new wire-shape field, one new lowercase enum, one
new UI-side module (`src/src/i18n.rs`), four vendored JSON catalogues,
one Settings UI tab evolution, one e2e spec evolution, one baseline
regeneration. Every component file with user-visible English strings
gets one or more `t!(i18n, ...)` call-site rewrites. **~15–20 files
touched** (the IPC crate, the new module, `src/src/app.rs`, the
locale-switcher view, plus every component carrying FR-013-scope
strings — full list in [§ Modules](#modules)). No new Tauri commands;
no new IPC commands; one new runtime dep.

## Constitution Check

*GATE: must pass before Phase 0. Re-checked after Phase 1.*

Only principles with material content are listed below per repo
artefact discipline.

### II. Local-Only — catalogues compiled in, zero remote fetch

Four JSON files are committed under `src/locales/` and embedded into
the WASM binary at compile time via the `leptos_i18n::load_locales!()`
proc-macro's `include_str!`-style read. There is no runtime fetch, no
CDN, no late-loaded catalogue, no async resolution step. The
auto-updater traffic is unchanged. The `_blockExternal` e2e fixture
remains effective. The `leptos-use` transitive crate's `use_locales`
helper reads `navigator.languages` (a Web platform property, not a
network call) for the OS-detection path — no egress. **PASS.**

### III. Type Safety Over Defensive Code — typed-key API, closed enum

- `Locale` is a closed four-variant sum type (`En`, `De`, `It`, `Tr`)
  with `#[serde(rename_all = "lowercase")]`, mirroring the `theme`
  field's lowercase string convention at
  `crates/presto-ipc/src/settings.rs:120-123`. Out-of-set wire values
  fail serde deserialisation; `#[serde(default)]` substitutes
  `None` per Spec FR-004.
- Every `t!(i18n, ...)` call site is **compile-time-checked** via the
  `leptos_i18n` proc-macro: the key argument is a typed Rust
  identifier path on the macro-generated `I18nKeys` struct, not a
  string literal. A typo fails `cargo build` with `error[E0599]: no
  method named ...`. **No stringly-typed `t("some.key")` lookups
  anywhere in the codebase** (SC-005); the library doesn't expose a
  stringly-typed surface — `leptos-fluent` (rejected per [research.md
  Decision 1](./research.md#decision-1--library-pick-leptos_i18n-at-v0511))
  does, but the picked library does not.
- Interpolation parameter names are also typed: `t!(i18n,
  daily.session_count, count = n)` rejects `cont = n` at compile time
  with "no field named `cont`".
- The `From<presto_ipc::Locale> for i18n::Locale` impl in
  `src/src/i18n.rs` is total (the variant sets are by-design 1:1);
  match-exhaustiveness fails compilation if either enum drifts.

**PASS.**

### IV. Visual Regression Is The UI Contract — one baseline

One baseline regenerates:
`settings-general-chromium-linux.png` (one new control row added above
the timer-durations section — the `#locale-selector` dropdown with
four native-self-name options). Per Spec FR-021 / SC-009, no baseline
outside Settings → General regenerates. The non-Settings-General
baselines are captured in English (the source-of-truth locale per Spec
Clarifications 2026-05-13) and stay locked. Per-locale baselines are
explicitly OUT of scope (Spec Clarifications 2026-05-13 — adding three
more screenshot sets per touched screen would quadruple the baseline
count for marginal contract value).

**Per-baseline justification** (pre-anchored here; restated verbatim
in the PR description):
- `settings-general-chromium-linux.png`: Language dropdown row added
  above the timer-durations section, four native-self-name options
  (English / Deutsch / Italiano / Türkçe). No other layout change.

The feature 003 sidebar-mask posture (`mask: [page.locator(".sidebar")]`
on non-sidebar baselines) remains in effect — the sidebar's visible
chrome (icons only by default) is the same in every locale and the
masked region doesn't shift. Sidebar tooltips ARE localised but the
tooltip's hover state is not captured by the baseline suite.

**Risk**: every visible English string is extracted into a typed-key
catalogue. A side effect of the extraction may be subtle width changes
(e.g. a localised string ends up slightly different in source-locale
length even before any translation lands; or the Leptos template's
text-node coalescing changes around `{t!(...)}` interpolations vs
plain `"..."` string literals). Per Spec Story 3 AC 3 / FR-021, such a
diff is a **code regression to fix at the call site** — not absorbed
into a re-baseline. The pre-implementation walk-through commits should
keep the strings byte-stable on the English source-of-truth side (key
typed swap, English value unchanged).

**PASS** with one documented baseline regeneration (Principle IV's
documented "intended change + one-line note" mechanism, not a
widening).

### V. Test-First For Stateful Engines — IPC + resolver scope

The engine has no new state; the manager state machines are untouched;
persistence helpers are untouched. The IPC `AppearanceSettings`
wire-shape evolution IS in Principle V scope (the round-trip is the
persistence boundary). Three RED-first tests precede implementation:

- `presto_ipc::settings::tests::locale_legacy_field_defaults_none`
  `[test-first]` — asserts a pre-feature-005 `AppearanceSettings`
  JSON (no `locale` key, only the feature 004 baseline shape)
  deserialises to `locale = None`. Mirrors the
  `ambient_sound_legacy_fields_default` precedent at
  `crates/presto-ipc/src/settings.rs:407-421`.
- `presto_ipc::settings::tests::locale_round_trip` `[test-first]` —
  asserts a non-default `AppearanceSettings` (e.g. `theme = "auto",
  timer_theme = "espresso", locale = De`) round-trips byte-stable
  through serde for each of the three non-default locales, AND the
  feature 002–004 fields preserved across the same round-trip
  (Spec Story 2 AC 5).
- `presto_ipc::settings::tests::locale_serialises_lowercase`
  `[test-first]` — asserts each of the four `Locale` variants
  serialises to its lowercase wire string (`"en"`, `"de"`, `"it"`,
  `"tr"`) and round-trips byte-stable in both directions.

The locale-resolution function `resolve_initial_locale` at
`src/src/i18n.rs` is also test-first per Spec FR-023:

- `presto_web::i18n::tests::resolve_initial_locale_persisted_wins`
  `[test-first]` — `persisted = De, os = ["en-US"]` → `De`.
- `presto_web::i18n::tests::resolve_initial_locale_os_wins_when_default`
  `[test-first]` — `persisted = En, os = ["de-DE"]` → `De`.
- `presto_web::i18n::tests::resolve_initial_locale_unsupported_os_falls_back`
  `[test-first]` — `persisted = En, os = ["fr-FR"]` → `En`.
- `presto_web::i18n::tests::resolve_initial_locale_empty_os_falls_back`
  `[test-first]` — `persisted = En, os = []` → `En` (covers Spec
  Story 4 AC 5 — `navigator.languages` unavailable).
- `presto_web::i18n::tests::resolve_initial_locale_first_match_wins`
  `[test-first]` — `persisted = En, os = ["zh-CN", "ja-JP", "tr-TR"]`
  → `Tr` (first matching supported prefix wins).

All seven resolver tests run under `cargo test --workspace --frozen`
because `resolve_initial_locale` is pure (no `web_sys`, no DOM) per
FR-023's "extractable for testing in isolation" direction. The
actual `navigator.languages()` read happens in `src/src/app.rs`'s
boot path and is e2e-covered.

UI plumbing (dropdown rendering, signal-binding, view re-rendering)
is e2e-covered, NOT in Principle V scope.

**Mock-first rule (VI): N/A** — no new Tauri commands.

**PASS.**

### VI. The Tauri Boundary Is Stable — no new commands, no new IPC

No new Tauri commands. The new `AppearanceSettings.locale` field
flows through the existing `save_settings` / `load_settings` round
trip. Wire-shape evolution is per the existing `#[serde(default)]`
pattern (mirrors the `theme` / `timer_theme` evolution at
`crates/presto-ipc/src/settings.rs:120-123` exactly). The mock-drift
gate (`scripts/check-mock-drift.sh`) sees no new commands and stays
green without mock changes — verified against
`tests/e2e/fixtures/tauriMock.js`.

**PASS.**

### IX. Lock Files Are First-Class — one new runtime dep

ONE new runtime dependency: `leptos_i18n = "=0.5.11"` (exact-pinned;
only `0.5.11` is verified against `leptos = "0.7"`; upgrade only when a
new `0.5.x` is explicitly retested — Fix C) with explicit features
`["csr", "json_files", "icu_compiled_data"]` and `default-features = false`
to drop the `cookie` feature, added to `src/Cargo.toml`'s `[dependencies]`
block. `Cargo.lock` MUST be
regenerated and staged in the same commit as the manifest change
(Principle IX / Spec FR-020). CI's `cargo build --frozen` and
`cargo fetch --locked` (per `.agentex.yml`) catch any drift.

The lockfile-drift gate
(`scripts/check-lockfile-drift.sh`) sees the new `Cargo.toml` entry
AND the matching `Cargo.lock` update — green. No new npm dependency
(the library is pure Rust); `tests/e2e/package-lock.json` stays
byte-stable.

**PASS.**

### Verdict

No principle is **VIOLATION**. The one IV baseline regeneration is a
routine intended change with a per-baseline note, not a widening.
The translation-completeness gate (Spec FR-006) is satisfied by
construction via the existing `cargo clippy -- -D warnings` invocation
— no new CI script. Proceed to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/005-i18n/
├── plan.md                  # This file
├── research.md              # Phase 0 — three resolved external decisions: (1) library pick (leptos_i18n v0.5.11 vs leptos-fluent — typed-key contract decides); (2) catalogue file format (JSON, library default); (3) missing-translation enforcement path (clippy -D warnings — no bespoke script)
├── data-model.md            # Phase 1 — Locale enum, AppearanceSettings.locale field, library-side i18n::Locale parallel enum, message-catalogue file layout, in-memory locale signal type, pure-function resolver helper
├── contracts/
│   └── components.md        # Phase 1 — four contracts: Locale wire shape; AppearanceSettings field evolution; leptos_i18n API surface (typed-key macro, locale-switcher API, OS-detection helper, locale-changed signal); translation-completeness invariant (FR contract)
├── checklists/              # Authored at /speckit-specify (already present)
├── quickstart.md            # Phase 1 — contributor's path: local build, where catalogues live, how to add a key, how to add a locale, how to run translation-completeness check, how to switch locale in the running app
└── tasks.md                 # Phase 2 — generated by /speckit-tasks (NOT this command)
```

### Source Code (new and touched paths)

```text
crates/presto-ipc/src/
└── settings.rs                                    # + pub enum Locale { En, De, It, Tr } with #[serde(rename_all = "lowercase")];
                                                   # + locale: Option<Locale> on AppearanceSettings (#[serde(default)]); + 3 tests
                                                   # (locale_legacy_field_defaults_none, locale_round_trip,
                                                   # locale_serialises_lowercase)

src/Cargo.toml                                     # + leptos_i18n = { version = "=0.5.11", default-features = false,
                                                   #   features = ["csr", "json_files", "icu_compiled_data"] }
                                                   # + [package.metadata.leptos-i18n] block: default = "en",
                                                   #   locales = ["en", "de", "it", "tr"]

src/Cargo.lock                                     # REGENERATED. ~12-18 new crates from leptos_i18n transitives
                                                   # (leptos-use, icu_locid, codee, typed-builder, etc.). Staged
                                                   # in the same commit as Cargo.toml (Principle IX).

src/src/i18n.rs                                    # NEW. Houses:
                                                   # (a) `leptos_i18n::load_locales!();` invocation at module top,
                                                   #     which expands to a child `i18n` module with the
                                                   #     library-generated Locale enum + I18nKeys + I18nContext.
                                                   # (b) From<presto_ipc::Locale> for i18n::Locale (and inverse) impls.
                                                   # (c) Pure-fn `resolve_initial_locale(persisted, os_languages) -> Locale`.
                                                   # (d) Helper `compute_initial_library_locale(&Settings) -> Option<i18n::Locale>`
                                                   #     for boot-time wiring (returns None when persisted is default,
                                                   #     so the library's OS-detection path runs).
                                                   # (e) `match_two_letter_prefix(&str) -> Option<Locale>` helper.
                                                   # (f) 5 RED-first tests for the resolver.

src/src/lib.rs                                     # + pub mod i18n;

src/src/app.rs                                     # Boot-time wiring: at the existing settings-load spawn_local
                                                   # block (~ line 200-215), pass `compute_initial_library_locale(
                                                   # &loaded)` to <I18nContextProvider initial_locale=...>. The
                                                   # provider wraps the existing app tree. Add a small Effect
                                                   # that watches `settings.appearance.locale` and forwards changes
                                                   # to `i18n.set_locale(...)` (forwarding direction only — the
                                                   # dropdown writes to the IPC signal).

src/src/components/settings/general.rs             # + new <select id="locale-selector"> control row placed ABOVE
                                                   # the existing "Timer Durations" section (i.e. above the
                                                   # #focus-duration field). Four <option> entries with native
                                                   # self-names in fixed order: English / Deutsch / Italiano /
                                                   # Türkçe. The on:change handler updates
                                                   # settings.appearance.locale; the existing debounced autosave
                                                   # Effect at src/src/app.rs:215+ persists. The surrounding
                                                   # "Language" label is `t!(i18n, settings.general.language_label)`
                                                   # so it localises with the rest of the UI.

src/locales/                                       # NEW directory. Four hand-curated JSON catalogues:
├── en.json                                        # Source-of-truth catalogue (Spec A13). Holds every key in
                                                   # FR-013 scope. Exact contents are a tasks-phase concern; the
                                                   # plan only commits to the catalogue scaffolding existing.
├── de.json                                        # German translation.
├── it.json                                        # Italian translation.
└── tr.json                                        # Turkish translation.

# Every component file carrying FR-013-scope user-visible English strings gets a
# bulk string-extraction pass — replacing literal `"..."` string nodes in `view!`
# macros with `{t!(i18n, namespace.key)}` typed-key call sites. The list is
# exhaustive per FR-013 / FR-014:

src/src/components/timer/mod.rs                    # Timer-screen strings: mode badges (Focus / Break / Long Break),
                                                   # state suffixes (Paused / Auto-paused / Overtime), control
                                                   # button labels (Reset / Undo / Start / Pause / Resume /
                                                   # Skip session), corresponding aria-labels (each its own key
                                                   # per Spec FR-013 / A11).

src/src/components/timer/messages.rs               # Timer-screen toast / message strings (if any user-visible
                                                   # strings — actual key set is tasks-phase).

src/src/components/timer/tag_tracking.rs           # Tag-picker placeholder "New tag..." + "Choose tag" header
                                                   # (FR-013 — tag picker / manager).

src/src/components/settings/notifications.rs       # Every setting's label, helper text, unit suffix; the
                                                   # auto-save toast "Settings saved" / "Failed to save settings"
                                                   # (shared with the other settings tabs via the SettingsToast
                                                   # type).

src/src/components/settings/shortcuts.rs           # Shortcuts tab labels + helper text.

src/src/components/settings/theme.rs               # Theme tab labels + helper text.

src/src/components/settings/automation.rs          # Automation tab labels + helper text.

src/src/components/settings/goals.rs               # Goals tab labels + helper text.

src/src/components/settings/advanced.rs            # Advanced tab labels + helper text.

src/src/components/settings/updates.rs             # Updates tab labels + helper text.

src/src/components/settings/mod.rs                 # The eight tab names (General / Shortcuts / Notifications /
                                                   # Theme / Automation / Goals / Advanced / Updates).

src/src/components/stats/period_selector.rs        # Statistics period tabs (Daily / Weekly / Monthly / Yearly).

src/src/components/stats/period_nav.rs             # Statistics period-navigation strings.

src/src/components/stats/focus_trend.rs            # Statistics tile labels + axis labels + "No data" empty-state.

src/src/components/stats/peak_focus_time.rs        # Statistics tile labels.

src/src/components/stats/monthly_peak_day.rs       # Statistics tile labels.

src/src/components/stats/tag_usage_pie.rs          # Statistics tile labels.

src/src/components/stats/bar_chart.rs              # Statistics axis labels (if any user-visible — most are dynamic
                                                   # chrono date strings, out of scope per FR-014 / A8).

src/src/components/stats/line_chart.rs             # Statistics axis labels (same — most are dynamic chrono).

src/src/components/stats/mod.rs                    # Statistics view header strings (if any).

src/src/components/daily/mod.rs                    # Daily-view header "Daily Overview", section header "Today's
                                                   # Sessions", empty-state "No sessions completed".

src/src/components/daily/sessions_history_table.rs # Table column headers + empty-state strings.

src/src/components/daily/sessions_timeline.rs      # Timeline labels / empty-state strings.

src/src/components/daily/month_grid.rs             # Calendar month names (January-December) + day-of-week
                                                   # headers (Sun-Sat). Per FR-013 / FR-025 / A8: month name
                                                   # strings ARE localised; the chrono-formatted numeric date
                                                   # parts (e.g. "13" for the day) stay English-formatted.

src/src/components/update_notification.rs          # Update notification strings: "Update available", surrounding
                                                   # version-display label, "Update via Homebrew" (or platform
                                                   # equivalent), "Skip release".

# Sidebar nav (the four tooltips Timer / Statistics / Daily / Settings) — the
# tooltip strings live in `src/src/app.rs` or a sidebar component module. The
# exact file path is tasks-phase to confirm; the FR-013 scope obligates them
# whichever module owns them.

src/src/app.rs                                     # ALSO: sidebar nav tooltip strings if they live here. Plus
                                                   # the I18nContextProvider mount described above.

tests/e2e/
├── settings-general.spec.js                       # + e2e flow exercising #locale-selector:
│                                                   #   (a) open Settings → General, assert the dropdown row is
│                                                   #       visible above #focus-duration,
│                                                   #   (b) pick `Deutsch`, assert the surrounding label flips to
│                                                   #       `Sprache`,
│                                                   #   (c) navigate to another settings tab, assert a known
│                                                   #       label (e.g. Notifications tab name) renders in German,
│                                                   #   (d) pick `English`, assert the original strings restore.
└── __screenshots__/visual-regression/
    └── settings-general-chromium-linux.png        # REGENERATED. One new control row above timer-durations.
```

**Structure Decision**: One new UI-side module (`src/src/i18n.rs`) is
the right place for the `load_locales!()` macro invocation and the
boot-time wiring because it cleanly separates the library integration
from the existing app body in `src/src/app.rs`. The IPC field
evolution lives entirely in the existing
`crates/presto-ipc/src/settings.rs` file; no new crate file is needed.
The locale-switcher UI sits in the existing
`src/src/components/settings/general.rs` (the General tab is where
appearance settings naturally live, alongside `theme` and
`timer_theme`).

## Modules

Terse change table.

| Path | Change |
|---|---|
| `crates/presto-ipc/src/settings.rs` | `+ pub enum Locale { En, De, It, Tr }` with `#[serde(rename_all = "lowercase")]`, `#[derive(Default)]`, `#[default]` on `En` (the enum variant default, used as the resolver fallback — NOT the field default). `+ pub locale: Option<Locale>` (`#[serde(default)]`) on `AppearanceSettings`. `Default` impl on `AppearanceSettings` adds `locale: None`. |
| `crates/presto-ipc/src/settings.rs::tests` | `+ locale_legacy_field_defaults_none` (legacy JSON without `locale` field → `None`; mirrors `ambient_sound_legacy_fields_default` at `:407-421`); `+ locale_round_trip` (each variant round-trips byte-stable, including `Some(En)` ↔ `"en"` distinct from `None`; existing `theme` + `timer_theme` preserved alongside); `+ locale_serialises_lowercase` (four-variant lowercase wire-shape assertion; `None` ↔ absent/null). |
| `src/Cargo.toml` | `+ leptos_i18n = { version = "=0.5.11", default-features = false, features = ["csr", "json_files", "icu_compiled_data"] }` (exact-pinned — only `0.5.11` verified against `leptos = "0.7"`; Fix C). `+ [package.metadata.leptos-i18n] default = "en", locales = ["en", "de", "it", "tr"]`. |
| `src/Cargo.lock` | REGENERATED in lockstep with the `Cargo.toml` change. Estimated ~12-18 new crates from `leptos_i18n` transitives. |
| `src/src/i18n.rs` | NEW. `leptos_i18n::load_locales!()` macro invocation at module top (expands to a child `i18n` module). `From<presto_ipc::Locale> for i18n::Locale` impl + inverse. Pure-fn `resolve_initial_locale`. Helper `compute_initial_library_locale`. Helper `match_two_letter_prefix`. Seven RED-first `cargo test` cases for the resolver (T008–T014). |
| `src/src/lib.rs` | `+ pub mod i18n;`. |
| `src/src/app.rs` | Boot path: pass `i18n::compute_initial_library_locale(&loaded)` to the new `<I18nContextProvider initial_locale=...>` wrapper around the existing app tree. New `Effect` watches `settings.appearance.locale` and forwards changes to the library's `i18n.set_locale(...)` (forwarding direction only). |
| `src/src/components/settings/general.rs` | `+ <select id="locale-selector">` control row placed ABOVE the existing `<h3>"Timer Durations"</h3>` block at `:124`. Four `<option>` entries: `English` / `Deutsch` / `Italiano` / `Türkçe` (fixed order, native self-names, never re-translated per FR-015). The surrounding label uses `t!(i18n, settings.general.language_label)`. `on:change` handler writes `settings.appearance.locale = match new_value { ... }` and calls `toast.show(...)` with the localised "Settings saved" key — same shape as the existing on-change handlers at `:78-114`. The existing five timer-duration input rows are unchanged. Every hard-coded English string in the existing view body gets a typed-key rewrite (the eight settings tab names, every setting's label, the helper text). |
| `src/src/components/settings/{shortcuts,notifications,theme,automation,goals,advanced,updates}.rs` | Each tab's hard-coded English strings rewrite to `t!(i18n, settings.<tab>.<key>)` typed-key call sites. The existing `SettingsToast` "Settings saved" / "Failed to save settings" calls swap their string literals for `t!(i18n, settings.auto_save_ok)` / `t!(i18n, settings.auto_save_err)`. Selector contracts (`#focus-duration`, etc.) are PRESERVED — only the visible text strings change. |
| `src/src/components/settings/mod.rs` | The eight tab names rewrite to typed keys. Selector contracts preserved. |
| `src/src/components/timer/{mod,messages,tag_tracking}.rs` | Every visible English string in the timer screen (mode badges, state suffixes, control button labels, corresponding aria-labels, tag-picker strings) rewrites to a typed-key call site. The metronome / ambient-audio side-effects are unaffected — those gate on `settings.notifications.metronome` / `settings.notifications.ambient_sound_*`, not on locale. |
| `src/src/components/stats/{period_selector,period_nav,focus_trend,peak_focus_time,monthly_peak_day,tag_usage_pie,bar_chart,line_chart,mod}.rs` | Period tabs, tile labels, axis labels (string ones — chrono ones stay), empty-state strings rewrite to typed keys. |
| `src/src/components/daily/{mod,sessions_history_table,sessions_timeline,month_grid,day_clamp}.rs` | Daily-view header, section header, table column headers, empty-state strings, calendar month names + day-of-week headers rewrite to typed keys. The chrono-formatted date strings stay English (FR-014 / FR-025 / A8). |
| `src/src/components/update_notification.rs` | Update notification strings (`Update available`, surrounding version-display label, platform-specific Update-via-Homebrew label, `Skip release`) rewrite to typed keys. Version-number literal stays as-is (FR-014). |
| `src/locales/en.json` | NEW. Source-of-truth catalogue. Exact key set is a tasks-phase concern; the plan commits to the file existing with the FR-013 scope filled in. |
| `src/locales/de.json` | NEW. German translation of every `en.json` key. Source-locale fallback at render time is ALLOWED only for any `(beta)`-marked locale per Spec User Story 5 / A9 — the v1 default is 100% coverage so the build-time check passes. |
| `src/locales/it.json` | NEW. Italian translation of every `en.json` key. |
| `src/locales/tr.json` | NEW. Turkish translation of every `en.json` key. |
| `tests/e2e/settings-general.spec.js` | `+` e2e flow exercising `#locale-selector` (open Settings → General, pick `Deutsch` from the dropdown, assert the surrounding label flips to `Sprache`, navigate to another tab, assert a known label renders in German, pick `English`, assert the original strings restore). |
| `tests/e2e/__screenshots__/visual-regression/settings-general-chromium-linux.png` | REGENERATED with one-line PR note: "settings-general: Language dropdown row added above the timer-durations section, four native-self-name options (English / Deutsch / Italiano / Türkçe). No other layout change." |

**[BEST-GUESS PM DECISION]** Sidebar nav tooltip strings (`Timer`,
`Statistics`, `Daily`, `Settings` — per Spec FR-013) live in
`src/src/app.rs` or a small sidebar component module — confirmed at
tasks-phase. The plan commits to localising them whichever file
houses them.

**[BEST-GUESS PM DECISION]** The `<select>` element's option labels
in the locale-switcher are hard-coded as plain Rust string literals
(not `t!(...)` call sites) because the spec EXPLICITLY mandates the
option labels stay native self-names regardless of active locale
(FR-015 / Spec Story 1 AC 4). The surrounding label `"Language" /
"Sprache" / "Lingua" / "Dil"` IS localised.

## Testing strategy and test-first markers

Per Principle V scope (IPC wire-shape evolution is the persistence
boundary), and per Spec FR-022 / FR-023, **seven RED-first resolver tests**
(T008–T014) plus three IPC tests (T004–T006) precede implementation:

| Module | Test runner | Test-first? | Notes |
|---|---|---|---|
| `presto_ipc::settings::tests::locale_legacy_field_defaults_none` | `cargo test` (workspace) | **YES (RED-first)** `[test-first]` | Mirrors `ambient_sound_legacy_fields_default` at `crates/presto-ipc/src/settings.rs:407-421`. Legacy `AppearanceSettings` JSON lacking the `locale` key deserialises to `None` (no explicit locale — Fix A). SC-002. |
| `presto_ipc::settings::tests::locale_round_trip` | `cargo test` | **YES (RED-first)** `[test-first]` | Each variant round-trips byte-stable, including `Some(En)` ↔ `"en"` (Fix A: explicit English MUST NOT round-trip as `None`). The same fixtures also carry `theme = "auto"` + `timer_theme = "espresso"` — covers Spec Story 2 AC 5. SC-003. |
| `presto_ipc::settings::tests::locale_serialises_lowercase` | `cargo test` | **YES (RED-first)** `[test-first]` | Four-variant wire-shape assertion: `Some(En) ↔ "en"`, `Some(De) ↔ "de"`, `Some(It) ↔ "it"`, `Some(Tr) ↔ "tr"`. Inverse direction asserted too. `None` ↔ absent/null also asserted. SC-003. |
| `presto_web::i18n::tests::resolve_initial_locale_persisted_some_wins` | `cargo test` | **YES (RED-first)** `[test-first]` | `(Some(De), ["en-US"]) → De`. `Some(_)` always wins over OS. Spec Story 4 AC 3 / FR-009 step 1 / FR-011. SC-010 branch 1. |
| `presto_web::i18n::tests::resolve_initial_locale_persisted_some_en_wins` | `cargo test` | **YES (RED-first)** `[test-first]` | **Fix A critical case**: `(Some(En), ["de-DE"]) → En`. Explicit English MUST NOT be overridden by a German OS locale. SC-010 branch 2. |
| `presto_web::i18n::tests::resolve_initial_locale_none_falls_to_os_de` | `cargo test` | **YES (RED-first)** `[test-first]` | `(None, ["de-DE"]) → De`. `None` falls through to OS detection. Spec Story 4 AC 1 / FR-009 step 2. SC-010 branch 3. |
| `presto_web::i18n::tests::resolve_initial_locale_none_swiss_german_matches_de` | `cargo test` | **YES (RED-first)** `[test-first]` | `(None, ["de-CH"]) → De`. Swiss German two-letter prefix `de` matches `Locale::De`. SC-010 branch 4. |
| `presto_web::i18n::tests::resolve_initial_locale_none_unsupported_falls_back_to_en` | `cargo test` | **YES (RED-first)** `[test-first]` | `(None, ["fr-FR"]) → En`. Unsupported OS locale falls back. Spec Story 4 AC 2 / FR-009 step 3. SC-010 branch 5. |
| `presto_web::i18n::tests::resolve_initial_locale_none_empty_os_falls_back_to_en` | `cargo test` | **YES (RED-first)** `[test-first]` | `(None, []) → En`. No OS data — fallback. Spec Story 4 AC 5 / FR-010. SC-010 branch 6. |
| `presto_web::i18n::tests::resolve_initial_locale_none_first_match_wins` | `cargo test` | **YES (RED-first)** `[test-first]` | `(None, ["zh-CN", "ja-JP", "tr-TR"]) → Tr`. First matching supported prefix wins. SC-010 branch 7. |
| Settings UI (`#locale-selector` round-trip + locale-switch view re-render) | Playwright e2e | NO | UI plumbing — e2e + visual regression covers it. SC-001 / SC-007. |
| Visual-regression baseline regeneration | Playwright `toHaveScreenshot` | NO | One baseline (`settings-general-chromium-linux.png`); PR-time visual review against the per-baseline justification in §IV. SC-009. |
| Translation-completeness check (every `en.json` key exists in `de.json` / `it.json` / `tr.json`) | `cargo clippy -- -D warnings` | NO | Enforced by the existing CI clippy gate via the proc-macro's `MissingKey` → `#[deprecated]` → `-D warnings` chain. SC-006. No new test runner. |

**Mock-first ordering rule** (per Principle VI): **N/A this feature.**
No new Tauri commands; the mock-drift gate stays green without
modifications — verified against `tests/e2e/fixtures/tauriMock.js`.

## CI gates

Reference `.agentex.yml` (post-004 stage definitions). All gates
already exist; this feature interacts with seven of them.

### Mock-drift gate — `scripts/check-mock-drift.sh`

**No action needed.** No new `#[tauri::command]` handlers, no new
mock cases. Run as a sanity check; expect green.

### Engine-purity gate — `scripts/check-engine-purity.sh`

**Stays green by construction.** All new code lives under
`src/src/components/`, `src/src/app.rs`, `src/src/i18n.rs`, and
`crates/presto-ipc/`; nothing touches `src/src/engine/`. Zero new
`web_sys` references under the engine path.

### Strict static analysis — `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` + `cargo fmt --check`

**Load-bearing.** New code in `src/src/i18n.rs` and the rewrites in
every component file land clippy-pedantic-clean. The new
`presto_ipc::Locale` enum derives `Default` / `Debug` / `Clone` /
`Copy` / `PartialEq` / `Eq` / `Serialize` / `Deserialize` — no
`#[allow(...)]` needed. The library's macro-generated code (the
`load_locales!()` output) MUST also pass pedantic per Spec A16; if
the macro's output triggers any specific lint, the plan adds the
narrowly-scoped `#[allow]` at the macro call site in `src/src/i18n.rs`
with an inline justification (NOT a blanket workspace-level
allowance). **[BEST-GUESS PM DECISION]** This is a tasks-phase
discovery — if a specific lint fires, it gets a targeted fix.

**Translation-completeness check piggy-backs here.** The proc-macro
emits `#[deprecated(note = "Missing key ...")]` annotations on a
generated `warnings()` fn; the `-D warnings` flag promotes the
`deprecated` lint to a hard build error. **No extra CI step.** This
satisfies Spec FR-006 by construction.

### Strict static analysis: `clippy::pedantic` + the `nursery` and `all` workspace-lints

The workspace's `[lints.clippy]` block at the repo root sets `all =
"deny"` and `nursery = "deny"` (per `Cargo.toml`:23-24). Both apply
to new code in this feature. The new pure-fn `resolve_initial_locale`
and `match_two_letter_prefix` are small enough that pedantic should
be clean by default; the `Locale` enum's derive set should not
trigger nursery lints.

### `wasm-bindgen-test` + `wasm-pack test --node`

**Not load-bearing for this feature.** The seven RED-first resolver
tests run under `cargo test --workspace --frozen` (pure functions,
no DOM). The Settings UI plumbing is e2e-covered, not wasm-bindgen-
covered. **N/A.**

### Playwright e2e + visual regression

**One baseline regenerates.** `tests/e2e/settings-general.spec.js`
gains an e2e flow exercising the `#locale-selector` round-trip and
the locale-switch view re-render assertion.
`tests/e2e/__screenshots__/visual-regression/settings-general-chromium-linux.png`
regenerates with the per-baseline note from §IV. Other baselines
(timer, statistics-*, daily, tag-manager, update-notification,
settings-* for the other tabs) stay byte-stable — any diff on those
is a regression to fix in code, not absorbed into the baseline (FR-021).

### Lockfile-drift gate

**Load-bearing.** ONE new runtime dependency (`leptos_i18n`); both
`src/Cargo.toml` and `src/Cargo.lock` MUST update together (Principle
IX / Spec FR-020). The gate at `scripts/check-lockfile-drift.sh` sees
the `Cargo.toml` manifest change and the matching `Cargo.lock`
update — green when staged together. **NO npm dependency added** —
the library is pure Rust; `tests/e2e/package-lock.json` stays
byte-stable.

### Baseline-cap gate

**Stays at the default (≤2 baselines).** One baseline regenerates;
under the cap. The feature-003 carve-out (BASELINE_CAP=17 on
`003-*` and `ui-parity-port` branches) does NOT apply to `005-i18n`
— the per-`.agentex.yml`-lint logic gates on branch name pattern.

## Implementation phasing

Six phases. Phase 0 sets up the dep + the catalogue scaffolding
(pre-flight); Phase 1 adds the `Locale` enum + `AppearanceSettings`
field test-first (IPC); Phase 2 writes the resolver test-first
(pure-function unit); Phase 3 wires the library at boot
(`<I18nContextProvider>` + the forwarding Effect); Phase 4 implements
the Settings → General locale switcher UI; Phase 5 sweeps every
component file for the bulk string-extraction pass; Phase 6 regenerates
the baseline and runs the final gate sweep.

### Phase 0 — Pre-flight: dep add + catalogue scaffolding

**Entry**: clean branch `005-i18n` post-spec.
**Exit**: `src/Cargo.toml` carries the new
`leptos_i18n = "=0.5.11"` line (exact-pinned — Fix C) and the
`[package.metadata.leptos-i18n]` block. `src/Cargo.lock` is
regenerated. Four empty-shell JSON catalogues exist at
`src/locales/{en,de,it,tr}.json` (each one a `{}` placeholder — the
proc-macro accepts an empty catalogue at this phase). `cargo build
--workspace --frozen` exits zero. No new tests yet — Phase 1 adds
them.

**Missing-key gate verification (Fix D — must complete before Phase 0 closes)**:

Before moving to Phase 1, verify the proc-macro's `MissingKey` lint path
is live and will fail CI on missing keys:

1. Add one key to `src/locales/en.json`, e.g. `{ "probe": "probe" }`.
2. Add that same key to `de.json`, `it.json`, but deliberately OMIT it
   from `tr.json`.
3. Run `cargo clippy --workspace --all-targets --frozen -- -D warnings`.
4. **Expected**: non-zero exit with a `deprecated` lint citing the missing
   key in `tr`. If this happens, the proc-macro path is live — revert the
   deliberate omission and proceed.
5. **If clippy exits zero** (the proc-macro path is leaky): add a
   `check-translation-completeness.sh` script to the repo's `scripts/`
   directory and register it in `.agentex.yml`'s `lint:` block as a
   backup gate (per FR-006's CI-enforcement clause). Document the leaky
   path finding here. Only then proceed to Phase 1.

**Test-first**: N/A (pre-flight), except for the Fix D gate verification above.

### Phase 1 — IPC widening: `Locale` enum + `AppearanceSettings.locale` (test-first)

**Entry**: Phase 0 complete.
**Exit**: `crates/presto-ipc/src/settings.rs` defines `pub enum
Locale { En, De, It, Tr }` with `#[serde(rename_all = "lowercase")]`
+ `#[derive(Default)]` + `#[default]` on `En`. `AppearanceSettings`
gains `pub locale: Option<Locale>` with `#[serde(default)]` (default
`None`); the `Default` impl returns `locale: None`. Three new test
cases in `presto_ipc::settings::tests` pass, including a critical
invariant test that `Some(Locale::En)` and `None` round-trip as distinct
values (Fix A).
**Test-first**: YES per Principle V (wire-shape contract).
- **Test-first commit ordering** (AGENTS.md §Test-first commit
  ordering, Principle V): three RED commits land first (each one
  failing test in isolation; `cargo test --workspace --frozen`
  exits non-zero on the new asserts). The GREEN commit follows in
  a separate commit (enum + field + Default impl land; `cargo test
  --workspace --frozen` exits zero). The pairs are NOT collapsed.

### Phase 2 — Locale resolver: pure-function unit (test-first)

**Entry**: Phase 1 complete (IPC `Locale` enum exists).
**Exit**: `src/src/i18n.rs` exists with:
- `leptos_i18n::load_locales!()` macro invocation at module top
  (compiles against the four empty-shell catalogues from Phase 0).
- `From<presto_ipc::Locale> for i18n::Locale` + inverse impls.
- Pure-fn `resolve_initial_locale(persisted, os_languages)` and
  `match_two_letter_prefix(lang)`.
- `compute_initial_library_locale(&Settings) -> Option<i18n::Locale>`.
- Seven test cases (RED-first; T008–T014).
`src/src/lib.rs` gains `pub mod i18n;`. `cargo test --workspace
--frozen` exits zero with all seven resolver tests green.
**Test-first**: YES per Spec FR-023 (the resolver's pure-function
testability is explicitly required by the spec).
- **Test-first commit ordering**: seven RED commits (T008–T014)
  precede the GREEN commit (T015); pairs NOT collapsed.

### Phase 3 — Boot-time wiring: `<I18nContextProvider>` + forwarding Effect

**Entry**: Phases 0-2 complete.
**Exit**: `src/src/app.rs` wraps the existing app tree in
`<I18nContextProvider initial_locale=...>` (the `initial_locale` value
comes from `i18n::compute_initial_library_locale(&loaded)`). A new
`Effect` watches `settings.appearance.locale` and forwards changes to
`i18n.set_locale(...)`. The change handler in
`src/src/components/settings/general.rs` is NOT yet added — Phase 4
does that. At this phase, the locale is read from `settings.json` at
boot but the user can't yet pick a different one through the UI; the
typed-key call sites in components don't yet exist either (Phase 5
does that). `cargo build --workspace --frozen` exits zero;
`cargo test --workspace --frozen` exits zero; `trunk build --release`
exits zero. The UI still renders in English by default because no
component file has yet been rewritten to call `t!(...)`.
**Test-first**: NO (UI plumbing — e2e covers the boot-time path).

### Phase 4 — Settings UI: locale-switcher

**Entry**: Phase 3 complete.
**Exit**: `src/src/components/settings/general.rs` gains the new
`<select id="locale-selector">` control row above the existing
`<h3>"Timer Durations"</h3>` section. Four `<option>` entries
(`English` / `Deutsch` / `Italiano` / `Türkçe`, fixed order, native
self-names hard-coded). `on:change` handler writes
`settings.appearance.locale = match value { ... }`. The dropdown's
selected option reflects `settings.appearance.locale` reactively.
`tests/e2e/settings-general.spec.js` evolves with an e2e flow
exercising the new selector. Visual regression baseline is NOT yet
regenerated — Phase 6 does that after the bulk string-extraction
pass lands.
**Test-first**: NO (UI plumbing).

### Phase 5 — Bulk string extraction (every component file in FR-013 scope)

**Entry**: Phase 4 complete.
**Exit**: Every hard-coded English string in the FR-013 scope is
moved into `src/locales/en.json` and replaced at the call site with
a `t!(i18n, namespace.key)` typed-key macro call. The three target
catalogues (`de.json` / `it.json` / `tr.json`) carry the same key
set with hand-curated translations (per FR-029, no machine
translation; the translation pass is a contributor exercise).
`cargo clippy --workspace --all-targets --frozen -- -D warnings -W
clippy::pedantic` exits zero (no `MissingKey` warnings; no
`SurplusKey` warnings — symmetry honoured per A13). The e2e flow
from Phase 4 is extended to assert non-Settings strings flip too
(e.g. a known label in another tab renders in German after the
locale switch). UI renders in the picked locale across every view.
**Test-first**: NO (UI plumbing — e2e + translation-completeness
gate cover it).

### Phase 6 — Visual-regression baseline regen + final gate sweep

**Entry**: Phases 0-5 complete.
**Exit**:
`tests/e2e/__screenshots__/visual-regression/settings-general-chromium-linux.png`
is regenerated locally via `npx playwright test
tests/e2e/visual-regression.spec.js --update-snapshots`, reviewed
visually against the §IV per-baseline justification, and committed
in a single commit. Full gate sweep exits 0. The PR description
restates the per-baseline note verbatim.
**Test-first**: N/A (visual gate is itself the test).

## Post-design Constitution Check

Re-checked after Phase 1 design (research.md, data-model.md,
contracts/components.md, quickstart.md). Verdicts unchanged from
§[Constitution Check](#constitution-check). Material principles
re-affirmed:

- **II**: research.md Decision 1 confirms the `leptos_i18n` library
  bundles all four catalogues into the WASM binary at compile time
  via the `load_locales!()` proc-macro's `include_str!`-style read;
  no CDN, no runtime fetch, no network egress.
- **III**: contracts/components.md §3 restates the typed-key macro
  contract — `t!(i18n, ...)` accepts a typed Rust identifier path,
  not a string literal. A typo fails `cargo build`. SC-005's
  zero-stringly-typed-lookups posture is satisfied by library
  construction.
- **IV**: §[Constitution Check IV](#iv-visual-regression-is-the-ui-contract--one-baseline)
  pre-anchors the one per-baseline justification. quickstart.md
  lists the verbatim text for copy-paste into the PR description.
- **V**: contracts/components.md §4 confirms the translation-
  completeness invariant is enforced via the proc-macro's
  `#[deprecated]` warning emission, promoted to a hard failure by
  the existing `cargo clippy -- -D warnings` gate — no extra CI
  step, no test runner addition.
- **VI**: contracts/components.md explicitly states "no new Tauri
  commands"; the mock-drift gate stays green without changes.
- **IX**: research.md Decision 1 confirms `leptos_i18n` is the ONE
  new runtime dep; `Cargo.lock` updates lockstep with `Cargo.toml`.

## Complexity Tracking

> No Constitution Check violations require justification. The one
> IV baseline regeneration is a routine intended change (Principle
> IV's documented "intended change + one-line note" mechanism), not
> a widening. The one new runtime dependency (`leptos_i18n`) is
> covered by Principle IX's documented "manifest + lockfile in
> lockstep" mechanism, not a widening.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| (none) | — | — |

### Bus-factor / version-pin risk (Fix F)

`leptos_i18n` is single-maintainer (`@Baptistemontan`, ~158 stars as of
research.md Decision 1). The `0.5.x` line is captive to `leptos = "0.7"`.
When a future presto leptos-upgrade spec bumps to `leptos = "0.8"`,
`leptos_i18n` MUST bump in lockstep (`0.6.x` for leptos 0.8, `0.7.x` for
leptos 0.9, etc. — track the README compatibility table). Track via a
follow-up issue tied to the leptos-upgrade cycle. The exact-pin
`version = "=0.5.11"` (Fix C) mitigates accidental Cargo resolver drift
within the `0.5.x` series.

### Pluralization audit (Fix G)

Before Phase 5 (bulk string extraction) the implementor MUST run:

```bash
grep -rn 'format!.*session\|format!.*min\b' src/src/
```

to surface count-bearing strings in scope. Likely candidates per FR-013:

- Statistics tiles: "X sessions", "Y minutes" — if present as `format!`
  calls, these are plural-sensitive.
- Daily view: "No sessions completed" (zero-form) vs "1 session" vs
  "N sessions" — if the current code branches on count.
- Update notification: likely "Update available" (singular, no plural).
- Settings labels: stable phrases, no plurals expected.

**If any plural-sensitive strings exist**:
- Enable the `plurals` cargo feature on `leptos_i18n` in `src/Cargo.toml`.
- Add plural-rule-aware keys to the catalogue (ICU CLDR syntax via the
  `icu_plurals` crate's data tables, available via the existing
  `icu_compiled_data` feature already in the dep).
- Document the specific keys and their plural rules here.

**If zero plural-sensitive strings exist** (all count-sensitive English
already uses the "Sessions: 5" rewriting pattern per Spec Edge Cases
bullet 5):
- Document the audit result here and leave `plurals` OFF.
- The `icu_compiled_data` feature remains (already in the dep declaration)
  for a future follow-up that enables plurals.

**[ACTION REQUIRED at Phase 5 entry]**: run the grep, fill in findings here.
