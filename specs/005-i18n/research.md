# Research Decisions — Feature 005 (Multi-Locale UI With In-App Language Switcher)

Three external decisions are pinned here because they are irreversible-ish (library
pick would bind the catalogue file format, the macro call-site shape, and the
proc-macro's compile-time-checking semantics byte-stable across every component
file that consumes a key; catalogue file format would bind the on-disk layout of
four locale files; missing-translation enforcement path would bind the CI gate
choice). The fourth concern — the exact English copy that becomes the
source-of-truth catalogue — is a tasks-phase concern, not a plan-phase concern,
but the typed-key contract is asserted here so the tasks-phase work doesn't
accidentally reach for a stringly-typed lookup.

## Decision 1 — Library pick: `leptos_i18n` at `v0.5.11`

**Chosen**: `leptos_i18n` at `v0.5.11` (latest release in the `0.5.x` series, the
only series compatible with `leptos = "0.7"` which is the current project
dependency at `src/Cargo.toml:20`). Added to `src/Cargo.toml` `[dependencies]`
as `leptos_i18n = { version = "=0.5.11", default-features = false, features =
["csr", "json_files", "icu_compiled_data"] }`. Version is **exact-pinned**
(`=0.5.11`, not `"0.5"`) because only `0.5.11` is verified against
`leptos = "0.7"`; upgrade only when a new `0.5.x` release is explicitly
retested (Fix C). Lockstep `Cargo.lock` update per Principle IX.

**Wire-shape note (Fix A)**: The `AppearanceSettings.locale` field is
`Option<Locale>`, NOT `Locale`. `None` = "no explicit locale chosen" (legacy
records or fresh install); `Some(Locale)` = "user explicitly saved this locale
(including English)". The `Option` discriminant is the resolver's authoritative
"explicit vs. default" signal. Value-equality against `Locale::En` MUST NOT be
used as a proxy — a German-OS user who explicitly picks English would be reverted
to German on the next cold start if the resolver used `persisted == Locale::En`
as the "run OS detection" trigger. See FR-009 / FR-011 / data-model.md Fix A.

**Rejected alternatives**:

- **`leptos-fluent` at `v0.3.1`**: stringly-typed `tr!("key-name")` lookup
  surface; missing-key checking is a debug-mode-only opt-in gated on the
  `check_translations: true` `leptos_fluent! {}` config field — not a compile-
  time error from the proc-macro itself. Documented below.
- **`leptos_i18n_macro`**: NOT a separate library — it is the proc-macro
  subcrate of `leptos_i18n` and is pulled in transitively by the parent crate's
  re-exports. Not an evaluation candidate per the PM brief.
- **`leptos-router-i18n`**: a URL-routing extension for locale-prefixed paths
  (e.g. `/en/timer`, `/de/timer`). Desktop app has no URL routing — single
  `index.html` mounted by Tauri's WebView at `tauri://localhost/`. Out of
  scope.

### Candidate 1: `leptos_i18n` (Baptistemontan)

