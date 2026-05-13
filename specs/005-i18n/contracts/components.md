# Component & Side-Effect Contracts — Feature 005

Four contracts.

1. `Locale` — wire-shape enum (closed sum type, lowercase strings).
2. `AppearanceSettings.locale` — single-field wire evolution; `#[serde(default)]`
   following the `theme` / `timer_theme` precedent.
3. `leptos_i18n` API surface — typed-key macro, locale-switcher API,
   OS-detection helper, locale-changed signal.
4. Translation-completeness invariant — every key in `en.json` exists in
   `de.json` / `it.json` / `tr.json`, enforced at compile time.

## 1. `Locale` — wire-shape contract

### Closed sum type (four variants)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    #[default]
    En,
    De,
    It,
    Tr,
}
```

The `#[serde(rename_all = "lowercase")]` attribute matches the
existing `theme` field's lowercase string convention at
`crates/presto-ipc/src/settings.rs:121-123` (`"auto"` / `"light"` /
`"dark"`) — two-letter ISO-639-1 codes have no internal word boundary
so kebab-case is unnecessary. Diverges from `AmbientSoundType` /
`StatusBarDisplay`'s kebab-case precedent (those have multi-word
variants like `WhiteNoise → "white-noise"` where kebab-case carries
information).

### Wire-shape assertion table

| Variant | Serialised JSON value (in `appearance.locale`) |
|---|---|
| `Locale::En` | `"en"` |
| `Locale::De` | `"de"` |
| `Locale::It` | `"it"` |
| `Locale::Tr` | `"tr"` |

The mapping is asserted byte-stable by
`presto_ipc::settings::tests::locale_serialises_lowercase` (one of
the three RED-first tests).

### `Default` impl

Returns `Locale::En` via `#[default]` on the variant — derived
automatically by `#[derive(Default)]`. This is the default value
used by `#[serde(default)]` on the `locale` field of
`AppearanceSettings`.

### Out-of-set wire value behaviour

A hand-edited / corrupted `settings.json` containing
`appearance.locale = "fr"` (or `null`, `""`, `42`, `"english"`) fails
serde's strict enum deserialisation. The `#[serde(default)]`
attribute on the field catches the failure and substitutes
`Locale::En`. Asserted by
`presto_ipc::settings::tests::locale_unsupported_wire_falls_back_to_en`
(per Spec Story 2 AC 4 / FR-004).

### Variant alignment with library-side `i18n::Locale`

The `leptos_i18n::load_locales!()` proc-macro generates a parallel
`i18n::Locale` enum in `src/src/i18n.rs`'s namespace with lowercase
variant names matching the on-disk file stems (`i18n::Locale::en`,
`i18n::Locale::de`, etc.). The two enums are kept variant-aligned
by hand; the alignment is documented by the
`From<presto_ipc::Locale> for i18n::Locale` impl (and its inverse)
in `src/src/i18n.rs`. The match-exhaustiveness check on these impls
fails compilation if either enum drifts.

## 2. `AppearanceSettings` evolution — wire-shape contract

One new field appended to the existing struct. The existing `theme`
field's `#[serde(default = "...")]` evolution at
`crates/presto-ipc/src/settings.rs:120-121` is the closest precedent:

```rust
    #[serde(default = "default_theme")]
    pub theme: String,
```

This feature's evolution uses `#[serde(default)]` (no `= "..."`
helper-fn form) because the `Locale::En` default flows from the
`#[default]` variant attribute, not from a separate `const fn`.

### Field addition

```rust
#[serde(default)]
pub locale: Option<Locale>,
```

`None` = "no explicit locale choice yet" (legacy settings or fresh install).
`Some(Locale)` = "user explicitly chose this locale" (including English).
The `Option` discriminant, not value-equality against `Locale::En`, is the
resolver's authoritative "explicit vs. default" signal (FR-009 / FR-011 / Fix A).

### `Default` impl addition

```rust
locale: None,
```

### Legacy fixture round-trip

A pre-feature-005 `appearance` JSON block (feature 002 / 003 / 004
baseline) lacking the `locale` key:

```json
{
  "theme": "auto",
  "timer_theme": "espresso"
}
```

Deserialises to:

| Field | Value | Source |
|---|---|---|
| `theme` | `"auto"` | Wire |
| `timer_theme` | `"espresso"` | Wire |
| `locale` | `None` | `#[serde(default)]` (feature 005 NEW) |

`None` means "no explicit locale chosen" — the resolver will consult OS
detection on cold start. This is distinct from `Some(Locale::En)` (explicit
English), which bypasses OS detection (FR-011 / Fix A).

This round-trip is asserted by
`presto_ipc::settings::tests::locale_legacy_field_defaults_none` — the
first of the three RED-first tests.

### Non-default round-trip

