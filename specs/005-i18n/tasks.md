# Tasks: Multi-Locale UI With In-App Language Switcher

**Input**: Design documents in `specs/005-i18n/`
**Prerequisites**: spec.md (5 user stories, 30 FRs, 13 SCs, 16 Assumptions), plan.md (6 phases), data-model.md, contracts/components.md, research.md, quickstart.md

## Format

`- [ ] [TID] [P?] [US?] Description with file path` — User stories: **US1** = in-app locale switching (FR-007, FR-012–013, FR-015–016, SC-001, SC-004–005, SC-007, SC-013), **US2** = legacy settings compatibility (FR-001–004, SC-002–003), **US3** = visual regression baseline (FR-021, SC-009), **US4** = OS-locale detection (FR-009–011, SC-010), **US5** = translation-coverage beta indicator (FR-006). `[P]` = parallelisable with other `[P]` tasks in the same phase. Each task lists its **Done-signal** and **Files**. Test-first tasks carry explicit **RED** / **GREEN** commit-boundary labels (NOT collapsed — separate commits mandatory per AGENTS.md §Test-first commit ordering).

---

## Phase 0 — Pre-flight: dep add + catalogue scaffolding + missing-key gate verification

**Goal**: Add `leptos_i18n = "=0.5.11"` (exact-pinned) to `src/Cargo.toml`, create four empty JSON catalogue skeletons under `src/locales/`, and verify the proc-macro's `MissingKey` lint path is live and will fail CI on missing keys. No tests yet — that's Phase 1.

**Exit**: `src/Cargo.toml` carries the new `leptos_i18n` entry and `[package.metadata.leptos-i18n]` block. `src/Cargo.lock` is regenerated. Four empty-shell JSON catalogues exist at `src/locales/{en,de,it,tr}.json`. The missing-key gate is verified live. `cargo build --workspace --frozen` exits zero. `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` exits zero.

