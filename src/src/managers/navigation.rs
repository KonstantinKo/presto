// `NavigationManager` — the Rust port of `src/managers/navigation-manager.js`.
//
// Spec 001-leptos-migration §Phase 3a (T157-T160). Owns the active
// view (`NavView`) and the active settings sub-tab (`SettingsTab`).
// Router-style state machine: any `NavView::X → NavView::Y`
// transition is allowed (data-model.md §`NavView`). Initial state is
// `NavView::Timer`.

/// Top-level views in the application sidebar.
///
/// Mirrors the `data-view` attribute values read at
/// `src/managers/navigation-manager.js:51`. The `Settings` variant
/// nests a `SettingsTab` so the manager can preserve the active
/// settings sub-tab across navigations to and from `Settings`
/// (data-model.md §`NavView`).
///
/// Spec 001-leptos-migration §Phase 3a T158.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavView {
    /// Initial state per data-model.md §`NavView`. The default lives
    /// here (rather than in a hand-written `impl Default`) to satisfy
    /// `clippy::derivable_impls`.
    #[default]
    Timer,
    Tasks,
    History,
    Calendar,
    Tags,
    Team,
    Settings(SettingsTab),
}

/// Settings page sub-tabs. Mirrors data-model.md §`SettingsTab`.
/// Order matches the JS-side settings navigation handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    /// Default landing tab — matches the JS-side initial state when
    /// the user opens settings without specifying a sub-tab.
    #[default]
    General,
    Shortcuts,
    Notifications,
    Automation,
    Advanced,
    Goals,
    Theme,
    Updates,
}

/// Active-view state machine.
///
/// Owns the current `NavView` and exposes a `transition_to(view)`
/// method that lands the new view unconditionally (router-style; no
/// gating). The settings-tab preservation logic lives in T160.
#[derive(Debug, Clone, Default)]
pub struct NavigationManager {
    /// Current top-level view. Initial state is `NavView::Timer`
    /// per data-model.md §`NavView`.
    current: NavView,
}

impl NavigationManager {
    /// Construct a fresh manager rooted at `NavView::Timer`. Mirrors
    /// `src/managers/navigation-manager.js:7` (`this.currentView =
    /// "timer"`).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            current: NavView::Timer,
        }
    }

    /// Borrow the active view.
    #[must_use]
    pub const fn current(&self) -> NavView {
        self.current
    }

    /// Land `view` as the active view. No gating — every transition
    /// is allowed (data-model.md §`NavView`). Mirrors the JS-side
    /// `switchView` body at
    /// `src/managers/navigation-manager.js:56-106` minus the DOM
    /// effects (those live in the Phase 4 components layer).
    pub const fn transition_to(&mut self, view: NavView) {
        self.current = view;
    }
}

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
