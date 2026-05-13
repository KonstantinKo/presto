# Data Model — Feature 005 (Multi-Locale UI With In-App Language Switcher)

Three shapes evolve / are introduced.

1. `Locale` — new closed sum-type enum on the IPC wire.
2. `AppearanceSettings` — one new field appended to an existing struct.
3. Message-catalogue file layout + the in-memory locale signal type
   (library-provided; documented here for cross-reference).

The on-disk schema is unchanged structurally — only a field-level addition
inside the existing `appearance` block. No migration is required because
the new field carries `#[serde(default)]` (defaulting to `None`).

**Fix A rationale**: the field type is `Option<Locale>`, NOT `Locale`. Using
`Option<Locale>` is critical for correctness: a user who explicitly picks
"English" writes `Some(Locale::En)` to disk. On subsequent cold starts the
resolver sees `Some(_)` and skips OS detection — honoring FR-011. With a
plain `Locale` type and a `Default` of `Locale::En`, a German user who
explicitly picks English would have their locale silently reverted to
German on the next cold start (OS detection fires because `Locale::En ==
Locale::default()` — the resolver cannot distinguish "explicit English"
from "never picked anything").

## 1. `Locale` (new — `crates/presto-ipc/src/settings.rs`)

Closed four-variant Rust enum. Wire shape is lowercase per Spec FR-002 /
A5 — diverges from `AmbientSoundType` / `StatusBarDisplay`'s kebab-case
precedent because two-letter ISO-639-1 codes have no internal word
boundary that kebab-case would clarify.

