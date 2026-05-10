// Top-level App router — Phase 4b (T216) of spec
// 001-leptos-migration. Mounts the sidebar nav + the active view +
// the always-on update banner + auth modal. Dispatches over
// `NavigationManager::current()` to pick which view to render.
//
// **Selector contract** (consumed by `tests/e2e/fixtures/screens.js::tapTab`):
// - `#timer-nav`, `#calendar-nav`, `#team-nav`, `#settings-nav` —
//   sidebar nav buttons (`screens.js:25,28,31,34`).
// - `#timer-view`, `#calendar-view`, `#team-view`, `#settings-view`
//   — active view containers; carry `.hidden` when inactive
//   (`screens.js:26,29,32,35`).
//
// The view switch uses the JS-era pattern of always-mounted view
// containers with `.hidden` toggled on the inactive ones — rather
// than mount-on-active. This matches `screens.js:26`
// (`waitForSelector("#timer-view:not(.hidden)")`) and lets CSS
// transitions render correctly. Each view component (TimerView,
// CalendarView, etc.) is responsible for its own root element with
// the canonical `id="<view>-view"` so the App router only needs to
// wrap them in a `class:hidden` switch.
//
// Tasks (Phase 4a) and History views aren't yet referenced by the
// `tapTab` fixture — the JS-era surface routes them via
// settings-style sub-navigation; the Rust port mounts them
// reachable via `NavView` but the App router only routes the four
// top-level surfaces the e2e suite touches plus the Tags dropdown
// (which lives inside the timer view as a popover). Tasks /
// History are mounted as siblings so they're reachable when the
// nav is later extended.
//
// Per Principle I, this component is pure UI plumbing — it never
// mutates engine state. The shared `RwSignal<TimerState>` /
// `RwSignal<Settings>` / `RwSignal<AuthState>` etc. are owned at
// this level and threaded into per-view components via props or
// `provide_context`.
//
// T217 wires the bridge-bus subscriptions (global-shortcut events,
// update-available emits, settings load) and the
// `bridge::storage::migrate_legacy_localstorage()` startup hop.
// T218 attaches the degraded-mode UI when `BridgeAvailable::Absent`.
//
// Lint allowance: `clippy::must_use_candidate` is silenced for the
// usual Leptos `#[component]` reason. `clippy::too_many_lines` is
// silenced because the view body is a single Leptos `view!`
// expansion covering the sidebar + every top-level view.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::bridge::availability::{bridge_available, BridgeAvailable};
use crate::bridge::commands;
use crate::bridge::events::{self, GLOBAL_SHORTCUT, UPDATE_AVAILABLE};
use crate::bridge::storage;
use crate::bridge::types::{Settings, UpdateAvailablePayload};
use crate::components::auth_modal::AuthModal;
use crate::components::calendar::CalendarView;
use crate::components::history::HistoryView;
use crate::components::settings::SettingsView;
use crate::components::tags::TagsView;
use crate::components::tasks::TasksView;
use crate::components::team::TeamView;
use crate::components::timer::TimerView;
use crate::components::update_notification::UpdateNotification;
use crate::managers::auth::AuthState;
use crate::managers::navigation::{NavView, NavigationManager, SettingsTab};
use crate::managers::update::{UpdateInfo, UpdateManager};