A new-build `appearance` JSON block:

```json
{
  "theme": "auto",
  "timer_theme": "espresso",
  "locale": "de"
}
```

Serialises and deserialises byte-stable for each of the four
variants; the `theme` and `timer_theme` values from features 002–004
survive the round-trip alongside the new field. Asserted by
`presto_ipc::settings::tests::locale_round_trip` — the second of the
three RED-first tests.

### Wire-shape constraint summary

- No new struct types on the wire (the new field lives inside the
  existing `AppearanceSettings`).
- No new `#[allow(...)]` annotations.
- No new Tauri commands. `save_settings` / `load_settings`
  transparently round-trip the new field as part of the existing
  `Settings` payload.

## 3. `leptos_i18n` API surface — library contract

`leptos_i18n` at `v0.5.11` exposes the following surface, wired
into the presto frontend via `src/src/i18n.rs` (new module) and
`src/src/app.rs` (boot path).

### Typed-key macro: `t!(i18n, key.path[, name = value, ...])`

The primary translation lookup site. **Compile-time-checked**:

```rust
use crate::i18n::*;

#[component]
fn TimerModeBadge(mode: TimerMode) -> impl IntoView {
    let i18n = use_i18n();
    let label = move || match mode {
        TimerMode::Focus => t!(i18n, timer.mode_focus),
        TimerMode::Break => t!(i18n, timer.mode_break),
        TimerMode::LongBreak => t!(i18n, timer.mode_long_break),
    };
    view! { <span class="mode-badge">{label}</span> }
}
```

**Key argument is a typed Rust identifier path** (not a string
literal). A typo `t!(i18n, timer.mod_focus)` produces
`error[E0599]: no method named 'mod_focus'` at compile time — the
type system rejects it before render. **Interpolation parameter
names are also typed**:

> **Attribute-value sites — use `td_string!` / `t_string!`, NOT `t!`**
>
> `t!(i18n, key.path)` returns `impl IntoView` (a reactive node for
> text-node children). For HTML attribute values (`aria-label=`,
> `title=`, `data-tooltip=`, `placeholder=`) the value must be a
> `String` or `Signal<String>`, not `impl IntoView`. Use
> `td_string!(i18n, key.path)` (owned `String`) or
> `t_string!(i18n, key.path)` (reactive `Signal<String>`) for those
> call sites. Example:
>
> ```rust
> // WRONG — t! returns impl IntoView, not String:
> // aria-label=move || t!(i18n, controls.reset_aria)
>
> // CORRECT — t_string! returns Signal<String>:
> aria-label=move || t_string!(i18n, controls.reset_aria)
> ```
>
> All `aria-label`, `title`, `data-tooltip`, and `placeholder`
> attributes in the FR-013 scope MUST use `t_string!` (or
> `td_string!` for non-reactive contexts), never `t!`. Both are
> compile-time-checked typed-key calls on the same `I18nKeys` struct
> — no stringly-typed fallback.
>
> *Note for implementors*: verify the exact macro name against the
> `leptos_i18n` v0.5.11 changelog at crate-add time. The public API
> in v0.5.x uses `td_string!` for owned strings and the signal form
> may vary; update this contract if the macro is renamed in the pinned
> version.

```rust
// In en.json: "session_count": "You completed {{ count }} sessions"
let i18n = use_i18n();
let n = move || count.get();
view! { <p>{t!(i18n, daily.session_count, count = n)}</p> }
// A typo `count = ...` -> `cont = ...` fails compilation with
// "no field named 'cont'".
```

### Locale-switcher API: `i18n.get_locale()` / `i18n.set_locale(...)`

The locale-active signal lives in the `I18nContext` returned by
`use_i18n()`. Both reads and writes are reactive:

```rust
use crate::i18n::*;

#[component]
fn LanguageDropdown() -> impl IntoView {
    let i18n = use_i18n();
    let current = move || i18n.get_locale();
    let on_change = move |ev: web_sys::Event| {
        let target_value = event_target_value(&ev);
        let new_locale = match target_value.as_str() {
            "en" => Locale::en,
            "de" => Locale::de,
            "it" => Locale::it,
            "tr" => Locale::tr,
            _ => return,
        };
        i18n.set_locale(new_locale);
    };
    view! {
        <select id="locale-selector" on:change=on_change prop:value=move || locale_wire(current())>
            <option value="en">"English"</option>
            <option value="de">"Deutsch"</option>
            <option value="it">"Italiano"</option>
            <option value="tr">"Türkçe"</option>
        </select>
    }
}
```