```rust
/// User-selectable UI locale (feature 005).
///
/// Closed sum type: four variants, one per supported locale. The library
/// (`leptos_i18n`) generates its own `Locale` enum from `locales/*.json`
/// as part of the `load_locales!()` proc-macro expansion; this IPC-side
/// enum is the wire-shape projection that `AppearanceSettings.locale`
/// serialises through. The two enums are kept variant-aligned by the
/// translation-completeness check (the proc-macro's `MissingKey`
/// warning promotes to a `deprecated`-lint failure under
/// `cargo clippy -- -D warnings`).
///
/// Wire shape is `lowercase` strings (`"en"`, `"de"`, `"it"`, `"tr"`)
/// matching the `theme` field's lowercase convention at
/// `crates/presto-ipc/src/settings.rs:121` (`"auto"` / `"light"` /
/// `"dark"`) — two-letter ISO-639-1 codes have no internal word boundary
/// so kebab-case is unnecessary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    /// English (source-of-truth locale per Spec A13).
    /// Wire string `"en"`. The `#[default]` attribute ties this variant
    /// to the `#[serde(default)]` attribute on
    /// `AppearanceSettings.locale` — a missing field on the wire
    /// deserialises to `Locale::En`.
    #[default]
    En,
    /// Deutsch — wire string `"de"`.
    De,
    /// Italiano — wire string `"it"`.
    It,
    /// Türkçe — wire string `"tr"`.
    Tr,
}
```

### Wire-shape ↔ variant mapping

| Variant | Wire string | Dropdown native self-name (per Spec FR-015) |
|---|---|---|
| `En` | `"en"` | `English` |
| `De` | `"de"` | `Deutsch` |
| `It` | `"it"` | `Italiano` |
| `Tr` | `"tr"` | `Türkçe` |

The native self-names in the dropdown are constants in the locale-switcher
view code (`src/src/components/settings/general.rs`) — they are NEVER
persisted on the wire and NEVER re-translated when the active locale
changes (FR-015 / Spec Story 1 AC 4). Only the lowercase strings cross
the IPC boundary.

### `Default` impl

Returns `Locale::En` via `#[default]` on the variant — derived
automatically by `#[derive(Default)]`. This is the default value used by
`#[serde(default)]` on the `locale` field of `AppearanceSettings`.

### Out-of-set deserialisation

A hand-edited / corrupted `settings.json` carrying
`appearance.locale = "fr"` (or `null`, `""`, `42`) fails serde's strict
enum deserialisation; the `#[serde(default)]` attribute on the field
catches the failure and substitutes `Locale::En`. Asserted by the
`locale_unsupported_wire_falls_back_to_en` test (Spec Story 2 AC 4 /
FR-004 / SC-002 supporting case).

## 2. `AppearanceSettings` evolution (`crates/presto-ipc/src/settings.rs`)

One new field appended to the existing struct. The `theme` /
`timer_theme` fields are the existing `#[serde(default = "...")]`
serde-evolution precedent at `crates/presto-ipc/src/settings.rs:120-123`.

### Before (feature 004 baseline at `:117-133`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AppearanceSettings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_timer_theme")]
    pub timer_theme: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            timer_theme: default_timer_theme(),
        }
    }
}
```

### After (feature 005)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AppearanceSettings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_timer_theme")]
    pub timer_theme: String,
    /// (feature 005 — UI locale)
    ///
    /// `None` = "user has never explicitly chosen a locale" — the resolver
    /// runs OS detection on cold start. `Some(Locale)` = "user explicitly
    /// chose this locale" — the resolver uses the persisted value verbatim,
    /// even when `Some(Locale::En)` (explicit English must not trigger OS
    /// re-detection on a German OS). This `Option<Locale>` discriminant is
    /// the authoritative "explicit vs. default" signal per FR-009 / FR-011.
    ///
    /// Wire shape: `None` is absent / `null` on the wire (the
    /// `#[serde(default)]` attribute handles the missing-key case); `Some(l)`
    /// serialises as the lowercase string `"en"` / `"de"` / `"it"` / `"tr"`
    /// via `#[serde(rename_all = "lowercase")]` on the inner enum.
    /// An out-of-set wire value (`"fr"`, `""`, `42`, `"english"`) fails serde
    /// and substitutes `None` via the `#[serde(default)]` fallback.
    #[serde(default)]
    pub locale: Option<Locale>,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            timer_theme: default_timer_theme(),
            locale: None,
        }
    }
}
```

### Field-level invariants

| Field | Type | Default | Boundary | Wire encoding |
|---|---|---|---|---|
| `locale` | `Option<Locale>` | `None` | UI dropdown; closed enum | `None` → absent/null; `Some(En)` → `"en"`; `Some(De)` → `"de"`; `Some(It)` → `"it"`; `Some(Tr)` → `"tr"` |

`None` is the default (no explicit locale chosen). An out-of-set wire value
falls back to `None` per the `#[serde(default)]` semantic — never panics,
never logs an error toast, never opens a recovery prompt.

### Legacy compatibility

Pre-feature-005 `appearance` JSON missing the `locale` key deserialises
to `None` (test `locale_legacy_field_defaults_none`). A new-build
`appearance` record persisted with each supported locale (`Some(De)`,
`Some(It)`, `Some(Tr)`, `Some(En)`) round-trips byte-stable through serde
with lowercase wire encoding (test `locale_round_trip`). A critical
invariant: `Some(Locale::En)` and `None` MUST round-trip as distinct values
— the serde round-trip test asserts that a record persisted with `locale:
Some(En)` reloads as `Some(En)`, NOT `None`. A new-build record containing
the existing `theme` and `timer_theme` fields plus the new `locale` field
survives the round-trip with all three fields preserved byte-stable (covered
under `locale_round_trip` against a full-record fixture per Spec Story
2 AC 5).

## 3. Message catalogue (new — `src/locales/`)

Library-dependent. `leptos_i18n` v0.5.11 with its default `json_files`
feature reads one JSON file per locale at `<crate-root>/locales/
<locale>.json`. For the presto frontend crate at `src/Cargo.toml`, this
expands to `src/locales/{en,de,it,tr}.json`. The locale list and the
source-of-truth (default) locale are declared in `src/Cargo.toml`'s
`[package.metadata.leptos-i18n]` block:

```toml
[package.metadata.leptos-i18n]
default = "en"
locales = ["en", "de", "it", "tr"]
```

### File layout

```text
src/
├── Cargo.toml                  # [package.metadata.leptos-i18n] block
└── locales/
    ├── en.json                 # Source-of-truth catalogue (Spec A13)
    ├── de.json                 # German translation
    ├── it.json                 # Italian translation
    └── tr.json                 # Turkish translation
```

### Catalogue key-set example

```json
// en.json — source of truth (Spec A13)
{
  "timer": {
    "mode_focus": "Focus",
    "mode_break": "Break",
    "mode_long_break": "Long Break",
    "state_paused": "Paused",
    "state_auto_paused": "Auto-paused",
    "state_overtime": "Overtime",
    "ctrl_reset": "Reset",
    "ctrl_undo": "Undo",
    "ctrl_start": "Start",
    "ctrl_pause": "Pause",
    "ctrl_resume": "Resume",
    "ctrl_skip": "Skip session"
  },
  "settings": {
    "general": {
      "language_label": "Language",
      "focus_duration_label": "Focus Duration (minutes):"
    },
    "auto_save_ok": "Settings saved",
    "auto_save_err": "Failed to save settings"
  }
  // ...
}
```

The exact key shape (flat vs nested namespaces) is a tasks-phase
decision. `leptos_i18n` supports both via its "Namespaces" feature
(top-level `_namespaces_` declaration in the catalogue file) and via
nested key paths inside a single file. **[BEST-GUESS PM DECISION]**
The plan defaults to a single nested-keys-per-view-area structure
(one top-level key per major view: `timer`, `settings`, `statistics`,
`daily`, `calendar`, `tag`, `update`, `sidebar`) — the namespace
feature isn't necessary for a four-locale, ~100-key surface.

### Wire shape constraint

Catalogue files are read at **compile time** by the `load_locales!()`
proc-macro and embedded into the WASM binary as Rust `&'static str`
constants. They are NEVER read at runtime, NEVER fetched from a
network URL, NEVER loaded from a path outside the bundle. The Tauri
auto-updater's release-check payload is unaffected by this feature
(per Spec FR-008 / FR-019).

### Translation-completeness invariant

Every key present in `en.json` MUST be present in `de.json`,
`it.json`, and `tr.json` — verified at compile time by the
`leptos_i18n` proc-macro's `MissingKey` warning, which the presto
workspace's `cargo clippy --workspace --all-targets --frozen --
-D warnings -W clippy::pedantic` CI invocation (`.agentex.yml`
`lint:`) promotes to a hard build failure via the `deprecated` lint
(the macro emits `MissingKey` warnings as `#[deprecated(note =
"Missing key ...")]` annotations on a generated `warnings()` fn).

The asymmetry is deliberate per Spec A13: `en.json` is the
source-of-truth. A key removed from `en.json` is also a build-time
failure if it remains in any of the three target locales (the
proc-macro's `SurplusKey` warning, same promotion path) — preventing
stale translations from accumulating across English-side edits.

## 4. In-memory locale signal (UI-side runtime state — NOT serialised)

Library-provided. `leptos_i18n` v0.5.11 exposes the active locale as
a Leptos reactive signal accessible via the `I18nContext` struct:

```rust
// Inside any Leptos component:
let i18n = use_i18n();           // Retrieves I18nContext from Leptos context.
let current = i18n.get_locale(); // Returns Locale (the library-generated enum).
i18n.set_locale(Locale::de);     // Setter — reactively re-renders all consumers.
```

The library-generated `Locale` enum lives in the macro-emitted `i18n`
module (e.g. `i18n::Locale::en`, `i18n::Locale::de`, `i18n::Locale::it`,
`i18n::Locale::tr`); its variants are **lowercase** because they mirror
the JSON file stems on disk. There is NO requirement that the IPC-side
`presto_ipc::Locale` and the library-generated `i18n::Locale` are the
same type — they are two separate closed sum types kept variant-aligned
by hand (and by the proc-macro's translation-completeness check, which
asserts the locale set declared in `Cargo.toml`'s
`[package.metadata.leptos-i18n] locales` matches the on-disk file
list).

### IPC-side ↔ library-side `Locale` conversion

A small `From<presto_ipc::Locale> for i18n::Locale` impl (and its
inverse) lives in `src/src/i18n.rs` (the new module) to translate
between the wire-side enum and the library-side enum. Both impls are
total (the variant sets are by-design 1:1):

```rust
impl From<presto_ipc::Locale> for i18n::Locale {
    fn from(l: presto_ipc::Locale) -> Self {
        match l {
            presto_ipc::Locale::En => i18n::Locale::en,
            presto_ipc::Locale::De => i18n::Locale::de,
            presto_ipc::Locale::It => i18n::Locale::it,
            presto_ipc::Locale::Tr => i18n::Locale::tr,
        }
    }
}
// And the inverse: impl From<i18n::Locale> for presto_ipc::Locale.
```

### Non-persistence rationale

The library-side locale signal is intentionally NOT serialised. The
authoritative on-disk value is `AppearanceSettings.locale` (the IPC
type). At app boot, the resolution chain (FR-009) runs once:

1. Read `settings.appearance.locale` (the IPC value — type `Option<Locale>`).
2. If `Some(locale)` — any variant, including `Some(En)` — use it verbatim.
   The `Some(_)` discriminant means "user explicitly chose this locale";
   OS detection is NOT run (FR-011 / Fix A).
3. If `None` (legacy record or fresh install with no explicit choice), invoke
   the `leptos_i18n` library's OS-detection path via
   `leptos-use::use_locales`. Map the first OS-locale's two-letter
   prefix to one of the four supported variants; fall back to
   `Locale::En` if no match.
4. Seed the `I18nContext`'s locale signal with the resolved value
   via the `<I18nContextProvider initial_locale=...>` prop.

Subsequent locale changes go through the Settings → General
dropdown's `on:change` handler, which writes to
`settings.appearance.locale` (the IPC value, persisted by the
existing debounced settings-autosave Effect at `src/src/app.rs:215+`)
AND mirrors the change into the library's `i18n.set_locale(...)` so
view re-rendering kicks in immediately. The two-way binding is
handled by a small Leptos `Effect` in `src/src/i18n.rs` that watches
`settings.appearance.locale` for changes and forwards them to the
library; the dropdown writes to the IPC signal only, and the Effect
propagates.

### Test boundary

The locale-resolution function (`resolve_initial_locale`) is extracted
into a pure helper in `src/src/i18n.rs` for unit-test isolation
(FR-023). Its signature:

```rust
/// Resolve the cold-start locale per FR-009's strict precedence:
/// (1) persisted IPC value if `Some(_)` (any variant, including En),
/// (2) OS-locale two-letter prefix match if persisted is `None`,
/// (3) fall back to `Locale::En`.
///
/// The `Option<Locale>` discriminant is the authoritative "explicit vs.
/// default" signal — `Some(Locale::En)` (explicit English) and `None`
/// (no choice yet) are NOT equivalent. Using `None` here, not
/// `Locale::En == Locale::default()`, is the critical Fix A invariant:
/// a German-OS user who explicitly picks "English" writes `Some(En)`;
/// their next cold start sees `Some(_)` and skips OS detection.
///
/// Pure function: takes the persisted value and the OS-detected
/// language strings as parameters, returns the resolved `Locale`.
/// The `web_sys` call to `navigator.languages` happens in the caller
/// (in `src/src/app.rs` boot path); this function is testable in
/// isolation under `wasm-pack test --node` without a DOM.
pub fn resolve_initial_locale(
    persisted: Option<presto_ipc::Locale>,
    os_languages: &[String],
) -> presto_ipc::Locale {
    // (1) If the user has ever explicitly saved a locale, use it verbatim.
    // This covers Some(Locale::En) — explicit English must NOT trigger
    // OS re-detection on a German-locale OS.
    if let Some(locale) = persisted {
        return locale;
    }
    // (2) No explicit choice yet — OS detection: first OS-locale whose
    // two-letter prefix matches a supported variant wins.
    for lang in os_languages {
        if let Some(matched) = match_two_letter_prefix(lang) {
            return matched;
        }
    }
    // (3) Default fallback.
    presto_ipc::Locale::En
}

fn match_two_letter_prefix(lang: &str) -> Option<presto_ipc::Locale> {
    let prefix = lang.split(['-', '_']).next()?.to_lowercase();
    match prefix.as_str() {
        "en" => Some(presto_ipc::Locale::En),
        "de" => Some(presto_ipc::Locale::De),
        "it" => Some(presto_ipc::Locale::It),
        "tr" => Some(presto_ipc::Locale::Tr),
        _ => None,
    }
}
```

### Non-engine scope

Per Spec A15 / SC-011 / Principle I, neither the IPC `Locale` enum
nor the library's locale signal is read by anything under
`src/src/engine/`. The engine remains locale-agnostic; the engine-
purity grep gate at `scripts/check-engine-purity.sh` already
enforces zero `web_sys` references under the engine path, and this
feature adds no engine-side reads of `Locale` or `t!(...)` either.
The engine's session events emit timer-mode codes (`Focus` /
`Break` / `LongBreak`), not user-visible mode strings — the UI-side
mode badge renders the appropriate localised string via `t!(i18n,
timer.mode_focus)` (or the equivalent typed key) per FR-013.
