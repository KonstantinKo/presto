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
}
