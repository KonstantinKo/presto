// `NavigationManager` — the Rust port of `src/managers/navigation-manager.js`.
//
// Spec 001-leptos-migration §Phase 3a (T157-T160). Owns the active
// view (`NavView`) and the active settings sub-tab (`SettingsTab`).
// Router-style state machine: any `NavView::X → NavView::Y`
// transition is allowed (data-model.md §`NavView`). Initial state is
// `NavView::Timer`.

#[cfg(test)]
mod tests {
    use super::{NavView, NavigationManager, SettingsTab};

    /// T157 [RED]: initial state per data-model.md §`NavView`.
    /// Mirrors the JS-side `NavigationManager` constructor at
    /// `src/managers/navigation-manager.js:7` (`this.currentView =
    /// "timer"`).
    ///
    /// Done-signal: this test currently fails because `NavView`,
    /// `SettingsTab`, and `NavigationManager` do not yet exist.
    /// T158 GREEN lands them.
    #[test]
    fn initial_view_is_timer() {
        let nav = NavigationManager::new();
        assert_eq!(nav.current(), NavView::Timer);
    }

    /// T157 [RED]: any-to-any transition rule per data-model.md
    /// §`NavView` ("any `NavView::X → NavView::Y` is allowed").
    /// Iterates every (from, to) pair across the seven top-level
    /// views and asserts each transition lands.
    #[test]
    fn any_view_to_any_view_transition_allowed() {
        let views = [
            NavView::Timer,
            NavView::Tasks,
            NavView::History,
            NavView::Calendar,
            NavView::Tags,
            NavView::Team,
            NavView::Settings(SettingsTab::General),
        ];
        for from in views {
            for to in views {
                let mut nav = NavigationManager::new();
                nav.transition_to(from);
                assert_eq!(nav.current(), from, "first transition to {from:?}");
                nav.transition_to(to);
                assert_eq!(nav.current(), to, "second transition {from:?} -> {to:?}");
            }
        }
    }
}