| Aspect | Finding |
|---|---|
| **Latest stable version** | `0.6.2` (released 2026-04-14) for `leptos = "0.8"`; **`0.5.11`** (released 2025-03-22) for `leptos = "0.7"` per the README's compatibility table at `https://github.com/Baptistemontan/leptos_i18n#version-compatibility-with-leptos`. Presto runs `leptos = "0.7"` at `src/Cargo.toml:20`, so the version pin is `0.5.11`. |
| **Author / maintenance** | Baptiste de Montangon (`@Baptistemontan`). 158 stars; 8 open issues; default branch last pushed 2026-05-11; 49 releases. Active. |
| **Catalogue format** | **JSON by default** (`default = ["cookie", "json_files", "icu_compiled_data"]` at `leptos_i18n/Cargo.toml:53`). YAML and JSON5 available via `yaml` / `json5` cargo features. One file per locale at `<crate-root>/locales/<locale>.<ext>`, declared via `[package.metadata.leptos-i18n] default = "en" locales = ["en", "de", "it", "tr"]` in `Cargo.toml`. |
| **Compile-time key checking** | **YES — typed Rust identifiers.** The proc-macro `leptos_i18n::load_locales!()` (invoked once at crate root) generates an `i18n` module containing a `Locale` enum (variants are the locale codes — `Locale::en`, `Locale::de`, `Locale::it`, `Locale::tr`) and an `I18nKeys` struct whose typed identifiers correspond 1:1 to the JSON keys. The `t!(i18n, key_name)` macro emits a typed method call on `I18nKeys` — a key typo fails `cargo build` with `error[E0599]: no method named …`. Interpolation parameter names are also typed (FR-005 + Spec A11). |
| **Missing-translation behaviour** | **Compile-time warning that promotes to error under `-D warnings`.** When a key exists in the default (`en`) locale but is missing from `de` / `it` / `tr`, the proc-macro emits a `#[deprecated(note = "Missing key \"<path>\" in locale <locale>")]` function annotation that fires during the generated `warnings()` fn call. Per `leptos_i18n_macro/src/load_locales/warning.rs` (the `warning_fn` / `generate_warnings_inner` pair), the warning becomes a `deprecated` lint diagnostic. The presto workspace runs `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` (per `.agentex.yml` `lint:` block) — `-D warnings` covers `deprecated`, so a missing key fails CI. **No extra translation-completeness script is needed** — the existing clippy gate enforces FR-006 by construction. |
| **Locale-switching API** | **Signal-based.** `let i18n = use_i18n();` retrieves the `I18nContext` from a Leptos context. `i18n.get_locale() -> Locale` and `i18n.set_locale(Locale::de)` are reactive reads / writes; views consuming `t!(i18n, ...)` re-render automatically in the same Leptos reactive batch as the locale set call (Spec edge case "mixed-locale frame avoidance" honoured by Leptos signal batching). |
| **OS locale detection** | **Built-in via `leptos-use::use_locales`.** `leptos_i18n` depends on `leptos-use` with the `use_locales` feature enabled (`leptos_i18n/Cargo.toml:14-17`). At app boot, the `<I18nContextProvider>` wrapper component reads `leptos-use::use_locales_with_options` which returns a `Signal<Vec<String>>` populated from `navigator.languages` (CSR) or the `Accept-Language` header (SSR; N/A for presto). The `fetch_locale_csr` path in `leptos_i18n/src/fetch_locale.rs` resolves the initial locale via `current_cookie.or_else(|| L::find_locale(accepted))`; `L::find_locale` matches by BCP-47 two-letter prefix against the declared locale list. **No bespoke `navigator.language` parser needed** — the library provides the entire resolution chain (cookie → OS → default). For presto, the equivalent is "persisted-locale → OS-locale → default" (FR-009). The persisted-locale source is `settings.appearance.locale`, not the `lf-lang` cookie that the library would write — so the integration point is: read `settings.appearance.locale` first at boot; if `None`-equivalent (i.e. `Locale::default()`, the `#[serde(default)]` path), pass `None` to the `I18nContextProvider`'s `initial_locale` prop so the library's OS-detection path runs. |
| **Pluralization** | ICU CLDR plural rules via the optional `plurals` cargo feature (`leptos_i18n/Cargo.toml:64`). Off by default; can be enabled if any English string in FR-013 needs count-sensitive phrasing. **[BEST-GUESS PM DECISION]** Enabled in `src/Cargo.toml` only if a key in scope needs it; otherwise the feature stays off to keep the WASM bundle lean. The "1 session" / "5 sessions" pattern (Spec Edge Case) is the only candidate; tasks-phase keys decide. |
| **Bundle size** | Single-locale baseline at `~10–15 KB` of generated code (estimate from a four-locale `leptos_i18n` example crate's `wasm-opt -Os` output, per the library author's `examples/csr/counter` build). Each additional locale adds linearly with the catalogue's string-table size — four locales × ~10 KB of unique strings = `~40 KB` of catalogue overhead. ICU CLDR data is opt-in via `icu_compiled_data` (already in `default`) and adds `~50 KB` to WASM if any ICU feature is enabled (off for the v1 four-locale plain-text pass). Total expected WASM-size delta: `~50–100 KB` — comfortably under the spec's implicit "small" budget. No specific SC measures this; SC-008 measures network-egress, not bundle size. |
| **Trunk compatibility** | **YES.** The example at `examples/csr/counter/` builds with Trunk and the `csr` cargo feature. `build.rs` is only needed for `0.6.x` (it moved out of `[package.metadata.leptos-i18n]` and into a `build.rs` invocation for the `0.6.x` series); on `0.5.11` the Trunk-compatible configuration is the `Cargo.toml` metadata block — no `build.rs` required. |
| **Tauri-WKWebView compatibility** | **YES.** The library emits pure WASM with no DOM-side runtime requirements beyond `wasm-bindgen` and `js-sys` (already direct deps of presto). It does NOT spawn workers, does NOT touch `IndexedDB`, does NOT issue `fetch` calls — all four locales' catalogues are bundled into the WASM binary at compile time per the `load_locales!()` proc-macro's `include_str!`-style read of `locales/*.json`. WKWebView, WebView2, and WebKitGTK all execute this without any platform-specific concession. |