- [ ] T001 Add `leptos_i18n = "=0.5.11"` dependency and `[package.metadata.leptos-i18n]` block to `src/Cargo.toml`; regenerate `src/Cargo.lock` in lockstep
  - **Files**: `src/Cargo.toml`, `src/Cargo.lock`
  - **Change** (per plan.md §Summary and §IX, research.md Decision 1, contracts/components.md §3): add to `[dependencies]` block: `leptos_i18n = { version = "=0.5.11", default-features = false, features = ["csr", "json_files", "icu_compiled_data"] }`. Add at the end of `src/Cargo.toml`: `[package.metadata.leptos-i18n]`, `default = "en"`, `locales = ["en", "de", "it", "tr"]`. Version is **exact-pinned** (`=0.5.11`) — only this version is verified against `leptos = "0.7"` (Fix C). Then run `cargo fetch` or `cargo build` (without `--frozen`) to regenerate `Cargo.lock`. The `cookie` default feature is deliberately dropped via `default-features = false` (presto persists locale via `settings.appearance.locale`, not the library's `lf-lang` cookie — contracts/components.md §3 feature table). No npm change; `tests/e2e/package-lock.json` stays byte-stable.
  - **Done-signal**: `grep 'leptos_i18n' src/Cargo.toml` returns the `=0.5.11` exact-pinned entry. `grep 'leptos-i18n' src/Cargo.toml` returns the `[package.metadata.leptos-i18n]` block with `default = "en"` and the four locales. `cargo build --workspace --frozen` exits zero after lockfile is staged. `git diff tests/e2e/package-lock.json` returns zero lines.

- [ ] T002 Create `src/locales/` directory with four empty-shell JSON catalogue files (`en.json`, `de.json`, `it.json`, `tr.json`)
  - **Files**: `src/locales/en.json`, `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json` (new directory tree)
  - **Change** (per plan.md §Phase 0, data-model.md §3): create `src/locales/en.json` with content `{}`. Create `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json` each with content `{}`. The `leptos_i18n::load_locales!()` proc-macro accepts an empty catalogue at this stage — the proc-macro reads from `<crate-root>/locales/<locale>.json`, which for the presto frontend at `src/Cargo.toml` expands to `src/locales/{en,de,it,tr}.json`. The files are not served; they are compile-time embedded (FR-008 / FR-019 / Principle II).
  - **Done-signal**: `ls src/locales/*.json | wc -l` returns 4. Each file contains only `{}`. `cargo build --workspace --frozen` exits zero (proc-macro reads four empty catalogues). `dist/` tree does NOT contain a `dist/locales/` subdirectory after `trunk build` (catalogues are compiled in, not served).
  - **BlockedBy**: T001.

- [ ] T003 Verify the `leptos_i18n` proc-macro missing-key gate (Fix D): inject a deliberate missing key, run `cargo clippy --workspace --all-targets --frozen -- -D warnings`, confirm non-zero exit, then revert
  - **Files**: `src/locales/en.json`, `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json` (temporary probe; reverted)
  - **Procedure** (per plan.md §Phase 0 "Missing-key gate verification"):
    1. Edit `src/locales/en.json` to `{ "probe": "probe" }`. Edit `de.json` and `it.json` identically. Leave `tr.json` as `{}` (deliberate omission).
    2. Run `cargo clippy --workspace --all-targets --frozen -- -D warnings`. Expected: **non-zero exit** with a `deprecated` lint citing the missing key in `tr`. The error MUST cite `"probe"` and locale `"tr"`.
    3. If clippy exits **non-zero** as expected: proc-macro gate is live. Revert all four files to `{}`. Proceed.
    4. If clippy exits **zero** (the proc-macro path is leaky): create `scripts/check-translation-completeness.sh` (parses each `locales/*.json`, computes key-set diff vs `en.json`, fails loudly on any missing key). Register it in `.agentex.yml`'s `lint:` block. Document the leaky-path finding in a comment in `src/src/i18n.rs`. Revert the four files to `{}` and proceed.
  - **Done-signal**: The probe test exits non-zero citing the missing key (gate live) OR a backup `check-translation-completeness.sh` is registered and passing (fallback gate). All four catalogue files are reverted to `{}` before this task closes. `cargo build --workspace --frozen` exits zero on the clean state.
  - **BlockedBy**: T002.

**Phase 0 exit**: `cargo build --workspace --frozen` exits zero. `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` exits zero. `src/locales/` has four `{}` JSON files. Missing-key gate verified live. `Cargo.lock` staged alongside `Cargo.toml` change (Principle IX / Spec FR-020).

---

## Phase 1 — IPC widening: `Locale` enum + `AppearanceSettings.locale` [test-first]

**Goal**: Add `pub enum Locale` and `pub locale: Option<Locale>` to `crates/presto-ipc/src/settings.rs`, test-first per Principle V. The `Option<Locale>` discriminant (Fix A) is the critical invariant: `Some(Locale::En)` (explicit English) and `None` (no choice yet) are NOT equivalent. Three RED commits precede one GREEN commit; pairs are NOT collapsed.

**Exit**: Three new tests in `presto_ipc::settings::tests` pass. `cargo test --workspace --frozen -p presto-ipc settings::tests` green. No UI or i18n module code yet — that's Phase 2.

### [US2] Test-first triplet — legacy compat + wire shape

- [ ] T004 [US2] **[test-first RED]** Write failing `presto_ipc::settings::tests::locale_legacy_field_defaults_none` in `crates/presto-ipc/src/settings.rs`
  - **Files**: `crates/presto-ipc/src/settings.rs` (test module)
  - **Test body** (per plan.md §V, contracts/components.md §2 "Legacy fixture round-trip", SC-002): deserialise the pre-feature-005 `AppearanceSettings` JSON fixture `{ "theme": "auto", "timer_theme": "espresso" }` (no `locale` key — the feature 004 baseline shape). Assert `locale == None`. Assert `theme == "auto"` and `timer_theme == "espresso"` are preserved (covers Spec Story 2 AC 5). The critical invariant: `None` MUST NOT equal `Some(Locale::En)` — assert `appearance.locale != Some(Locale::En)`. Mirrors `ambient_sound_legacy_fields_default` at `:407-421`. Commit the failing test separately from implementation.
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc settings::tests::locale_legacy_field_defaults_none` exits **non-zero** (compile-fail referencing undefined `Locale` enum or missing `locale` field on `AppearanceSettings`). The test body references `Locale` and `appearance.locale` directly so the build fails on missing symbols — NOT a `todo!()` or `assert!(false)` placeholder. Committed separately from T007 (GREEN).

- [ ] T005 [US2] **[test-first RED]** Write failing `presto_ipc::settings::tests::locale_round_trip` in `crates/presto-ipc/src/settings.rs`
  - **Files**: `crates/presto-ipc/src/settings.rs` (test module)
  - **Test body** (per plan.md §V, contracts/components.md §2 "Non-default round-trip", SC-003): for each of the three non-default locales (`De`, `It`, `Tr`), construct a `AppearanceSettings { theme: "auto".into(), timer_theme: "espresso".into(), locale: Some(Locale::De) }` (etc.), serialise via `serde_json::to_string`, deserialise back, assert round-trip byte-stable. ALSO: for `Some(Locale::En)` — construct with `locale: Some(Locale::En)`, serialise, deserialise, assert result is `Some(Locale::En)` NOT `None` (Fix A critical invariant). Assert the existing `theme` and `timer_theme` fields survive each round-trip alongside the new `locale` field (Spec Story 2 AC 5 / SC-003). Separate commit from T004.
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc settings::tests::locale_round_trip` exits **non-zero** (compile-fail). Separate commit from T004. The test body references `Locale::De`, `Locale::En`, etc. so the build fails on missing symbols.
  - **BlockedBy**: T004.

- [ ] T006 [US2] **[test-first RED]** Write failing `presto_ipc::settings::tests::locale_serialises_lowercase` in `crates/presto-ipc/src/settings.rs`
  - **Files**: `crates/presto-ipc/src/settings.rs` (test module)
  - **Test body** (per plan.md §V, contracts/components.md §1 "Wire-shape assertion table", SC-003): enumerate all four `Locale` variants explicitly in both directions: `Locale::En` → serialises to `"en"`, `"en"` deserialises to `Locale::En`; `Locale::De` ↔ `"de"`; `Locale::It` ↔ `"it"`; `Locale::Tr` ↔ `"tr"`. ALSO assert the out-of-set path: `"fr"` deserialises via `Option<Locale>` with `#[serde(default)]` to `None` (Spec FR-004 / Story 2 AC 4). ALSO assert `None` encodes as absent/null on the wire and re-reads as `None`. Separate commit from T005.
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc settings::tests::locale_serialises_lowercase` exits **non-zero** (compile-fail). Separate commit from T005. References `Locale::En`, `Locale::De`, `Locale::It`, `Locale::Tr` directly.
  - **BlockedBy**: T005.

### [US2] Implementation GREEN: `Locale` enum + `AppearanceSettings` field

- [ ] T007 [US2] **[test-first GREEN]** Implement `pub enum Locale` and add `locale: Option<Locale>` to `AppearanceSettings` in `crates/presto-ipc/src/settings.rs`
  - **Files**: `crates/presto-ipc/src/settings.rs`
  - **Changes** (per data-model.md §1 "Locale", §2 "AppearanceSettings evolution", contracts/components.md §1, plan.md §Modules table):
    1. Add `pub enum Locale` with `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]`, `#[cfg_attr(feature = "specta", derive(specta::Type))]`, `#[serde(rename_all = "lowercase")]`; variants: `#[default] En`, `De`, `It`, `Tr`. The `#[default]` on `En` provides the default value used by `#[serde(default)]` on the `locale` field (Spec FR-002 / FR-003). Wire shape matches the existing `theme` field's lowercase string convention at `:121-123` (`"auto"` / `"light"` / `"dark"`), NOT the kebab-case `AmbientSoundType` convention.
    2. Add to `AppearanceSettings` struct (after `timer_theme` field): `#[serde(default)] pub locale: Option<Locale>`. The field type is `Option<Locale>` NOT `Locale` — this is Fix A (data-model.md Fix A rationale): `None` = "no explicit locale chosen" (legacy records, fresh install); `Some(Locale)` = "user explicitly saved this locale" (including English). The `Option` discriminant is the resolver's authoritative "explicit vs. default" signal per FR-009 / FR-011.
    3. Update `Default for AppearanceSettings` to include `locale: None` in the returned `Self`.
    4. No new `#[allow(...)]` annotations needed.
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc settings::tests::locale_legacy_field_defaults_none` AND `locale_round_trip` AND `locale_serialises_lowercase` all pass. `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green. Committed separately from T004/T005/T006. `grep 'pub enum Locale' crates/presto-ipc/src/settings.rs` returns a hit. `grep 'pub locale: Option<Locale>' crates/presto-ipc/src/settings.rs` returns a hit.
  - **BlockedBy**: T006.

**Phase 1 exit**: `cargo test --workspace --frozen` green (3 new tests + all pre-existing). `cargo clippy` + `cargo fmt --check` green. The three RED commits and the single GREEN commit exist as four separate git commits (NOT collapsed). No UI or i18n module code yet.

---

## Phase 2 — Locale resolver: `src/src/i18n.rs` module [test-first]

**Goal**: Create `src/src/i18n.rs` with the `leptos_i18n::load_locales!()` invocation, `From` conversion impls, pure-fn `resolve_initial_locale`, and all seven RED-first resolver tests (T008–T014). Tests must precede implementation per Spec FR-023 / Principle V. Seven RED commits precede one GREEN commit; pairs are NOT collapsed.

