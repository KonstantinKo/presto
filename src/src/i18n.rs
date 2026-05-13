// Feature 005: locale resolution and library wiring.
//
// Hosts `leptos_i18n::load_locales!()` (Phase 3 wiring) and the
// pure-function locale resolver that powers the cold-start chain
// per FR-009 / FR-011 / Fix A. The resolver is pure so unit tests
// under `cargo test --workspace --frozen` can cover every branch of
// the precedence chain without a DOM (FR-023).

#[cfg(test)]
mod tests {
    use super::resolve_initial_locale;
    use presto_ipc::Locale;

    /// T008 [RED → T015 GREEN]: any `Some(_)` persisted locale wins
    /// over OS detection. Spec FR-009 step 1.
    #[test]
    fn resolve_initial_locale_persisted_some_wins() {
        assert_eq!(
            resolve_initial_locale(Some(Locale::De), ["en-US"]),
            Locale::De,
        );
    }

    /// T009 [RED → T015 GREEN]: **Fix A critical case** — explicit
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

    /// T010 [RED → T015 GREEN]: `None` (no explicit choice) falls
    /// through to OS detection; `de-DE` matches `Locale::De` via the
    /// two-letter prefix. Spec FR-009 step 2.
    #[test]
    fn resolve_initial_locale_none_falls_to_os_de() {
        assert_eq!(resolve_initial_locale(None, ["de-DE"]), Locale::De);
    }

    /// T011 [RED → T015 GREEN]: Swiss German `de-CH` matches
    /// `Locale::De` via the two-letter prefix. The prefix splitter
    /// must not require a country tag and must lowercase both halves
    /// of the BCP-47 tag before matching.
    #[test]
    fn resolve_initial_locale_none_swiss_german_matches_de() {
        assert_eq!(resolve_initial_locale(None, ["de-CH"]), Locale::De);
    }

    /// T012 [RED → T015 GREEN]: an unsupported OS-locale prefix
    /// (Chinese here) falls back to `Locale::En`. Spec FR-009 step 3.
    #[test]
    fn resolve_initial_locale_none_unsupported_falls_back_to_en() {
        assert_eq!(resolve_initial_locale(None, ["zh-CN"]), Locale::En);
    }
}