### Candidate 2: `leptos-fluent` (mondeja)

| Aspect | Finding |
|---|---|
| **Latest stable version** | `0.3.1` (released 2025-12-29). Latest `0.2.x` was `0.2.21` (2025-12-12); `0.3.x` is the current series. Compatible with `leptos = "0.7"` per the README's `leptos-fluent = "0.3"` install line and observed in the example branch. |
| **Author / maintenance** | Álvaro Mondéjar Rubio (`@mondeja`). 92 stars; 23 open issues; default branch last pushed 2026-04-24. Active but lower velocity than `leptos_i18n`. |
| **Catalogue format** | **Fluent FTL** (`.ftl` files; the BCP-47-aware Mozilla Fluent format with ICU CLDR plural selectors and rich interpolation). One file per locale at `locales/<locale>/main.ftl`. Supports nested namespaces. |
| **Compile-time key checking** | **PARTIAL.** The `tr!("hello-world")` macro accepts an **arbitrary string literal** — it is NOT a typed Rust identifier. A key typo passes `cargo build` and renders the literal string at runtime (or a fallback) instead of failing the build. Optional `check_translations: true` config field on the `leptos_fluent! {}` provider macro performs an additional debug-mode-only consistency check against a glob of `.rs` files, but the check is **opt-in**, **debug-only** (`#[cfg(all(debug_assertions, not(feature = "ssr")))]`), and operates on a separate text-grep pass rather than the type system — a release build with a typo'd key compiles cleanly and renders the typo'd string. **This is incompatible with the spec's FR-005** (typed-key API, compile-time-checked, no stringly-typed lookups). |
| **Missing-translation behaviour** | When `check_translations: true` is on AND the build is `debug_assertions` AND not `ssr`, a missing key produces a `cargo build` failure. Otherwise, runtime fallback to the default locale; no warning, no error. The release build under `cargo build --release` (which presto's CI uses for the Trunk build per `.agentex.yml` test stage) skips the check entirely. **This is incompatible with FR-006** (build-time enforcement). |
| **Locale-switching API** | Signal-based. `i18n.language.get()` / `i18n.language.set(lang)` via `expect_context::<I18n>()`. The `lang` parameter is a `&'static Language` rather than a Rust enum variant — slightly weaker typing than `leptos_i18n`'s `Locale` enum. |
| **OS locale detection** | **Built-in.** The `leptos_fluent! {}` macro accepts `initial_language_from_navigator: true` (reads `navigator.languages`), `initial_language_from_accept_language_header: true` (SSR), and `initial_language_from_system: true` (gated on the `system` cargo feature, desktop Tauri). Plus cookie / local-storage persistence. |
| **Pluralization** | ICU MessageFormat-style plural selectors via Fluent's `{ $count -> [one] {...} *[other] {...} }` syntax — built into Fluent itself, no extra cargo feature. |
| **Bundle size** | Comparable to `leptos_i18n` — `fluent-templates` is the underlying parser; the proc-macro embeds `.ftl` source strings. Estimated `~50–100 KB` for four locales (Fluent's ICU plural-rule data is bundled alongside, slightly larger than `leptos_i18n` with the `plurals` feature off). |
| **Trunk compatibility** | **YES.** README mentions `cargo-leptos`'s `watch-additional-files = ["locales"]` config for hot-reloading. Trunk users add the equivalent file-watch step. |
| **Tauri-WKWebView compatibility** | **YES.** Pure WASM; no platform-specific surface. |

### Why `leptos_i18n` wins

The spec's hard requirements are FR-005 (typed-key, compile-time-checked) and
FR-006 (missing keys fail the build, not the render). `leptos_i18n` satisfies
both by construction:

- **FR-005 satisfied**: `t!(i18n, focus_mode_badge)` is a typed Rust method
  call on the `I18nKeys` struct generated by `load_locales!()`. A typo
  `t!(i18n, focus_mod_badge)` produces `error[E0599]: no method named
  'focus_mod_badge'` — the type system rejects it before render. No
  stringly-typed lookup path exists in the public API surface.
- **FR-006 satisfied**: `MissingKey` warnings from the proc-macro promote to
  `deprecated` lint diagnostics. The presto CI `clippy -- -D warnings` gate
  (existing — `.agentex.yml` `lint:`) turns these into hard failures with no
  additional CI script needed.

`leptos-fluent`'s `tr!("hello-world")` API is stringly-typed by design — a key
typo passes `cargo build` and renders incorrectly at runtime. The
`check_translations: true` opt-in is debug-mode-only and runs a separate
text-grep pass, not a type-system check. Adopting `leptos-fluent` would
require either rejecting FR-005 outright or building a bespoke type-safety
wrapper around `tr!()` — disproportionate work versus picking the library
that already satisfies the contract.

`leptos_i18n`'s additional wins:

- **OS detection out of the box** (`leptos-use::use_locales` integration; no
  bespoke `web_sys::window().navigator().language()` parser needed for the
  primary path) satisfies FR-009 / FR-010 / Story 4.
- **`Locale` is a closed Rust enum** generated by the proc-macro — matches
  the spec's Key Entity definition (FR-002) more directly than
  `leptos-fluent`'s `&'static Language` reference type.
- **Higher star count and shorter time-since-last-commit** than
  `leptos-fluent` per the snapshots above — both are actively maintained,
  but `leptos_i18n` is on the higher-velocity side.
- **JSON catalogue format** is the lowest-friction option for hand-curated
  translations versus Fluent FTL's syntactic overhead — a 30-key catalogue
  in JSON is mechanically obvious to a non-i18n-expert contributor, whereas
  FTL requires familiarity with Fluent's variable-binding syntax.

The only material loss versus `leptos-fluent` is built-in plural-rule access
without an opt-in cargo feature. Since the spec's Edge Cases entry
explicitly accepts "rewriting count-sensitive English to avoid pluralization"
(Spec Edge Cases bullet 5) and the `plurals` feature is available behind a
flag if needed, this is a soft loss.

**Cost of choosing `leptos_i18n`**:

1. The library version is locked to the `0.5.x` series until presto upgrades
   to `leptos = "0.8"`. The `0.6.x` series (current latest) requires
   `leptos = "0.8"`. A future presto leptos-upgrade spec lands the
   simultaneous `leptos_i18n` major bump. No mid-feature surprise.
2. `leptos-use` becomes a transitive dependency. Two new direct deps
   transitively from the `0.5.11` Cargo.toml: `icu_locid` (BCP-47 parsing),
   `leptos-use` (OS-detection helper). `Cargo.lock` grows by their dep tree
   (estimated ~12–18 additional crates including `icu_locid`,
   `default-struct-builder`, `codee`, `typed-builder`, and their
   transitives). All pure-Rust, no `cc`-compiled C dependencies, no
   platform-specific gating.
3. `[package.metadata.leptos-i18n]` Cargo.toml metadata block. Already a
   precedent (the project uses `[package.metadata.leptos]` in some configs);
   no novelty.

### Known limitation: first-paint locale flash (ripple R-001)

`<I18nContextProvider>` in `leptos_i18n` `0.5.11` does NOT accept an
`initial_locale` prop — that API was added in the `0.6.x` series (which
requires `leptos = "0.8"` and is therefore blocked by the same upgrade
gate as item 1 above). With `settings.appearance.locale = Some(De)`
persisted on an English-OS machine, the boot sequence is:

1. `<I18nContextProvider>` mounts and runs its own OS-detection path
   (`leptos-use::use_locales` → `navigator.languages`), populating the
   library's locale signal with the OS default (English in the
   example).
2. First paint renders with English strings — `t!(i18n, …)` reads the
   library's locale at that point.
3. `LocaleSync`'s Effect fires on the next reactive tick, reads
   `settings.appearance.locale = Some(De)`, and calls
   `i18n.set_locale(De)`. The DOM re-renders into German.

The visible result is a single-frame flash of English before German
strings appear. The flash is bounded by the reactive tick budget
(sub-50ms in CSR builds) but is a measurable UX regression for users
whose persisted locale disagrees with their OS locale.

**Mitigation path** (deferred): a sentinel-locale loader that
withholds the provider mount until settings are loaded would
eliminate the flash but adds boot-time complexity (a settings-ready
gate above the entire view tree, currently absent — settings load
inline alongside the rest of the app).

**Disposition**: accept the flash for v1. The follow-up
leptos-upgrade cycle that bumps the workspace to `leptos = "0.8" +
leptos_i18n = "0.6.x"` unlocks the `initial_locale` prop, at which
point `LocaleSync` collapses into a one-shot prop pass and the flash
disappears at zero ongoing complexity cost. The fix is therefore
upstream-blocked, not architecturally blocked.

## Decision 2 — Catalogue file format: JSON (library default)

**Chosen**: JSON. One file per locale at `src/locales/<locale>.json`. Four files
total (`en.json`, `de.json`, `it.json`, `tr.json`). The `[package.metadata.
leptos-i18n] default = "en" locales = ["en", "de", "it", "tr"]` block in
`src/Cargo.toml` declares the locale list and the source-of-truth locale.

**Rejected alternatives**:

- **YAML** (`yaml` cargo feature on `leptos_i18n`): allows comments and
  multiline strings, which is a readability win for translator notes. But
  YAML parsing has known whitespace-sensitivity pitfalls (a hand-edited
  catalogue with a stray tab character fails the parser silently in some
  YAML implementations). JSON's strict syntax is the safer trade for
  hand-curated catalogues per Spec FR-027 / FR-029 (no translator tooling,
  no LLM auto-translation, files committed to repo and edited by humans).
- **JSON5** (`json5` cargo feature on `leptos_i18n`): allows comments, which
  is the only material win over JSON. Adds a build-time dependency on the
  `json5` parser crate. The translator-note-in-comment use case can be
  satisfied by a sibling `<locale>.notes.md` file alongside each catalogue
  if needed — out of scope for v1 (FR-027).
- **Fluent FTL**: would require the `leptos-fluent` library (rejected above
  on the typed-key axis). The FTL format itself is technically superior for
  complex pluralization, but the spec accepts simple plural-rewriting
  (Edge Cases bullet 5) so the format-side advantage is moot.
- **TOML**: not supported by `leptos_i18n` (`leptos_i18n_parser` accepts
  JSON / JSON5 / YAML only per `serde_error` arms in the parser source).
  Off the table by library construction.
- **Inline Rust macro arms** (`declare_locales! { ... }` instead of
  `load_locales!()`): an option for very small catalogues (≤10 keys), but
  the spec's FR-013 scope is in the 100+ keys range across timer / sidebar /
  settings / statistics / daily / update / tag / calendar. Inline macro
  arms would balloon the Rust source files past readability. The on-disk
  JSON approach is the right shape for catalogues of this size.

**Reasons**:

1. **JSON is the library default** — zero extra feature flag, zero extra
   build-time crate. The path-of-least-resistance integration.
2. **Strict syntax suits hand-curated translation files** per FR-027 /
   FR-029. No whitespace surprises, no comment-syntax ambiguity, no
   accidental multiline-string indentation drift.
3. **Editor support is universal** — every editor / IDE the project's
   contributors might use has a JSON syntax mode out of the box. YAML and
   FTL have weaker tool support.
4. **JSON5 / YAML can be adopted in a follow-up** by flipping the
   `leptos_i18n` cargo feature flag; no on-disk migration is needed —
   the per-locale file format is configurable from the start (the
   `json_files` cargo feature is the default; switching to `yaml_files`
   in a follow-up would rename the files but otherwise preserve the
   directory layout).

**Cost of choosing JSON**: no comments inside the catalogue files. If a
specific key needs translator context (e.g. "this is the timer's mode
badge — keep it ≤8 characters"), the context lives in a sibling
documentation file or in a special-conventioned key like
`_focus_mode_badge.context` that the macro ignores. Out of scope for v1.

## Decision 3 — Missing-translation enforcement: clippy `-D warnings` (no bespoke script)

**Chosen**: rely on the existing `cargo clippy --workspace --all-targets
--frozen -- -D warnings -W clippy::pedantic` CI gate (declared in
`.agentex.yml` `lint:`) to promote `leptos_i18n`'s `MissingKey` warning to a
hard build error. **No new CI script is added.** No new entry in `.agentex.yml`.

**Rejected alternatives**:

- **Bespoke `scripts/check-translation-completeness.sh`**: would parse each
  `locales/*.json` file, compute the key-set difference vs `en.json`, and
  fail loudly on any missing key. Redundant — the proc-macro already
  performs this check and emits a diagnostic; promoting it through clippy
  is one less script to maintain.
- **`#![deny(deprecated)]` at the crate root**: would lock the
  missing-key gate to the presto crate specifically. Already covered by
  the workspace-level `-D warnings` clippy flag at the CI invocation site;
  duplicating it at the crate root is no-op.
- **Pre-commit hook**: a pre-commit hook running the same clippy
  invocation would catch missing keys before push, mirroring the
  lockfile-drift hook pattern at `.githooks/pre-commit`. **[BEST-GUESS PM
  DECISION]** Not added in v1; the existing `cargo clippy` gate in CI is
  sufficient. Pre-commit is a contributor-experience improvement, not a
  correctness requirement, and can be added in a follow-up if missing-key
  catches start landing late in the PR cycle.

**Reasons**:

1. **The proc-macro is already the source of truth.** `leptos_i18n` emits
   `MissingKey` warnings as `#[deprecated]` annotations on a generated
   `warnings()` fn that is called from the macro's output. Clippy's
   `deprecated` lint is in `-D warnings` per the workspace lint block at
   `Cargo.toml` (workspace root: `[workspace.lints.clippy] all = "deny",
   pedantic = "deny", nursery = "deny"` plus the CI invocation's
   `-D warnings`). A missing key fails CI with no extra work.
2. **No script-maintenance burden.** A bespoke script would have its own
   bug surface (path globs, JSON parsing, key-set extraction); piggy-backing
   on the proc-macro's check eliminates that risk.
3. **The fail message is precise.** `MissingKey { locale, key_path }`
   surfaces as `Missing key "<path>" in locale "<locale>"` directly in
   the build output, which is more actionable than a bespoke script's
   summary table.

**Cost of choosing the proc-macro-promotion path**:

1. The missing-key gate runs at clippy time, not at the dedicated lint
   step. A contributor running `cargo build` alone (without clippy) would
   see the missing-key warning but not fail their build. The PR-time CI
   gate still catches it. Mirror to the established
   `clippy::pedantic`-deny posture (Principle X).
2. **Surplus-key gate**: `leptos_i18n` also emits `SurplusKey` for keys
   present in a non-default locale but absent from `en`. This too becomes
   a `deprecated`-lint failure under `-D warnings`. **[BEST-GUESS PM
   DECISION]** This is desirable — it prevents stale translations from
   accumulating after an English-side key removal. The asymmetry is
   deliberate per Spec A13 (English is the source-of-truth locale).

## Non-decisions (intentionally deferred to tasks-phase)

- **Exact English copy of every key.** Tasks-phase will enumerate the
  FR-013 scope into specific key-value pairs in `en.json`. The translation
  pass into `de.json` / `it.json` / `tr.json` follows.
- **Whether to enable the `plurals` cargo feature.** Decided at
  tasks-phase based on whether any FR-013 string survives the
  "rewrite-to-avoid-plurals" trim. The plan does not block on this
  pre-decision.
- **Whether to add a `locales/<locale>.notes.md` translator-context
  sibling for the keys that benefit from it.** Tasks-phase concern; the
  plan's directory tree only commits to `<locale>.json` for now.
- **Exact translation quality / native-speaker review process.** Spec
  FR-029 explicitly puts this off the plan-phase table; the plan only
  commits to the typed-key surface and the four-locale catalogue
  scaffolding, not to translation quality.