Note: the actual locale-switcher integration in presto writes to
`settings.appearance.locale` (the IPC signal) and a small `Effect`
in `src/src/i18n.rs` mirrors the change into
`i18n.set_locale(...)`. The dropdown writes ONE signal (the IPC
settings signal); the Effect forwards to the library. This keeps
the persistence path (via the existing debounced settings-autosave
Effect at `src/src/app.rs:215+`) the single source of truth for
on-disk state.

### OS-detection helper: `<I18nContextProvider initial_locale=...>`

`leptos_i18n` integrates with `leptos-use::use_locales` to read the
browser's preferred-language list (`navigator.languages`) at boot.
The provider's `initial_locale` prop accepts an `Option<Locale>`:

- `None` → run the library's full resolution chain (OS detection
  via `navigator.languages`, falling back to the default locale
  declared in `[package.metadata.leptos-i18n]`).
- `Some(locale)` → use the provided value verbatim, skipping
  detection.

The presto wiring:

```rust
// In src/src/app.rs boot path:
let initial = i18n::compute_initial_library_locale(&loaded_settings);
view! {
    <I18nContextProvider initial_locale=initial>
        // ... app tree
    </I18nContextProvider>
}
```

Where `compute_initial_library_locale` lives in `src/src/i18n.rs`:

```rust
/// Resolve the cold-start library-side locale.
///
/// If `settings.appearance.locale` is `Some(_)` (any variant, including
/// `Some(Locale::En)`) — the user has explicitly chosen a locale —
/// use it verbatim and skip OS detection (FR-011 / Fix A).
/// If `None` — no explicit choice yet — return `None` so the library's
/// OS-detection path runs.
///
/// The `Option<Locale>` discriminant is the authoritative "explicit vs.
/// default" signal. Value-equality against `Locale::En` MUST NOT be used
/// as a proxy (see Fix A rationale in data-model.md).
pub fn compute_initial_library_locale(
    settings: &Settings,
) -> Option<i18n::Locale> {
    settings.appearance.locale.map(|l| l.into())
}
```

### Pure-function locale-resolution helper for unit testing (FR-023)

`leptos_i18n`'s built-in OS-detection path is not directly testable
under `wasm-pack test --node` (no DOM). The pure helper
`resolve_initial_locale` in `src/src/i18n.rs` exists for the FR-023
test surface:

```rust
/// Pure function: given a persisted value and an OS-detected
/// language list, return the resolved `Locale` per FR-009's strict
/// precedence chain.
///
/// `persisted = None` → run OS detection (no explicit choice yet).
/// `persisted = Some(_)` → use the persisted value verbatim (any variant,
/// including `Some(Locale::En)` — explicit English bypasses OS detection).
pub fn resolve_initial_locale(
    persisted: Option<presto_ipc::Locale>,
    os_languages: &[String],
) -> presto_ipc::Locale;
```

Test coverage (FR-023, Fix A, Fix H):

1. `persisted = Some(De), os = ["en-US"]` → `De` (any `Some(_)` wins over OS).
2. `persisted = None, os = ["de-DE"]` → `De` (`None` falls through to OS
   detection; OS matches `de`).
3. `persisted = None, os = ["fr-FR"]` → `En` (no supported match; fallback).
4. `persisted = None, os = []` → `En` (no OS data; fallback — covers
   the `navigator.languages` empty / unavailable case).
5. `persisted = None, os = ["zh-CN", "ja-JP", "tr-TR"]` → `Tr` (first
   matching supported prefix wins).
6. **`persisted = Some(En), os = ["de-DE"]` → `En`** (critical Fix A case:
   explicit English MUST NOT be overridden by a German OS locale — the
   `Some(_)` discriminant is the authoritative signal, not value-equality).
7. `persisted = None, os = ["de-CH"]` → `De` (Swiss German maps to `De` —
   two-letter prefix `de` matches).
8. `persisted = None, os = ["zh-CN"]` → `En` (unsupported prefix falls back —
   matches Fix H's extended test matrix).

### Locale-changed signal

The `I18nContext`'s locale is a Leptos `RwSignal<Locale>` (more
precisely, a typed wrapper around one — the API surface
`get_locale()` / `set_locale(...)` is the library's stable signature
and is signal-based under the hood). Every `t!(...)` call site
re-derives from this signal; Leptos's reactive batching produces
a single re-render tick on a locale change (Spec Edge Case
"mixed-locale frame avoidance" honoured by the framework, no
bespoke "translation barrier" required per Spec Edge Case bullet 1
/ FR-012 / SC-007).

### Library cargo features used by presto

