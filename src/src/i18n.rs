// Feature 005: locale resolution and library wiring.
//
// Hosts `leptos_i18n::load_locales!()` (the proc-macro reads the four
// `src/locales/<locale>.json` catalogues at compile time and embeds
// them into the WASM bundle), the pure-function locale resolver that
// powers the cold-start chain per FR-009 / FR-011 / Fix A, the
// `From<presto_ipc::Locale>` <-> `i18n::Locale` impls for translating
// between the IPC-side wire enum and the library-generated enum, and
// the `compute_initial_library_locale` boot-time helper.
//
// The resolver is pure so unit tests under `cargo test --workspace
// --frozen` cover every branch of the precedence chain without a DOM
// (FR-023). The `web_sys::window().navigator().languages()` read
// happens in `src/src/app.rs`'s boot path via the library's built-in
// `leptos-use::use_locales` integration — not in this module.

use presto_ipc::Locale;

leptos_i18n::load_locales!();

/// Resolve the cold-start locale per FR-009's strict precedence chain.
///
/// 1. If `persisted` is `Some(_)` (any variant, including `Some(En)`),
///    return it verbatim — the user has explicitly chosen a locale;
///    skip OS detection. This is the Fix A invariant: explicit English
///    MUST NOT trigger OS re-detection on a German OS.
/// 2. Otherwise iterate `os_langs`; the first BCP-47 tag whose
///    two-letter prefix matches one of the four supported locales wins.
/// 3. Fall back to `Locale::En` when no OS-language matches.
///
/// Pure function: takes the persisted value and the OS-detected
/// language strings as parameters. The actual `navigator.languages`
/// read happens at the caller in `src/src/app.rs`.
pub fn resolve_initial_locale(
    persisted: Option<Locale>,
    os_langs: impl IntoIterator<Item = impl AsRef<str>>,
) -> Locale {
    if let Some(locale) = persisted {
        return locale;
    }
    for lang in os_langs {
        if let Some(matched) = match_two_letter_prefix(lang.as_ref()) {
            return matched;
        }
    }
    Locale::En
}

/// Match a single BCP-47 language tag against the four supported
/// locales by its two-letter prefix, lowercased.
///
/// `"de-DE"` -> `Some(De)`. `"de-CH"` -> `Some(De)`. `"DE"` ->
/// `Some(De)`. `"de_AT"` -> `Some(De)` (underscore separator handled
/// alongside hyphen for resilience against locale-string variants
/// across OS-detection paths). Unsupported prefixes return `None`.
fn match_two_letter_prefix(lang: &str) -> Option<Locale> {
    let prefix = lang.split(['-', '_']).next()?;
    match prefix.to_ascii_lowercase().as_str() {
        "en" => Some(Locale::En),
        "de" => Some(Locale::De),
        "it" => Some(Locale::It),
        "tr" => Some(Locale::Tr),
        _ => None,
    }
}

impl From<Locale> for i18n::Locale {
    fn from(l: Locale) -> Self {
        match l {
            Locale::En => Self::en,
            Locale::De => Self::de,
            Locale::It => Self::it,
            Locale::Tr => Self::tr,
        }
    }
}

impl From<i18n::Locale> for Locale {
    fn from(l: i18n::Locale) -> Self {
        match l {
            i18n::Locale::en => Self::En,
            i18n::Locale::de => Self::De,
            i18n::Locale::it => Self::It,
            i18n::Locale::tr => Self::Tr,
        }
    }
}