**Exit**: `src/src/i18n.rs` exists with all helpers implemented. `src/src/lib.rs` has `pub mod i18n;`. All eleven resolver tests pass under `cargo test --workspace --frozen`. `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green.

### [US4] Test-first: locale-resolution precedence chain (FR-023)

Each of the following RED tests covers one branch of FR-009's strict precedence chain. Each is a separate commit.

- [ ] T008 [US4] **[test-first RED]** Write failing `presto_web::i18n::tests::resolve_initial_locale_persisted_some_wins` in `src/src/i18n.rs`
  - **Files**: `src/src/i18n.rs` (new file — create the file with the failing test only; no implementation yet)
  - **Test body** (per plan.md §V table row 4, contracts/components.md §3 "Pure-function locale-resolution helper" test 1, SC-010 branch 1): call `resolve_initial_locale(Some(presto_ipc::Locale::De), &["en-US".to_string()])` and assert the result is `presto_ipc::Locale::De`. Any `Some(_)` wins over OS detection — even when the OS locale is a supported language. Note: `resolve_initial_locale` takes `os_languages: &[String]` (or `impl Iterator<Item=&str>`; the exact signature is chosen by the implementor, consistent across all resolver test tasks). Create `src/src/lib.rs`'s `pub mod i18n;` entry in this commit so the test compiles (failing on the missing fn, not missing mod). Commit the failing test separately from T015 (GREEN).
  - **Done-signal**: `cargo test --workspace --frozen -p presto-web i18n::tests::resolve_initial_locale_persisted_some_wins` exits **non-zero** (compile-fail on missing `resolve_initial_locale` function). The test body references `resolve_initial_locale` directly so the build fails on missing symbol.

- [ ] T009 [US4] **[test-first RED]** Write failing `presto_web::i18n::tests::resolve_initial_locale_persisted_some_en_wins` in `src/src/i18n.rs`
  - **Files**: `src/src/i18n.rs` (test module)
  - **Test body** (per plan.md §V table row "Fix A critical case", contracts/components.md §3 test 6, SC-010 branch 6): **Fix A critical case** — call `resolve_initial_locale(Some(presto_ipc::Locale::En), &["de-DE".to_string()])` and assert the result is `presto_ipc::Locale::En`. Explicit English (`Some(Locale::En)`) MUST NOT be overridden by a German OS locale. The `Some(_)` discriminant is the authoritative signal, not value-equality against `Locale::En`. Separate commit from T008.
  - **Done-signal**: `cargo test --workspace --frozen -p presto-web i18n::tests::resolve_initial_locale_persisted_some_en_wins` exits **non-zero** (compile-fail). Separate commit.
  - **BlockedBy**: T008.

- [ ] T010 [US4] **[test-first RED]** Write failing `presto_web::i18n::tests::resolve_initial_locale_none_falls_to_os_de` in `src/src/i18n.rs`
  - **Files**: `src/src/i18n.rs` (test module)
  - **Test body** (per contracts/components.md §3 test 2, plan.md §V table row 5, SC-010 branch 2): call `resolve_initial_locale(None, &["de-DE".to_string()])` and assert the result is `presto_ipc::Locale::De`. `None` means no explicit locale chosen — OS detection runs; `de-DE`'s two-letter prefix `de` maps to `Locale::De`. Separate commit from T009.
  - **Done-signal**: exits **non-zero** (compile-fail). Separate commit.
  - **BlockedBy**: T009.

- [ ] T011 [US4] **[test-first RED]** Write failing `presto_web::i18n::tests::resolve_initial_locale_none_swiss_german_matches_de` in `src/src/i18n.rs`
  - **Files**: `src/src/i18n.rs` (test module)
  - **Test body** (per contracts/components.md §3 test 7, plan.md §V "Fix H"): call `resolve_initial_locale(None, &["de-CH".to_string()])` and assert the result is `presto_ipc::Locale::De`. Swiss German `de-CH` has two-letter prefix `de` → maps to `Locale::De`. Separate commit.
  - **Done-signal**: exits **non-zero** (compile-fail). Separate commit.
  - **BlockedBy**: T010.

- [ ] T012 [US4] **[test-first RED]** Write failing `presto_web::i18n::tests::resolve_initial_locale_none_unsupported_falls_back_to_en` in `src/src/i18n.rs`
  - **Files**: `src/src/i18n.rs` (test module)
  - **Test body** (per contracts/components.md §3 test 3, plan.md §V table row 6, SC-010 branch 3): call `resolve_initial_locale(None, &["fr-FR".to_string()])` and assert the result is `presto_ipc::Locale::En`. Unsupported OS locale → fallback to English. Separate commit.
  - **Done-signal**: exits **non-zero** (compile-fail). Separate commit.
  - **BlockedBy**: T011.

- [ ] T013 [US4] **[test-first RED]** Write failing `presto_web::i18n::tests::resolve_initial_locale_none_empty_os_falls_back_to_en` in `src/src/i18n.rs`
  - **Files**: `src/src/i18n.rs` (test module)
  - **Test body** (per contracts/components.md §3 test 4, plan.md §V table row 7, SC-010 branch 4, Spec FR-010): call `resolve_initial_locale(None, &[])` (empty OS language list — simulates `navigator.languages` unavailable or returning empty) and assert the result is `presto_ipc::Locale::En`. No panic, no error. Separate commit.
  - **Done-signal**: exits **non-zero** (compile-fail). Separate commit.
  - **BlockedBy**: T012.

- [ ] T014 [US4] **[test-first RED]** Write failing `presto_web::i18n::tests::resolve_initial_locale_none_first_match_wins` in `src/src/i18n.rs`
  - **Files**: `src/src/i18n.rs` (test module)
  - **Test body** (per contracts/components.md §3 test 5, plan.md §V table row 8, SC-010 branch 5): call `resolve_initial_locale(None, &["zh-CN".to_string(), "ja-JP".to_string(), "tr-TR".to_string()])` and assert the result is `presto_ipc::Locale::Tr`. First matching supported prefix wins — `zh-CN` and `ja-JP` are unsupported; `tr-TR`'s prefix `tr` matches `Locale::Tr`. Separate commit.
  - **Done-signal**: exits **non-zero** (compile-fail). Separate commit.
  - **BlockedBy**: T013.

### [US4] Implementation GREEN: `i18n.rs` module with resolver + library wiring

- [ ] T015 [US4] **[test-first GREEN]** Implement `src/src/i18n.rs` — `load_locales!()`, `resolve_initial_locale`, `match_two_letter_prefix`, `compute_initial_library_locale`, `From` impls; register `pub mod i18n` in `src/src/lib.rs`
  - **Files**: `src/src/i18n.rs` (implement in full), `src/src/lib.rs`
  - **Changes** (per plan.md §Phase 2, §Modules table for `src/src/i18n.rs`, data-model.md §4, contracts/components.md §3):
    1. At module top: `leptos_i18n::load_locales!();` (the proc-macro expansion generates the child `i18n` module with `Locale` enum + `I18nKeys` + `I18nContext`).
    2. Implement `pub fn resolve_initial_locale(persisted: Option<presto_ipc::Locale>, os_languages: &[String]) -> presto_ipc::Locale` as a pure function matching data-model.md §4 "Test boundary" pseudocode: (1) if `Some(locale)` return it verbatim; (2) iterate `os_languages`, call `match_two_letter_prefix`, return first match; (3) fall back to `presto_ipc::Locale::En`. No `web_sys` calls in this fn (FR-023 testability requirement).
    3. Implement `fn match_two_letter_prefix(lang: &str) -> Option<presto_ipc::Locale>` matching data-model.md §4: split on `['-', '_']`, take first, lowercase, match `"en"` → `En`, `"de"` → `De`, `"it"` → `It`, `"tr"` → `Tr`, else `None`.
    4. Implement `pub fn compute_initial_library_locale(settings: &presto_ipc::Settings) -> Option<i18n::Locale>` per contracts/components.md §3 "OS-detection helper" — returns `settings.appearance.locale.map(|l| l.into())`. `None` signals the library to run OS detection; `Some(_)` bypasses it.
    5. Implement `impl From<presto_ipc::Locale> for i18n::Locale` (total match; exhaustiveness fails compilation if either enum drifts) and its inverse `impl From<i18n::Locale> for presto_ipc::Locale`.
    6. Add `pub mod i18n;` to `src/src/lib.rs`.
    7. No `#[allow(...)]` unless the proc-macro output triggers a specific pedantic lint (document inline if added).
  - **Done-signal**: all seven resolver tests pass (`cargo test --workspace --frozen -p presto-web` all `i18n::tests::*` green). `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green. `grep 'pub mod i18n' src/src/lib.rs` returns a hit. `grep 'load_locales!' src/src/i18n.rs` returns a hit. `grep -r 'web_sys' src/src/engine/` returns same set as before (engine purity, SC-011). Committed separately from T008–T014 (GREEN commits follow all RED commits).
  - **BlockedBy**: T014, T007.

**Phase 2 exit**: All seven resolver tests pass (T008–T014 RED + T015 GREEN). `cargo clippy` + `cargo fmt --check` green. `src/src/i18n.rs` exists. Seven RED commits + one GREEN commit exist as eight separate git commits (NOT collapsed).

---

## Phase 3 — Boot-time wiring: `<I18nContextProvider>` + forwarding Effect

**Goal**: Mount the `I18nContextProvider` around the existing app tree in `src/src/app.rs`, passing `compute_initial_library_locale(&loaded)` as `initial_locale`. Add a small `Effect` that watches `settings.appearance.locale` and forwards changes to `i18n.set_locale(...)`. UI still renders in English — no component string-extraction yet.

**Exit**: `cargo build --workspace --frozen` exits zero. `trunk build --release` (from `src/`) exits zero. App boots with the i18n provider mounted; locale reads from settings.json at boot. No test-first (UI plumbing — e2e covers the boot-time path per plan.md §Phase 3).

- [ ] T016 [US1] Mount `<I18nContextProvider>` around app tree in `src/src/app.rs` and wire the locale-forwarding Effect
  - **Files**: `src/src/app.rs`
  - **Changes** (per plan.md §Phase 3 and §Modules table for `src/src/app.rs`, contracts/components.md §3 "OS-detection helper"):
    1. Import `use crate::i18n::{self, compute_initial_library_locale}` and the library's `<I18nContextProvider>` and `use_i18n` from the `i18n` module generated by `load_locales!()`.
    2. In the boot-time `spawn_local` block (near existing settings-load at ~line 200-215), after loading settings, compute `let initial_locale = compute_initial_library_locale(&loaded_settings);`.
    3. Wrap the existing app-tree view in `<I18nContextProvider initial_locale=initial_locale>`. This prop: `None` → library runs OS detection via `leptos-use::use_locales` (calls `navigator.languages` — the P3 path for fresh installs); `Some(locale)` → use verbatim, skip OS detection (the `Some(_)` path per FR-011 / Fix A).
    4. Add a small `Effect` that watches `settings.appearance.locale` (the IPC `RwSignal`) and on change calls `i18n.set_locale(i18n::Locale::from(new_locale))` so every `t!(...)` call site re-renders in the same Leptos reactive tick (Spec FR-012 / SC-007 mixed-locale-frame avoidance). The dropdown (Phase 4) writes ONE signal (the IPC settings signal); this Effect propagates to the library.
    5. The existing debounced settings-autosave Effect at ~line 215+ is untouched — it already picks up `settings.appearance.locale` changes and persists them (FR-016 / plan.md §Summary).
  - **Done-signal**: `cargo build --workspace --frozen` exits zero. `trunk build --release` (from `src/`) exits zero. `grep 'I18nContextProvider' src/src/app.rs` returns a hit. `grep 'compute_initial_library_locale' src/src/app.rs` returns a hit. The existing app behaviour is unchanged (UI still renders in English — no component has been rewritten yet). `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green. `bash scripts/check-engine-purity.sh` exits 0 (zero new `web_sys` references under `src/src/engine/`).
  - **BlockedBy**: T015.

