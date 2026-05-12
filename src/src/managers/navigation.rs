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
/// Owns the current `NavView` and the last-selected
/// `SettingsTab`. `transition_to(view)` lands `view`
/// unconditionally (router-style; no gating).
/// `enter_settings()` lands `Settings(last_settings_tab)`,
/// preserving the user's settings-tab selection across
/// round-trips through other views (T160).
#[derive(Debug, Clone, Default)]
pub struct NavigationManager {
    /// Current top-level view. Initial state is `NavView::Timer`
    /// per data-model.md §`NavView`.
    current: NavView,
    /// Last-selected settings sub-tab. Initial state is
    /// `SettingsTab::General`. Updated by `select_settings_tab`;
    /// read by `enter_settings`. Mirrors the JS-side behaviour
    /// where the settings page restores its previous tab on
    /// re-entry rather than resetting to General.
    last_settings_tab: SettingsTab,
}

impl NavigationManager {
    /// Construct a fresh manager rooted at `NavView::Timer`. Mirrors
    /// `src/managers/navigation-manager.js:7` (`this.currentView =
    /// "timer"`).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            current: NavView::Timer,
            last_settings_tab: SettingsTab::General,
        }
    }

    /// Borrow the active view.
    #[must_use]
    pub const fn current(&self) -> NavView {
        self.current
    }

    /// Borrow the last-selected settings sub-tab. Useful for tests
    /// and for the components layer (Phase 4) to highlight the
    /// active tab without inspecting `current()`.
    #[must_use]
    pub const fn last_settings_tab(&self) -> SettingsTab {
        self.last_settings_tab
    }

    /// Land `view` as the active view. No gating — every transition
    /// is allowed (data-model.md §`NavView`). Mirrors the JS-side
    /// `switchView` body at
    /// `src/managers/navigation-manager.js:56-106` minus the DOM
    /// effects (those live in the Phase 4 components layer).
    ///
    /// When the supplied view is a `Settings(tab)`, the
    /// `last_settings_tab` slice is updated so the next
    /// `enter_settings()` call (which doesn't specify a tab)
    /// restores the same one.
    pub const fn transition_to(&mut self, view: NavView) {
        if let NavView::Settings(tab) = view {
            self.last_settings_tab = tab;
        }
        self.current = view;
    }

    /// Land `Settings(last_settings_tab)`. Convenience for the
    /// sidebar's "Settings" button which doesn't specify a sub-tab;
    /// the manager picks up where the user last left off (T160).
    pub const fn enter_settings(&mut self) {
        self.current = NavView::Settings(self.last_settings_tab);
    }

    /// Land `Settings(tab)` and record `tab` as the new
    /// last-selected sub-tab. Convenience for the settings-page
    /// in-page tab strip; equivalent to
    /// `transition_to(NavView::Settings(tab))`.
    pub const fn select_settings_tab(&mut self, tab: SettingsTab) {
        self.last_settings_tab = tab;
        self.current = NavView::Settings(tab);
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

    /// T159 [RED]: settings-tab nested transition preservation.
    /// When the user navigates Settings(Theme) → Tasks → Settings,
    /// the second `Settings` landing must restore the previously
    /// active sub-tab (Theme), NOT reset to `General`. Mirrors the
    /// JS-side behaviour where `SettingsManager.populateSettingsUI`
    /// reads the last-selected settings tab from a manager-owned
    /// state slice rather than re-initialising on every Settings
    /// landing (see `setupSettingsNavigation` plumbing at
    /// `src/managers/settings-manager.js`).
    ///
    /// API: `enter_settings()` (no arg) lands `Settings(last_tab)`;
    /// `select_settings_tab(tab)` lands `Settings(tab)` AND records
    /// `tab` as the last-selected one for future `enter_settings()`
    /// calls.
    ///
    /// Done-signal: this test currently fails because
    /// `enter_settings` and `select_settings_tab` do not yet exist.
    /// T160 GREEN attaches them.
    #[test]
    fn settings_tab_transitions_preserve_selected_tab() {
        let mut nav = NavigationManager::new();

        // Pick a non-default tab.
        nav.select_settings_tab(SettingsTab::Theme);
        assert_eq!(nav.current(), NavView::Settings(SettingsTab::Theme));
        assert_eq!(nav.last_settings_tab(), SettingsTab::Theme);

        // Leave settings.
        nav.transition_to(NavView::Tasks);
        assert_eq!(nav.current(), NavView::Tasks);
        // Last-selected tab is preserved across the round-trip.
        assert_eq!(nav.last_settings_tab(), SettingsTab::Theme);

        // Re-enter settings without specifying a tab — must land
        // Settings(Theme), not Settings(General).
        nav.enter_settings();
        assert_eq!(
            nav.current(),
            NavView::Settings(SettingsTab::Theme),
            "re-entering settings must restore the last-selected tab",
        );

        // Switch tab and verify it lands.
        nav.select_settings_tab(SettingsTab::Shortcuts);
        assert_eq!(nav.current(), NavView::Settings(SettingsTab::Shortcuts));

        // Round-trip away and back, verifying Shortcuts is now the
        // remembered tab.
        nav.transition_to(NavView::Calendar);
        nav.enter_settings();
        assert_eq!(nav.current(), NavView::Settings(SettingsTab::Shortcuts));
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