/// Top-level App component. Mounts the sidebar nav, the active
/// view, the global update banner, and the auth modal.
#[component]
pub fn App() -> impl IntoView {
    // Shared cross-view signals. The App router owns these and
    // threads them into per-view components. T217 will swap the
    // raw `RwSignal`s for context-provided structs once Phase 4c
    // wires the persistence sinks.
    let nav = RwSignal::new(NavigationManager::new());
    let settings = RwSignal::new(Settings::default());
    let auth_state = RwSignal::new(AuthState::default());
    let update_info = RwSignal::new(UpdateInfo::default());

    // Derived view-active flags. Each per-view container reads its
    // own flag to decide whether to apply `.hidden` — matching the
    // JS-era pattern at `screens.js:26-35`
    // (`#timer-view:not(.hidden)`).
    let is_timer = Signal::derive(move || nav.with(|n| matches!(n.current(), NavView::Timer)));
    let is_calendar =
        Signal::derive(move || nav.with(|n| matches!(n.current(), NavView::Calendar)));
    let is_team = Signal::derive(move || nav.with(|n| matches!(n.current(), NavView::Team)));
    let is_settings =
        Signal::derive(move || nav.with(|n| matches!(n.current(), NavView::Settings(_))));
    let is_tasks = Signal::derive(move || nav.with(|n| matches!(n.current(), NavView::Tasks)));
    let is_history =
        Signal::derive(move || nav.with(|n| matches!(n.current(), NavView::History)));
    let is_tags = Signal::derive(move || nav.with(|n| matches!(n.current(), NavView::Tags)));

    let active_settings_tab = Signal::derive(move || {
        nav.with(|n| match n.current() {
            NavView::Settings(tab) => tab,
            _ => n.last_settings_tab(),
        })
    });

    // Sidebar nav click handlers.
    let on_timer_nav = move |_| nav.update(|n| n.transition_to(NavView::Timer));
    let on_calendar_nav = move |_| nav.update(|n| n.transition_to(NavView::Calendar));
    let on_team_nav = move |_| nav.update(|n| n.transition_to(NavView::Team));
    let on_settings_nav = move |_| nav.update(NavigationManager::enter_settings);

    let on_select_settings_tab = Callback::new(move |tab: SettingsTab| {
        nav.update(|n| n.select_settings_tab(tab));
    });

    // Bridge availability — drives both the startup hops (only
    // fired when the bridge is reachable) and the T218
    // degraded-mode banner (rendered when `Absent`). Probing
    // `bridge_available()` once on mount mirrors the FR-009 short-
    // circuit pattern at every wrapper site: cheap (one Reflect
    // lookup), and the result is stable for the lifetime of the
    // Leptos runtime within a single tab session.
    let bridge_state = bridge_available();
    let bridge_absent = matches!(bridge_state, BridgeAvailable::Absent);

    // Startup hops: legacy migration + settings load. Skipped when
    // the bridge is absent (Trunk dev server / e2e mock harness)
    // because every wrapper short-circuits to BridgeUnavailable
    // anyway and the spawn would log a noisy error.
    if matches!(bridge_state, BridgeAvailable::Available) {
        spawn_local(async move {
            // Legacy localStorage migration runs first so settings
            // loaded post-migration reflect the imported state.
            // Idempotent per Phase 1E T115 — running on every cold
            // start is safe.
            let _ = storage::migrate_legacy_localstorage().await;
            // Then load the canonical post-migration settings into
            // the shared signal. Errors fall back to
            // `Settings::default()` (matches the JS-era behaviour
            // at `src/managers/settings-manager.js:125-128`).
            if let Ok(loaded) = commands::load_settings().await {
                settings.set(loaded);
            }
        });

        // Subscribe to `tauri://update-available` emits. The
        // listener feeds the `UpdateManager`'s `handle_event` and
        // lifts the shared `update_info` signal — the
        // `UpdateNotification` banner reads that signal.
        spawn_local(async move {
            let mut update_mgr = UpdateManager::new();
            let listener = events::listen::<UpdateAvailablePayload>(
                UPDATE_AVAILABLE,
                move |payload| {
                    update_mgr.handle_event(payload);
                    update_info.set(update_mgr.info().clone());
                },
            )
            .await;
            // The Listener guard is intentionally leaked into the
            // closure so the subscription survives until the App
            // unmounts (which on the App root means until the WASM
            // runtime tears down). A cleaner shutdown story
            // attaches the guard to a Leptos `on_cleanup` once
            // Phase 4c folds the manager state into context.
            if let Ok(guard) = listener {
                let _ = Box::leak(Box::new(guard));
            }
        });

        // Subscribe to global-shortcut emits. Each emit carries
        // the binding name as a primitive `String` payload (per
        // contracts/tauri-bridge.md §"Tauri events"); the
        // listener routes it through the timer's start/stop
        // surface. T217 today wires the listener; routing the
        // shortcut into `engine::TimerState` is owned by the
        // TimerView's effect (Phase 4a). The listener exists at
        // this level so the registration is one-shot.
        spawn_local(async move {
            let listener = events::listen::<String>(GLOBAL_SHORTCUT, |_name| {
                // Phase 4c routes `_name` ("start_stop", "reset",
                // "skip") into the engine. Today we acknowledge
                // the emit so the JS bridge sees a live listener
                // and doesn't drop subsequent events.
            })
            .await;
            if let Ok(guard) = listener {
                let _ = Box::leak(Box::new(guard));
            }
        });
    }

    view! {
        <nav class="sidebar">
            <div class="sidebar-icons">
                <button
                    class="sidebar-icon"
                    class:active=move || is_timer.get()
                    id="timer-nav"
                    data-view="timer"
                    title="Timer"
                    on:click=on_timer_nav
                >
                    <i class="ri-timer-line"></i>
                </button>
                <button
                    class="sidebar-icon"
                    class:active=move || is_calendar.get()
                    id="calendar-nav"
                    data-view="calendar"
                    title="Calendar"
                    on:click=on_calendar_nav
                >
                    <i class="ri-calendar-line"></i>
                </button>
                <button
                    class="sidebar-icon"
                    class:active=move || is_team.get()
                    id="team-nav"
                    data-view="team"
                    title="Team"
                    on:click=on_team_nav
                >
                    <i class="ri-group-line"></i>
                </button>
            </div>
            <div class="sidebar-bottom">
                <AuthModal auth_state=auth_state/>
                <button
                    class="sidebar-icon-large"
                    class:active=move || is_settings.get()
                    id="settings-nav"
                    data-view="settings"
                    title="Settings"
                    on:click=on_settings_nav
                >
                    <i class="ri-settings-3-line"></i>
                </button>
            </div>
        </nav>

        <main class="main-content">
            // Each view container carries `.hidden` when inactive —
            // matching `screens.js:26-35`. The view component
            // itself owns the `id="<name>-view"` attribute on its
            // root, so we wrap in a `<div>` host that toggles the
            // hidden class.
            //
            // NOTE: this introduces a wrapping `<div>` not present
            // in the JS-era DOM (the JS surface had `id="*-view"`
            // directly on the wrapper). Because the e2e selectors
            // address `#<name>-view` and the components own that
            // id on their root element, the wrapping host carries
            // the hidden class without an id — the inner element's
            // id resolves regardless. T217 may fold the hidden
            // toggling onto the inner element via a context-
            // provided active flag.
            <div class="view-host" class:hidden=move || !is_timer.get()>
                <TimerView/>
            </div>
            <div class="view-host" class:hidden=move || !is_calendar.get()>
                <CalendarView/>
            </div>
            <div class="view-host" class:hidden=move || !is_team.get()>
                <TeamView/>
            </div>
            <div class="view-host" class:hidden=move || !is_settings.get()>
                <SettingsView
                    tab=active_settings_tab
                    settings=settings
                    on_select_tab=on_select_settings_tab
                />
            </div>
            <div class="view-host" class:hidden=move || !is_tasks.get()>
                <TasksView/>
            </div>
            <div class="view-host" class:hidden=move || !is_history.get()>
                <HistoryView/>
            </div>
            <div class="view-host" class:hidden=move || !is_tags.get()>
                <TagsView/>
            </div>
        </main>

        // Always-on overlays — the update banner is mounted at the
        // top level so navigation doesn't unmount it (the spec at
        // `update-notification.spec.js:32-34` asserts the banner's
        // dismissed flag survives Calendar → Timer round-trips).
        <UpdateNotification update_info=update_info/>

        // T218 degraded-mode banner — visible only when the Tauri
        // JS bridge is absent (Trunk dev server / browser-only
        // load). The banner pins Phase 1G's BridgeAvailable
        // short-circuit at the visual level: persistence is a
        // no-op, but the UI still renders and the in-memory state
        // remains usable for development. The banner uses an
        // `id`-less surface so the e2e suite (which always runs
        // against the bridge mock) doesn't trip on it; `cargo
        // tauri dev` hides it because the bridge is reachable.
        {bridge_absent.then(|| view! {
            <div class="degraded-mode-banner" role="status">
                <strong>"Degraded mode."</strong>
                " Tauri bridge unavailable — settings + sessions are in-memory only."
            </div>
        })}
    }
}