/// Boot-time helper: project the IPC-persisted locale into the
/// library's locale-provider input.
///
/// Returns `None` when the user has never explicitly chosen a locale
/// (legacy records or fresh install) so the library's
/// `<I18nContextProvider>` triggers its built-in OS-detection path
/// via `leptos-use::use_locales` (FR-011 / Fix A). Returns `Some(_)`
/// when the persisted value carries an explicit choice — including
/// `Some(En)` (Fix A critical case).
#[must_use]
pub fn compute_initial_library_locale(persisted: Option<Locale>) -> Option<i18n::Locale> {
    persisted.map(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{compute_initial_library_locale, i18n as lib_locale, resolve_initial_locale};
    use presto_ipc::Locale;

    /// T008 [RED -> T015 GREEN]: any `Some(_)` persisted locale wins
    /// over OS detection. Spec FR-009 step 1.
    #[test]
    fn resolve_initial_locale_persisted_some_wins() {
        assert_eq!(
            resolve_initial_locale(Some(Locale::De), ["en-US"]),
            Locale::De,
        );
    }

    /// T009 [RED -> T015 GREEN]: **Fix A critical case** — explicit
    /// English MUST NOT be overridden by a German OS locale. The
    /// `Some(_)` discriminant is the authoritative signal, not
    /// value-equality against `Locale::En`. A German-OS user who
    /// explicitly picks English persists `Some(En)`; the resolver
    /// sees `Some(_)` and skips OS detection.
    #[test]
    fn resolve_initial_locale_persisted_some_en_wins() {
        assert_eq!(
            resolve_initial_locale(Some(Locale::En), ["de-DE"]),
            Locale::En,
        );
    }

    /// T010 [RED -> T015 GREEN]: `None` (no explicit choice) falls
    /// through to OS detection; `de-DE` matches `Locale::De` via the
    /// two-letter prefix. Spec FR-009 step 2.
    #[test]
    fn resolve_initial_locale_none_falls_to_os_de() {
        assert_eq!(resolve_initial_locale(None, ["de-DE"]), Locale::De);
    }

    /// T011 [RED -> T015 GREEN]: Swiss German `de-CH` matches
    /// `Locale::De` via the two-letter prefix. The prefix splitter
    /// must not require a country tag and must lowercase both halves
    /// of the BCP-47 tag before matching.
    #[test]
    fn resolve_initial_locale_none_swiss_german_matches_de() {
        assert_eq!(resolve_initial_locale(None, ["de-CH"]), Locale::De);
    }

    /// T012 [RED -> T015 GREEN]: an unsupported OS-locale prefix
    /// (Chinese here) falls back to `Locale::En`. Spec FR-009 step 3.
    #[test]
    fn resolve_initial_locale_none_unsupported_falls_back_to_en() {
        assert_eq!(resolve_initial_locale(None, ["zh-CN"]), Locale::En);
    }

    /// T013 [RED -> T015 GREEN]: `None` + empty OS list (simulates
    /// `navigator.languages` unavailable or returning empty) falls
    /// back to `Locale::En`. No panic, no error. Spec FR-010 / Story
    /// 4 AC 5.
    #[test]
    fn resolve_initial_locale_none_empty_os_falls_back_to_en() {
        let empty: [&str; 0] = [];
        assert_eq!(resolve_initial_locale(None, empty), Locale::En);
    }

    /// T014 [RED -> T015 GREEN]: first matching supported prefix wins
    /// when the OS lists multiple preferred languages. `zh-CN`
    /// (unsupported) is skipped; the next, `de-DE`, matches —
    /// `tr-TR` (also supported) does NOT override because priority
    /// order is OS-given. Spec FR-009 priority-first semantics.
    #[test]
    fn resolve_initial_locale_none_first_match_wins() {
        assert_eq!(
            resolve_initial_locale(None, ["zh-CN", "de-DE", "tr-TR"]),
            Locale::De,
        );
    }

    /// `compute_initial_library_locale(None)` returns `None` so the
    /// library's built-in OS-detection path runs on cold start; any
    /// `Some(_)` (including `Some(En)`) projects to the matching
    /// library-side variant verbatim.
    #[test]
    fn compute_initial_library_locale_projects_explicit_choice() {
        assert!(compute_initial_library_locale(None).is_none());
        assert_eq!(
            compute_initial_library_locale(Some(Locale::De)),
            Some(lib_locale::Locale::de),
        );
        assert_eq!(
            compute_initial_library_locale(Some(Locale::En)),
            Some(lib_locale::Locale::en),
        );
    }
}
