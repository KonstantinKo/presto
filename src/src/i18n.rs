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
}