**Phase 3 exit**: `cargo build --workspace --frozen` and `trunk build --release` both exit zero. `cargo clippy` green. Engine purity gate green. Boot path wired; user cannot yet pick a locale from the UI.

---

## Phase 4 — Settings UI: locale-switcher dropdown

**Goal**: Add the `#locale-selector` dropdown to `src/src/components/settings/general.rs` above the timer-durations section. Add the e2e flow in `tests/e2e/settings-general.spec.js`. Visual baseline is NOT regenerated yet — that's Phase 6.

**Exit**: `<select id="locale-selector">` renders above `#focus-duration` in Settings → General. The `on:change` handler writes `settings.appearance.locale`. e2e flow passes. No test-first (UI plumbing per plan.md §Phase 4).

- [ ] T017 [US1] Add `<select id="locale-selector">` Language control row to `src/src/components/settings/general.rs` above the timer-durations section
  - **Files**: `src/src/components/settings/general.rs`
  - **Changes** (per plan.md §Phase 4 and §Modules table for `settings/general.rs`, contracts/components.md §3 "Locale-switcher API", Spec FR-015, FR-016):
    1. Import `use crate::i18n::*;` and `use_i18n` at the top of the file.
    2. Inside the component, retrieve `let i18n = use_i18n();`.
    3. Add a new control row above the existing `<h3>"Timer Durations"</h3>` block (i.e., above the `#focus-duration` field as the first control in the tab per FR-015). The row structure: a `<label>` element whose text is `{t!(i18n, settings.general.language_label)}` (the surrounding "Language" label IS localised — FR-015 / Spec Story 1 AC 4), followed by `<select id="locale-selector">` with four `<option>` entries in this fixed order: `<option value="en">"English"</option>`, `<option value="de">"Deutsch"</option>`, `<option value="it">"Italiano"</option>`, `<option value="tr">"Türkçe"</option>`. The option labels are native self-names hard-coded as Rust string literals — they are NEVER re-translated when the active locale changes (FR-015 / Spec Story 1 AC 4 / plan.md §Modules "BEST-GUESS PM DECISION").
    4. The `prop:value` binding reflects the active locale: `move || match settings.appearance.locale.get() { Some(Locale::De) => "de", Some(Locale::It) => "it", Some(Locale::Tr) => "tr", _ => "en" }`.
    5. The `on:change` handler: parse `event_target_value(&ev)` → match to `presto_ipc::Locale` variant → write `settings.appearance.locale.set(Some(new_locale))`. No bespoke toast for the language change (FR-016 — the existing debounced autosave Effect at `src/src/app.rs:215+` persists the change).
    6. No existing selector (`#focus-duration`, `#break-duration`, `#long-break-duration`, `#total-sessions`, `#max-session-time`, `#sessions-per-long-break`) is renamed or removed (FR-017).
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. `cargo build --workspace --frozen` compiles. `grep 'locale-selector' src/src/components/settings/general.rs` returns a hit. `grep '#focus-duration' src/src/components/settings/general.rs` — the existing selector is still present (FR-017 compliance). In browser / `trunk serve`, Settings → General shows the Language row above the timer-durations section.
  - **BlockedBy**: T016.