| Feature | Status | Reason |
|---|---|---|
| `csr` | ON | Presto is CSR-only (Tauri WebView). |
| `cookie` | OFF | Default feature; explicitly disabled — presto persists locale via `settings.appearance.locale`, not the library's `lf-lang` cookie. |
| `json_files` | ON (default) | JSON catalogue format per Decision 2 in `research.md`. |
| `icu_compiled_data` | ON (default) | Pulled by default; needed if `plurals` is enabled in a follow-up. |
| `plurals` | OFF in v1 | Spec Edge Case bullet 5 accepts plural-rewriting; enable in tasks-phase only if a key needs it. |
| `yaml` | OFF | JSON is the chosen format. |
| `json5` | OFF | JSON is the chosen format. |
| `tracking` | OFF | Increases `Cargo.toml` watch surface in dev; not needed. |
| `nightly` | OFF | Presto uses stable Rust. |

Final declaration in `src/Cargo.toml`:

```toml
leptos_i18n = { version = "=0.5.11", default-features = false, features = ["csr", "json_files", "icu_compiled_data"] }
```

Version is exact-pinned (`=0.5.11`, not `"0.5"`) because only `0.5.11`
is verified against `leptos = "0.7"`. Upgrade only when a new `0.5.x`
release is explicitly retested. See Fix C rationale in plan.md Phase 0.

(Disabling default features drops the `cookie` feature to keep the
WASM bundle lean and avoid the library's automatic cookie write —
presto handles persistence through the existing settings flow.)

## 4. Translation-completeness invariant — FR contract

For every key in `en.json`, the same key MUST exist in `de.json`,
`it.json`, and `tr.json`. Symmetrically, every key in any of the
three non-English catalogues MUST also exist in `en.json` (no stale
translations left over after an English-side key removal — Spec
A13).

### Enforcement path

`leptos_i18n`'s proc-macro emits `MissingKey` and `SurplusKey`
warnings during macro expansion (see
`leptos_i18n_parser/src/parse_locales/warning.rs`):

```rust
// Excerpted from leptos_i18n_parser:
pub enum Warning {
    MissingKey { locale: Key, key_path: KeyPath },
    SurplusKey { locale: Key, key_path: KeyPath },
    // ...
}
```

These warnings are surfaced as `#[deprecated(note = "Missing key
\"...\" in locale ...")]` annotations on a generated `warnings()` fn
that the macro output calls (see
`leptos_i18n_macro/src/load_locales/warning.rs`). The presto
workspace runs:

```bash
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic
```

(per `.agentex.yml` `lint:` block). `-D warnings` covers the
`deprecated` lint that the macro-generated `warnings()` fn call
triggers — so a missing key OR a surplus key fails CI as a hard
error, no additional CI script required.

### Wire-shape constraint

The translation-completeness invariant is a build-time contract,
not a wire-shape contract. The on-disk catalogue files are
private to the presto crate and never cross the IPC boundary —
the IPC `Locale` enum carries only the locale-code identifier,
never the catalogue contents.

### Per-locale catalogue file declaration

`src/Cargo.toml`'s `[package.metadata.leptos-i18n]` block declares
the locale list (`["en", "de", "it", "tr"]`) and the default
(`"en"`). The proc-macro reads the four corresponding JSON files at
expansion time. Adding a fifth locale in a follow-up requires:

1. Adding the variant to `presto_ipc::Locale`.
2. Adding the variant to the dropdown's native-self-name list.
3. Adding the locale code to `[package.metadata.leptos-i18n]
   locales`.
4. Creating `src/locales/<new>.json` with the complete key set.

All four steps are localised changes (no engine touch, no migration).

### Pluralization contract (deferred)

If a future tasks-phase decision enables the `plurals` cargo
feature for `leptos_i18n`, the catalogue's plural-rule syntax
becomes ICU CLDR via the `icu_plurals` crate's data tables. The
contract for those keys would shift from
`{"session_count": "You completed {{ count }} sessions"}` to a
plural-form-aware shape per the library's plural-form
documentation. Plan-phase scope keeps `plurals` OFF and rewrites
count-sensitive English to avoid it (Spec Edge Case bullet 5).

### Non-Tauri / non-IPC scope

The `leptos_i18n` library and the `src/src/i18n.rs` module are
entirely UI-side. They:

- Do NOT define any `#[tauri::command]` handler.
- Do NOT call `tauri::invoke(...)` or `tauri::event::listen(...)`.
- Do NOT add anything to `tests/e2e/fixtures/tauriMock.js`.
- Do NOT import anything from `src/src/engine/`.
- DO import `leptos_i18n`'s macros and types directly via the
  library re-exports.
- DO read from the shared `RwSignal<Settings>` to bind the
  library's locale signal to the persisted `appearance.locale`
  field (two-way: settings change → `i18n.set_locale(...)`;
  dropdown `on:change` → settings change).

### Mock-drift gate impact

The mock-drift gate (`scripts/check-mock-drift.sh`) sees zero new
`#[tauri::command]` handlers and zero new mock entries — verified
by spot-check against `tests/e2e/fixtures/tauriMock.js`. The gate
stays green by inaction.