- [ ] T018 [US1] Extend `tests/e2e/settings-general.spec.js` with a locale-switcher e2e flow
  - **Files**: `tests/e2e/settings-general.spec.js`
  - **Test flow** (per plan.md §Modules table for `settings-general.spec.js`, quickstart.md §New e2e flow, Spec FR-017, FR-022):
    1. Navigate to Settings → General.
    2. Assert `#locale-selector` is visible above `#focus-duration` (selector ordering per FR-015).
    3. Assert `#locale-selector` current value reflects the active locale (default: `"en"`).
    4. Pick `Deutsch` (`value="de"`) from `#locale-selector`.
    5. Assert the surrounding label element changes to `Sprache` (the German translation of "Language" — SC-007 mixed-locale assertion).
    6. Navigate to a non-Settings view (e.g. click a nav item) and navigate back to Settings → another tab; assert a known label in that tab renders in German (e.g. the Notifications tab name or a setting label — verifies locale signal is persistent across navigation).
    7. Return to Settings → General, pick `English` (`value="en"`), assert the surrounding label reverts to `Language`.
    8. Close and reopen Settings → General; assert `#locale-selector` still shows `"en"` (persistence check via the existing autosave Effect).
    - Do NOT rename or remove any existing selector (`#focus-duration`, `#break-duration`, `#long-break-duration`, `#total-sessions`, `#max-session-time`, `#sessions-per-long-break`).
  - **Done-signal**: `cd tests/e2e && npx playwright test settings-general.spec.js --reporter=line` exits 0 (all tests including the new locale-switcher flow). `grep 'locale-selector' tests/e2e/settings-general.spec.js` returns ≥2 hits. No existing test in that file fails. The Tauri mock at `tests/e2e/fixtures/tauriMock.js` is NOT modified (no new Tauri commands — FR-018 / SC-012).
  - **BlockedBy**: T017.

**Phase 4 exit**: `cargo clippy` + `cargo fmt --check` green. `#locale-selector` renders in the DOM above `#focus-duration`. e2e flow passes. Existing selectors intact. No visual baseline regenerated yet.

---

## Phase 5 — Bulk string extraction

**Goal**: Extract all in-scope English strings (FR-013 surface) into `src/locales/en.json`, replace each with a `t!(i18n, namespace.key)` typed-key macro call site, and populate `de.json` / `it.json` / `tr.json` with hand-curated translations. Run the pluralization audit (plan.md §Complexity Tracking Fix G) before starting. High-volume work — broken into sub-tasks by view area; each sub-task is 30–120 minutes.

**Exit**: Every hard-coded English string in the FR-013 scope is moved into the catalogue. `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` exits zero (no `MissingKey` / `SurplusKey` warnings). `trunk build --release` exits zero. e2e smoke confirms locale switching flips non-Settings strings.

### Pre-extraction audit

- [ ] T019 Run pluralization audit per plan.md Fix G before any extraction commit; document findings in a comment in `src/src/i18n.rs`
  - **Files**: `src/src/i18n.rs` (add a doc comment with audit findings)
  - **Procedure** (per plan.md §Complexity Tracking Fix G): run the two grep commands below to surface count-bearing format strings. Inspect each hit: if any string in FR-013 scope is count-sensitive ("1 session" / "N sessions"), enable the `plurals` cargo feature in `src/Cargo.toml` and add ICU plural-rule aware keys. If zero plural-sensitive strings exist (all count-sensitive strings already use non-plural-sensitive phrasing per Spec Edge Cases bullet 5), document the audit result and leave `plurals` OFF. The `icu_compiled_data` feature stays on regardless (already in the dep declaration for future follow-up).

    After completing the audit, add the following structured `PLURALIZATION_AUDIT` doc-comment block to `src/src/i18n.rs` (at module top, before `load_locales!()`):

    ```rust
    // PLURALIZATION_AUDIT (Phase 3, gate T019):
    //
    // Audited strings (grep results below) for count-bearing phrasing.
    // - format!(...) count-bearing patterns: <n found, listed here>
    // - In-scope strings with plural sensitivity: <list>
    // - Decision: <enable `plurals` feature flag | omit, no plural-sensitive strings>
    //
    // Grep commands run:
    //   grep -rE 'format!.*"\{\}.*(session|minute|task|tag)"' src/src/
    //   grep -rE '"\d+ (sessions?|minutes?|tasks?|tags?)"' src/src/
    //
    // Verification: this comment block exists and is signed-off in the
    // Phase 5 commit.
    ```

    Fill in the actual grep results. If `plurals` is enabled, `src/Cargo.toml` is updated and the `Cargo.lock` regenerated.
  - **Done-signal**: `grep -c "PLURALIZATION_AUDIT" src/src/i18n.rs` returns ≥ 1. The comment block contains the actual grep output and the "Decision" line is filled in. If `plurals` is enabled, `src/Cargo.toml` is updated and `Cargo.lock` regenerated.
  - **BlockedBy**: T018.

### Timer screen extraction

- [ ] T020 [P] [US1] Extract timer-screen strings into `src/locales/*.json` and rewrite `src/src/components/timer/mod.rs` to use `t!(i18n, ...)` typed-key call sites
  - **Files**: `src/locales/en.json`, `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json`, `src/src/components/timer/mod.rs`, `src/src/components/timer/messages.rs` (if user-visible strings exist there)
  - **Strings to extract** (per Spec FR-013 "Timer screen"): mode badges (`Focus` → `timer.mode_focus`, `Break` → `timer.mode_break`, `Long Break` → `timer.mode_long_break`); state suffixes (`Paused` → `timer.state_paused`, `Auto-paused` → `timer.state_auto_paused`, `Overtime` → `timer.state_overtime`); control button visible labels (`Reset` → `timer.ctrl_reset`, `Undo` → `timer.ctrl_undo`, `Start` → `timer.ctrl_start`, `Pause` → `timer.ctrl_pause`, `Resume` → `timer.ctrl_resume`, `Skip session` → `timer.ctrl_skip`); the corresponding verbose `aria-label` strings on each control button (each `aria-label` is its own catalogue key — contracts/components.md §3, Spec FR-013 / A11, e.g. `timer.ctrl_reset_aria`). Attribute-value sites (`aria-label=`) MUST use `t_string!` (or `td_string!`) NOT `t!` per contracts/components.md §3. Verify exact macro name against `leptos_i18n` v0.5.11 docs at task time.
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` exits zero (no `MissingKey` warnings for these keys). `grep -rn '"Focus"\|"Break"\|"Long Break"\|"Paused"\|"Overtime"\|"Reset"\|"Start"\|"Pause"\|"Resume"\|"Skip session"' src/src/components/timer/` returns zero hits (string literals extracted). `grep 't!(i18n' src/src/components/timer/mod.rs` returns ≥10 hits. SC-004 / SC-005 satisfied for this file.
  - **BlockedBy**: T019.

- [ ] T021 [P] [US1] Extract tag-picker strings into `src/locales/*.json` and rewrite `src/src/components/timer/tag_tracking.rs`
  - **Files**: `src/locales/en.json`, `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json`, `src/src/components/timer/tag_tracking.rs`
  - **Strings to extract** (per Spec FR-013 "Tag picker / manager"): `New tag…` placeholder → `tag.new_tag_placeholder`; `Choose tag` header → `tag.choose_tag_header`. Placeholder attribute uses `t_string!` (or `td_string!`) per contracts/components.md §3.
  - **Done-signal**: `cargo clippy` green (no `MissingKey`). `grep '"New tag\|"Choose tag"' src/src/components/timer/tag_tracking.rs` returns zero hits. SC-004 / SC-005 for this file.
  - **BlockedBy**: T019.

### Sidebar nav tooltips

- [ ] T022 [US1] Locate sidebar nav tooltip strings (Timer / Statistics / Daily / Settings), extract into `src/locales/*.json`, and rewrite the owning file (`src/src/app.rs` or a sidebar component)
  - **Files**: `src/locales/en.json`, `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json`, `src/src/app.rs` (or whichever module owns the sidebar tooltip strings — confirm at task time per plan.md §Modules "BEST-GUESS PM DECISION")
  - **Strings to extract** (per Spec FR-013 "Sidebar nav tooltips"): `Timer` → `sidebar.timer`; `Statistics` → `sidebar.statistics`; `Daily` → `sidebar.daily`; `Settings` → `sidebar.settings`. Tooltip strings that appear as HTML attributes use `t_string!` / `td_string!`.
  - **Done-signal**: `cargo clippy` green. `grep '"Timer"\|"Statistics"\|"Daily"\|"Settings"' src/src/app.rs` returns zero hits for any remaining hard-coded tooltip literals. SC-004 / SC-005 for sidebar strings.
  - **BlockedBy**: T019.

### Settings tabs and general-tab strings

- [ ] T023 [P] [US1] Extract Settings tab names and Settings → General tab strings into `src/locales/*.json` and rewrite `src/src/components/settings/mod.rs` and `src/src/components/settings/general.rs`
  - **Files**: `src/locales/en.json`, `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json`, `src/src/components/settings/mod.rs`, `src/src/components/settings/general.rs`
  - **Strings to extract** (per Spec FR-013 "Settings"): the eight tab names (`General` → `settings.tab_general`, `Shortcuts` → `settings.tab_shortcuts`, `Notifications` → `settings.tab_notifications`, `Theme` → `settings.tab_theme`, `Automation` → `settings.tab_automation`, `Goals` → `settings.tab_goals`, `Advanced` → `settings.tab_advanced`, `Updates` → `settings.tab_updates`); the auto-save toast strings (`Settings saved` → `settings.auto_save_ok`, `Failed to save settings` → `settings.auto_save_err`); all Settings → General visible labels, helper text, unit suffixes; the surrounding `settings.general.language_label` key added in T017 must appear in all four catalogues here if not already present.
  - **Done-signal**: `cargo clippy` green. `grep '"General"\|"Shortcuts"\|"Notifications"\|"Theme"\|"Automation"\|"Goals"\|"Advanced"\|"Updates"\|"Settings saved"\|"Failed to save settings"' src/src/components/settings/mod.rs src/src/components/settings/general.rs` returns zero hits for remaining hard-coded literals in these files. SC-004 / SC-005 for these files.
  - **BlockedBy**: T019.

- [ ] T024 [P] [US1] Extract Settings → Notifications, Shortcuts, Theme, Automation, Goals, Advanced, and Updates tab strings into `src/locales/*.json` and rewrite each component file
  - **Files**: `src/locales/en.json`, `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json`, `src/src/components/settings/notifications.rs`, `src/src/components/settings/shortcuts.rs`, `src/src/components/settings/theme.rs`, `src/src/components/settings/automation.rs`, `src/src/components/settings/goals.rs`, `src/src/components/settings/advanced.rs`, `src/src/components/settings/updates.rs`
  - **Strings to extract** (per Spec FR-013 "Settings" — all seven tabs' visible labels, helper / hint text, unit suffixes for every setting on each tab). Per-file scope anchor:

    | File | Estimated keys | Namespace |
    |---|---|---|
    | `settings/automation.rs` | ~6 | `settings.automation.*` |
    | `settings/notifications.rs` | ~15 (4 toggles + ambient sub-card) | `settings.notifications.*` |
    | `settings/theme.rs` | ~8 (light/dark/auto labels, theme tile names) | `settings.theme.*` |
    | `settings/goals.rs` | ~5 | `settings.goals.*` |
    | `settings/advanced.rs` | ~4 | `settings.advanced.*` |
    | `settings/shortcuts.rs` | ~6 (shortcut labels) | `settings.shortcuts.*` |
    | `settings/updates.rs` | ~8 (status strings, "Check for updates", etc.) | `settings.updates.*` |

    Counts are estimates; refine per actual `grep -E 'view! \{ "[A-Z]'` output on each file. The auto-save toast keys (`settings.auto_save_ok`, `settings.auto_save_err`) were extracted in T023 — import them here, do not re-extract. Existing selectors (`#focus-duration`, `#break-duration`, etc.) are preserved — only the visible text strings change.
  - **Done-signal**: `cargo clippy` green (no `MissingKey`). After extraction, each file's `grep -E 'view! \{ "[A-Z]' src/src/components/settings/<file>.rs` returns zero hits (no remaining English string literals in `view!` blocks). SC-004 / SC-005 for these files.
  - **BlockedBy**: T023.

### Statistics view strings

- [ ] T025 [P] [US1] Extract Statistics view strings into `src/locales/*.json` and rewrite `src/src/components/stats/` files
  - **Files**: `src/locales/en.json`, `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json`, `src/src/components/stats/period_selector.rs`, `src/src/components/stats/period_nav.rs`, `src/src/components/stats/focus_trend.rs`, `src/src/components/stats/peak_focus_time.rs`, `src/src/components/stats/monthly_peak_day.rs`, `src/src/components/stats/tag_usage_pie.rs`, `src/src/components/stats/bar_chart.rs`, `src/src/components/stats/line_chart.rs`, `src/src/components/stats/mod.rs`
  - **Strings to extract** (per Spec FR-013 "Statistics view"): the four period tab labels (`Daily` → `stats.period_daily`, `Weekly` → `stats.period_weekly`, `Monthly` → `stats.period_monthly`, `Yearly` → `stats.period_yearly`); tile labels (e.g. `Total sessions`, `Total focus time`, `Best day` — every visible tile heading → `stats.tile_*`); string-based axis labels (NOT chrono-formatted date strings which stay English per FR-014 / A8); "No data" empty-state strings (`stats.no_data`). `chrono`-rendered timestamps stay English — do NOT extract numeric date/time fragments.
  - **Done-signal**: `cargo clippy` green (no `MissingKey`). `grep -rn '"Daily"\|"Weekly"\|"Monthly"\|"Yearly"\|"No data"\|"Total sessions"\|"Total focus time"' src/src/components/stats/` returns zero hits for remaining hard-coded English literals in these files. SC-004 / SC-005 for stats files.
  - **BlockedBy**: T019.

### Daily view and calendar strings

- [ ] T026 [P] [US1] Extract Daily view and calendar strings into `src/locales/*.json` and rewrite `src/src/components/daily/` files
  - **Files**: `src/locales/en.json`, `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json`, `src/src/components/daily/mod.rs`, `src/src/components/daily/sessions_history_table.rs`, `src/src/components/daily/sessions_timeline.rs`, `src/src/components/daily/month_grid.rs`
  - **Strings to extract** (per Spec FR-013 "Daily view" and "Calendar"): `Daily Overview` → `daily.header`; `Today's Sessions` → `daily.sessions_header`; `No sessions completed` → `daily.empty_state`; table column headers; timeline labels / empty-state strings; twelve month names (`January` through `December` → `calendar.month_jan` … `calendar.month_dec`); seven day-of-week headers (`Sun`, `Mon`, `Tue`, `Wed`, `Thu`, `Fri`, `Sat` → `calendar.dow_sun` … `calendar.dow_sat`). Per FR-013 / FR-025 / A8: month name strings ARE localised; `chrono`-formatted numeric date parts (the day number digit, e.g. `"13"`) stay English and are NOT extracted.
  - **Done-signal**: `cargo clippy` green (no `MissingKey`). `grep -rn '"Daily Overview"\|"Today'\''s Sessions"\|"No sessions completed"\|"January"\|"February"\|"Sunday"\|"Monday"\|"Sun"\|"Mon"' src/src/components/daily/` returns zero hits for remaining hard-coded English literals. SC-004 / SC-005 for daily files.
  - **BlockedBy**: T019.

### Update notification strings

- [ ] T027 [P] [US1] Extract update notification strings into `src/locales/*.json` and rewrite `src/src/components/update_notification.rs`
  - **Files**: `src/locales/en.json`, `src/locales/de.json`, `src/locales/it.json`, `src/locales/tr.json`, `src/src/components/update_notification.rs`
  - **Strings to extract** (per Spec FR-013 "Update notification"): `Update available` → `update.title`; surrounding version-display label (the version number literal stays as-is per FR-014); `Update via Homebrew` (or the platform-specific equivalent) → `update.install_action`; `Skip release` → `update.skip`. The version number literal is NOT extracted.
  - **Done-signal**: `cargo clippy` green (no `MissingKey`). `grep '"Update available"\|"Skip release"\|"Update via Homebrew"' src/src/components/update_notification.rs` returns zero hits. SC-004 / SC-005 for this file.
  - **BlockedBy**: T019.

### Translation-completeness sweep

- [ ] T028 [US1] Verify translation completeness: run full clippy gate and confirm zero `MissingKey` / `SurplusKey` warnings; extend e2e smoke to assert non-Settings strings flip on locale switch
  - **Files**: `tests/e2e/settings-general.spec.js` (extend e2e flow to assert a known non-Settings string flips on locale switch)
  - **Procedure**:
    1. Run `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic`. Expected: zero exit (no `MissingKey`, no `SurplusKey`). If warnings appear, fix the missing or surplus keys before closing this task.
    2. Run `trunk build --release` (from `src/`). Expected: zero exit.
    3. Run `grep -rn '"Focus"\|"Break"\|"Long Break"\|"Daily Overview"\|"Settings saved"' src/src/components/` to confirm no hard-coded English literals remain in FR-013 scope.
    4. Extend the existing e2e locale-switcher flow in `tests/e2e/settings-general.spec.js` to assert that after picking `Deutsch`, a known non-Settings string (e.g. a timer mode badge or daily-view header) renders in German — confirming the locale signal propagates beyond the Settings tab.
    5. Run `cd tests/e2e && npx playwright test settings-general.spec.js --reporter=line` to confirm the extended flow passes.
    6. Verify SC-008: `grep -rn 'fetch(\|XMLHttpRequest\|reqwest::Client' src/ tests/` returns zero new hits vs pre-feature baseline.
    7. Verify SC-011: `grep -r 'Locale\|locale\|t!(' src/src/engine/` returns zero hits.
    8. Verify SC-012: `grep -c '#\[tauri::command\]' src-tauri/src/lib.rs` is unchanged from pre-feature baseline.
  - **Done-signal**: All verification steps exit zero / return expected values. `cargo clippy` gate green with zero translation warnings. e2e flow extended and passing. SC-004, SC-005, SC-006, SC-008, SC-011, SC-012 satisfied.
  - **BlockedBy**: T020, T021, T022, T023, T024, T025, T026, T027.

**Phase 5 exit**: `cargo clippy` and `trunk build --release` both exit zero. All FR-013-scope string literals extracted. `de.json` / `it.json` / `tr.json` carry complete key sets. e2e smoke passes with locale-switch assertion on non-Settings strings.

---

## Phase 6 — Visual regression baseline regen + final gate sweep

**Goal**: Regenerate exactly one baseline (`settings-general-chromium-linux.png`) with the one-line PR note. Confirm no other baselines flag a diff. Run the full gate sweep.

**Exit**: CI visual-regression run sees only one failing baseline diff (`settings-general-chromium-linux.png`). After regen, all baselines pass. Full gate sweep exits 0. US3 (SC-009) satisfied.

### [US3] Visual regression baseline

- [ ] T029 [US3] Confirm only `settings-general-chromium-linux.png` diffs, then regenerate it
  - **Files**: `tests/e2e/__screenshots__/visual-regression/settings-general-chromium-linux.png`
  - **Procedure** (per plan.md §Phase 6, quickstart.md §Regenerate affected baseline, Spec FR-021, SC-009):
    1. Run `cd tests/e2e && npx playwright test visual-regression.spec.js --reporter=line`. Confirm the ONLY failing baseline is `settings-general-chromium-linux.png`. Any diff on timer, statistics-*, daily, tag-manager, update-notification, settings-notifications, settings-shortcuts, settings-theme, settings-automation, settings-goals, settings-advanced, settings-updates is a **regression to fix in code** — do NOT absorb by re-baselining (FR-021 / Story 3 AC 3).
    2. Regenerate: `npx playwright test visual-regression.spec.js --update-snapshots --grep "settings-general"`.
    3. Review the regenerated PNG visually: the Language dropdown row should appear above the timer-durations section. No other layout change.
    4. Stage and commit the single PNG. The PR description MUST include verbatim: `settings-general-chromium-linux.png: Language dropdown row added above the timer-durations section, four native-self-name options (English / Deutsch / Italiano / Türkçe). No other layout change.` (per plan.md §IV and quickstart.md §Per-baseline justification).
  - **Done-signal**: `git status tests/e2e/__screenshots__/visual-regression/ | grep -v '^?' | wc -l` returns 1 (exactly one PNG modified). `npx playwright test visual-regression.spec.js --reporter=line` exits 0 (all baselines pass after regen). The sidebar-mask posture from feature 003 remains in effect. SC-009 satisfied.
  - **BlockedBy**: T028.

### Final gate sweep

- [ ] T030 Full final gate sweep before opening the PR
  - **Files**: (read-only verification; no source edits)
  - **Done-signal** (ALL must exit 0 or return expected values):
    - `cargo fmt --check`
    - `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` (translation-completeness check runs here — SC-006)
    - `cargo test --workspace --frozen` (includes 3 IPC round-trip tests + 7 resolver tests [T008–T014 RED + T015 GREEN = 8 Phase 2 commits] + all pre-existing)
    - `cd src && trunk build --release` (cold; verifies `load_locales!()` reads all four catalogue files — SC-008)
    - `cd tests/e2e && npx playwright test --reporter=line` (full e2e including new locale-switcher flow + visual regression)
    - `bash scripts/check-engine-purity.sh` (SC-011: zero new `web_sys` under `src/src/engine/`)
    - `bash scripts/check-mock-drift.sh` (SC-012: 0 new Tauri commands; no new mock entries)
    - `grep -rn 'fetch(\|XMLHttpRequest\|reqwest::Client' src/ tests/` — returns 0 new hits (SC-008: no new network egress)
    - `grep -r 'Locale\|locale\|t!(' src/src/engine/` — returns 0 hits (SC-011)
    - `grep -c '#\[tauri::command\]' src-tauri/src/lib.rs` — unchanged from pre-feature count (SC-012)
    - `grep -i 'ramazan\|murdercode' specs/005-i18n/tasks.md` — returns 0 hits
  - **BlockedBy**: T029.

**Phase 6 exit**: All gates exit 0. PR ready to open with the per-baseline note in the description.

---

## Dependencies (compact)

- **Phase 0** (T001–T003): T001 → T002 → T003. Sequential (T002 needs the `Cargo.toml` change; T003 needs the catalogue files).
- **Phase 1** (T004–T007): T001 is a soft prerequisite (IPC crate doesn't need `leptos_i18n`, but the branch should be consistent). T004 (RED) → T005 (RED) → T006 (RED) → T007 (GREEN). Sequential. **Four separate commits — NOT collapsed.**
- **Phase 2** (T008–T015): T007 must be complete (IPC `Locale` enum exists). T008 (RED) → T009 (RED) → T010 (RED) → T011 (RED) → T012 (RED) → T013 (RED) → T014 (RED) → T015 (GREEN). Sequential. **Eight separate commits (7 RED + 1 GREEN) — NOT collapsed.**
- **Phase 3** (T016): Blocked by T015 (i18n module exists). Sequential.
- **Phase 4** (T017–T018): T016 → T017 → T018. Sequential.
- **Phase 5** (T019–T028): T018 (or T016 for the extraction tasks themselves) → T019 → T020–T027 (parallelisable) → T028.
- **Phase 6** (T029–T030): T028 → T029 → T030. Sequential.

## Parallel opportunities

- **Phase 0**: T001, T002, T003 are sequential by dependency but each touches different files — no cross-task contention.
- **Phase 1**: T004–T007 are sequential by test-first discipline (each RED must land before the next RED).
- **Phase 2**: T008–T015 are sequential by test-first discipline.
- **Phase 5**: T020, T021, T022, T023, T025, T026, T027 are all parallelisable after T019 — they touch different component files. T024 is blocked by T023 (needs the `auto_save_*` keys extracted first). T028 is blocked by all of T020–T027.
- **Phase 3 and Phase 5 prep**: once T016 exists, T019 (pluralization audit) can start immediately in parallel with T017 (settings UI) — they touch different files.

---

## Notes

- **RED/GREEN commits are NOT collapsed** for T004–T006 (each RED lands separately), then T007 (GREEN); likewise T008–T014 (each RED lands separately), then T015 (GREEN). Per AGENTS.md §Test-first commit ordering and plan.md §Phase 1 / §Phase 2.
- **Fix A invariant** (plan.md Fix A, data-model.md Fix A): the `locale` field type is `Option<Locale>`, NOT `Locale`. `Some(Locale::En)` (explicit English) and `None` (no choice yet) are NOT equivalent. The resolver's authoritative "explicit vs. default" signal is the `Option<Locale>` discriminant — value-equality against `Locale::En` MUST NOT be used as a proxy anywhere in the codebase (FR-009 / FR-011).
- **No new Tauri commands** — `bash scripts/check-mock-drift.sh` stays green throughout. `tests/e2e/fixtures/tauriMock.js` is untouched. The new `AppearanceSettings.locale` field flows transparently through the existing `save_settings` / `load_settings` round-trip (FR-018 / SC-012 / plan.md §VI).
- **One new runtime dep** — `leptos_i18n = "=0.5.11"` exact-pinned. `src/Cargo.lock` updated in lockstep with `src/Cargo.toml` (Principle IX / Spec FR-020). No new npm dep; `tests/e2e/package-lock.json` stays byte-stable.
- **Locale option labels are never localised** — the four `<option>` values (`English`, `Deutsch`, `Italiano`, `Türkçe`) are hard-coded Rust string literals, not `t!(...)` call sites, per Spec FR-015 / Story 1 AC 4 / plan.md "BEST-GUESS PM DECISION". Only the surrounding `Language` label IS localised.
- **`aria-label` strings are distinct catalogue entries** — each verbose `aria-label` (e.g. `"Reset the timer to the start of the focus session"`) is its own typed key, separate from the button's visible text (`"Reset"`). Both use `t_string!` / `td_string!` (attribute-value macro form) per contracts/components.md §3. Both are compile-time-checked (Spec FR-013 / A11 / SC-005).
- **`chrono`-rendered timestamps stay English** — numeric date/time fragments are NOT extracted per FR-014 / FR-025 / A8. Only surrounding label strings are in scope.
- **Engine purity gate stays green by construction** — all new code lives under `src/src/components/`, `src/src/app.rs`, `src/src/i18n.rs`, and `crates/presto-ipc/`. Nothing touches `src/src/engine/`. Verified by `bash scripts/check-engine-purity.sh` in T030 (SC-011 / Principle I).
- **Only one visual regression baseline regenerates** — `settings-general-chromium-linux.png`. Any diff on untouched screens (timer, statistics, daily, tag-manager, other settings tabs, update-notification) is a regression to fix in code, not absorbed by re-baselining (FR-021 / SC-009 / Principle IV / Story 3 AC 3).
- **User Story 5 (beta badge)** is conditional per Spec A9: if all four locales reach 100% coverage at ship time (enforced by the clippy `MissingKey` gate), the `(beta)` badge feature is omitted entirely — no code, no UI. If any locale ships with gaps, the badge becomes a follow-up issue filed at end-gate per Spec Edge Cases "Beta-coverage indicator".
- **Missing-key gate** (FR-006 / research.md Decision 3): enforced by the existing `cargo clippy -- -D warnings` CI gate promoting `leptos_i18n`'s `MissingKey` `#[deprecated]` annotations to hard failures. No separate CI script needed (verified in T003). If T003 finds the proc-macro path leaky, a backup `scripts/check-translation-completeness.sh` is registered in `.agentex.yml`'s `lint:` block.
- **No fork attribution** — `grep -i 'ramazan\|murdercode' specs/005-i18n/tasks.md` returns 0 hits (verified in T030 final gate sweep).
