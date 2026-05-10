// Top-level App router. Spec: 001-leptos-migration §Phase 4b.
// Mounts the sidebar nav + the active view + the always-on update
// banner + auth modal. Dispatches over
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
use crate::bridge::types::{Session, Settings, UpdateAvailablePayload};
use crate::components::auth_modal::AuthModal;
use crate::components::calendar::CalendarView;
use crate::components::settings::SettingsView;
use crate::components::tasks::TasksView;
use crate::components::team::TeamView;
use crate::components::timer::TimerView;
use crate::components::update_notification::UpdateNotification;
use crate::managers::auth::AuthState;
use crate::managers::navigation::{NavView, NavigationManager, SettingsTab};
use crate::managers::update::{UpdateInfo, UpdateManager};
use crate::theme::loader;

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

    // Shared session log. TimerView pushes a `ManualSession` on
    // engine completion (focus session zero-cross OR a `skip()`
    // mid-focus that was over the JS-era 1-minute floor).
    // CalendarView reads the same signal to render the
    // `#sessions-table-body` rows that
    // `sessions-history.spec.js:38-41` exercises.
    let sessions = RwSignal::new(Vec::<crate::bridge::types::ManualSession>::new());

    // Phase 4e R-004: shared tag list. Seeded with the JS-era default
    // "Focus" tag so TimerView (which uses_context this signal) renders
    // the tag row immediately without waiting for the cold-start
    // load_tags IPC to resolve. The load overwrites the signal with the
    // persisted list; the default seed is only visible for the ~10ms
    // before the IPC returns. Matches the tauriMock fixture's
    // default _state.tags seed at tauriMock.js.
    let tags = RwSignal::new(vec![crate::bridge::types::Tag {
        id: "default-focus".to_string(),
        name: "Focus".to_string(),
        icon: "ri-brain-line".to_string(),
        color: "#4CAF50".to_string(),
        created_at: String::new(),
    }]);

    // R-004: session_data signal — tracks accumulated pomodoro counter
    // state (completed_pomodoros, total_focus_time, current_session).
    // Cold-start hydrated below; TimerView updates it on each
    // PomodoroCompleted event via save_session_data.
    let session_data = RwSignal::new(Option::<Session>::None);

    // Make `RwSignal<Settings>` available to descendants via context.
    // TimerView reads it to derive the engine's `Durations` from the
    // settings.timer fields (so `settings-general.spec.js` and the
    // debug-mode flow in `settings-advanced.spec.js` pick up the
    // edited values without going through a SettingsManager hop).
    // SettingsView still receives the same signal as a prop because
    // its sub-tabs were wired before context was available; both
    // surfaces refer to the same RwSignal so updates from either
    // path propagate to TimerView.
    provide_context(settings);
    provide_context(sessions);
    provide_context(tags);
    provide_context(session_data);

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
    // `NavView::History` and `NavView::Tags` are reachable through
    // the navigation manager but the App router does not mount their
    // standalone views (see the route block below for the rationale
    // — both surfaces are duplicates of selectors owned by other
    // views and would otherwise trip Playwright's strict-mode locator
    // resolution). The variants stay on `NavView` so navigation
    // history serialization stays stable.

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
            // R-006: sentinel gate — one IPC trip instead of 7 on
            // post-migration cold starts. If the sentinel file exists,
            // the migration has already run to completion on a prior
            // launch; skip the 7-call per-domain block entirely.
            // Errors (bridge absent, fs failure) fall through to the
            // migration block — safe because per-domain handlers are
            // idempotent (they skip when the target file already
            // exists). On the first post-cutover launch `false` is
            // returned (sentinel not yet written); after the migration
            // succeeds we write the sentinel so the next cold start
            // skips here.
            let migration_done = commands::is_legacy_migration_complete()
                .await
                .unwrap_or(false);
            if !migration_done {
                let migrated = storage::migrate_legacy_localstorage().await;
                if migrated.is_ok() {
                    // Best-effort write; if it fails the next cold start
                    // re-runs the migration (per-domain handlers are
                    // idempotent so re-running is safe).
                    let _ = commands::mark_legacy_migration_complete().await;
                }
            }
            // Then load the canonical post-migration settings into
            // the shared signal. Errors fall back to
            // `Settings::default()` (matches the JS-era behaviour
            // at `src/managers/settings-manager.js:125-128`).
            if let Ok(loaded) = commands::load_settings().await {
                // Phase 4e R-002: project the user-state slice
                // (`guest_mode`) into the auth signal AFTER the
                // migration ran (so a 0.4.x guest user's
                // `presto-guest-mode` localStorage flag has been
                // folded into `Settings.guest_mode` already). The
                // projection only fires when the in-memory
                // `auth_state` is still the cold-start
                // `Unauthenticated` shape — once the user signs in
                // (or hits Continue as Guest manually), the
                // post-load projection should not regress them.
                if loaded.guest_mode {
                    auth_state.update(|s| {
                        if matches!(s, AuthState::Unauthenticated) {
                            *s = AuthState::Guest;
                        }
                    });
                }
                let resolved = loader::resolve_color_mode(
                    &loaded.appearance.theme,
                    loader::system_prefers_dark(),
                );
                loader::apply_theme(resolved);
                loader::apply_timer_theme(&loaded.appearance.timer_theme);
                settings.set(loaded);
            }
            // Phase 4e R-003: cold-start session probe — read the
            // persisted Supabase session from disk; if present, lift
            // the auth signal into `SignedIn`. Mirrors the JS-era
            // `auth-manager.js`'s `getSession()` cold-start hop.
            // Only overrides the projection above when a real
            // session exists; the bridge-unavailable path is silently
            // absorbed (the dev server has no Tauri bridge, so
            // `Ok(None)` and `Err(BridgeUnavailable)` both reduce to
            // "no signed-in user" which leaves the auth signal at
            // its current value).
            if let Ok(Some(session)) = commands::supabase_get_session().await {
                auth_state.set(AuthState::SignedIn { user: session.user });
            }
        });

        // Phase 4e R-004: debounced settings persistence sink.
        //
        // The shared `RwSignal<Settings>` is the source of truth
        // for every Settings tab. This Effect re-runs whenever the
        // signal changes; rather than firing the bridge call on
        // every keystroke (a slider drag could pump >50 changes /
        // second), we schedule a `setTimeout` 300ms in the future
        // and reset the schedule on each subsequent change. Once
        // the user pauses for 300ms, the latest value lands on
        // disk via `commands::save_settings`.
        //
        // The first effect run captures the initial signal value
        // (right after `commands::load_settings` hydrates it). We
        // skip the FIRST save attempt so the load → save → load
        // cycle doesn't tail-chase itself; the cold-start guard
        // (`first_run`) flips after the first invocation.
        //
        // Mirrors the JS-era `scheduleAutoSave()` flow at
        // `src/managers/settings-manager.js:116`.
        let first_run = std::rc::Rc::new(std::cell::Cell::new(true));
        let pending_handle = std::rc::Rc::new(std::cell::Cell::new(
            None::<leptos::leptos_dom::helpers::TimeoutHandle>,
        ));
        Effect::new(move |_| {
            // Track the signal: every settings mutation re-runs
            // this closure.
            let snapshot = settings.get();
            // Skip the very first effect run — that fires on mount
            // before `load_settings` has even returned, so persisting
            // would write `Settings::default()` over the user's
            // on-disk state.
            if first_run.get() {
                first_run.set(false);
                return;
            }
            // Cancel any in-flight debounce window so the latest
            // edit wins. The `TimeoutHandle::clear()` is a no-op if
            // the timeout has already fired.
            if let Some(handle) = pending_handle.take() {
                handle.clear();
            }
            // Schedule the bridge save 300ms from now.
            let handle_clone = pending_handle.clone();
            let scheduled = leptos::leptos_dom::helpers::set_timeout_with_handle(
                move || {
                    let to_save = snapshot.clone();
                    handle_clone.set(None);
                    spawn_local(async move {
                        // Errors absorbed — the bridge-unavailable
                        // path on the dev server is expected; a real
                        // Tauri build only fails here for filesystem
                        // failures (which would surface in dev tools
                        // rather than UI). The next mutation fires
                        // another save.
                        let _ = commands::save_settings(to_save).await;
                    });
                },
                std::time::Duration::from_millis(300),
            );
            if let Ok(handle) = scheduled {
                pending_handle.set(Some(handle));
            }
        });

        // Phase 4f R-004: settings-driven side effects.
        //
        // These Effects track specific settings slices and fire
        // bridge calls when the relevant field changes. They are
        // separate from the debounced persistence sink above so
        // the OS-level side effects (shortcut registration, activity
        // monitoring, autostart, tray) don't wait 300ms.
        //
        // Effect ordering: all five Effects share the same
        // `RwSignal<Settings>` source. Leptos runs Effects in
        // declaration order within a single reactive update, so
        // the persistence sink fires first, then these side-effect
        // sinks. The ordering is documented but not relied upon —
        // each sink captures only its own settings slice via
        // `settings.with(...)` so there is no cross-Effect state
        // dependency.

        // shortcuts — fire when any shortcut binding changes.
        let shortcuts_first_run = std::rc::Rc::new(std::cell::Cell::new(true));
        Effect::new(move |_| {
            let shortcuts = settings.with(|s| s.shortcuts.clone());
            if shortcuts_first_run.get() {
                shortcuts_first_run.set(false);
                return;
            }
            spawn_local(async move {
                let _ = commands::register_global_shortcuts(shortcuts).await;
            });
        });

        // activity monitoring — fire when smart_pause or its timeout changes.
        // When smart_pause is on and only the timeout changes (not the toggle
        // itself), call update_activity_timeout to adjust the threshold without
        // a stop/start cycle. When the toggle flips, start or stop monitoring.
        let smart_pause_first_run = std::rc::Rc::new(std::cell::Cell::new(true));
        // Track previous smart_pause state to distinguish toggle from
        // timeout-only changes across re-runs.
        let prev_smart_pause = std::rc::Rc::new(std::cell::Cell::new(false));
        Effect::new(move |_| {
            let smart_pause = settings.with(|s| s.notifications.smart_pause);
            let timeout_secs =
                settings.with(|s| u64::from(s.notifications.smart_pause_timeout) * 60);
            if smart_pause_first_run.get() {
                smart_pause_first_run.set(false);
                prev_smart_pause.set(smart_pause);
                return;
            }
            let was_smart_pause = prev_smart_pause.get();
            prev_smart_pause.set(smart_pause);
            spawn_local(async move {
                if smart_pause {
                    if was_smart_pause {
                        // Already on, timeout changed: update threshold in-place.
                        let _ = commands::update_activity_timeout(timeout_secs).await;
                    } else {
                        // Toggle on: start monitoring with the current timeout.
                        let _ = commands::start_activity_monitoring(timeout_secs).await;
                    }
                } else {
                    let _ = commands::stop_activity_monitoring().await;
                }
            });
        });

        // autostart — fire when the setting flips.
        let autostart_first_run = std::rc::Rc::new(std::cell::Cell::new(true));
        Effect::new(move |_| {
            let autostart = settings.with(|s| s.autostart);
            if autostart_first_run.get() {
                autostart_first_run.set(false);
                return;
            }
            spawn_local(async move {
                if autostart {
                    let _ = commands::enable_autostart().await;
                } else {
                    let _ = commands::disable_autostart().await;
                }
            });
        });

        // Cold-start autostart probe — populate a context signal read
        // by the Updates settings tab to show the current OS autostart
        // state (independent of the Settings flag, which may differ if
        // the OS state was changed outside the app).
        let autostart_enabled = RwSignal::new(false);
        provide_context(autostart_enabled);
        spawn_local(async move {
            if let Ok(enabled) = commands::is_autostart_enabled().await {
                autostart_enabled.set(enabled);
            }
        });

        // Phase 4e R-004: session persistence sink. The shared
        // `sessions` signal is appended to by TimerView's tick
        // closure on `PomodoroCompleted` events. The Effect re-runs
        // on every push and persists the full bulk list via
        // `bridge::commands::save_manual_sessions`. Like the
        // settings sink, the first effect run is skipped so the
        // load → save cycle doesn't tail-chase.
        //
        // Manual session CRUD also runs through this signal (the
        // CalendarView add/edit/delete handlers update the same
        // `RwSignal<Vec<ManualSession>>`), so a single sink
        // captures both auto-completed timer sessions and manual
        // backfills. Bulk-rewrite matches the JS-era
        // `saveSessionsToStorage` flow at
        // `src/managers/session-manager.js:54-78`.
        let sessions_first_run = std::rc::Rc::new(std::cell::Cell::new(true));
        Effect::new(move |_| {
            let snapshot = sessions.get();
            if sessions_first_run.get() {
                sessions_first_run.set(false);
                return;
            }
            spawn_local(async move {
                let _ = commands::save_manual_sessions(snapshot).await;
            });
        });

        // Phase 4e R-004: cold-start hydration for session/tag lists.
        // Read the persisted bulk lists into the shared signals so the
        // CalendarView / TagsView starting state matches disk.
        spawn_local(async move {
            if let Ok(loaded) = commands::load_manual_sessions().await {
                // Bypass the persistence sink's first-run guard by
                // setting before any user mutation lands. The Effect
                // above sees this as the FIRST signal value and skips
                // the round-trip back to disk.
                sessions.set(loaded);
            }
        });
        // R-004: cold-start session-data hydration. Restores the
        // accumulated pomodoro counter state from disk so the
        // progress dots reflect sessions completed before the last
        // process restart. The signal is provided via context;
        // TimerView reads it on mount to pre-populate completed_pomodoros.
        spawn_local(async move {
            if let Ok(loaded) = commands::load_session_data().await {
                session_data.set(loaded);
            }
        });

        // Tag list hydration. Today the TimerView owns its own
        // local `RwSignal<Vec<Tag>>` because the JS-era surface
        // had a single dropdown anchored under `#timer-status`;
        // threading the shared `tags` context into that signal is
        // the load-bearing follow-up. The cold-start load runs
        // here so the eventual refactor consumes a populated
        // context — until then the loaded list is observed via
        // the persistence sink below (which writes any future
        // mutation through the bridge).
        spawn_local(async move {
            if let Ok(loaded) = commands::load_tags().await {
                tags.set(loaded);
            }
        });

        // Phase 4e R-004: tag persistence sink. Bulk re-save on
        // every mutation. The Tauri side has a single per-tag
        // `save_tag` and `delete_tag` rather than a bulk rewrite,
        // so we don't have a clean bulk-save command — instead the
        // sink exists to serialize new tags as they're added,
        // matching the JS-era `saveTagsToStorage` flow. Today the
        // sink is wired but most mutations still happen on a
        // local TimerView signal; once the TimerView consumes the
        // shared `tags` context, this will fire on every tag CRUD.
        let tags_first_run = std::rc::Rc::new(std::cell::Cell::new(true));
        Effect::new(move |_| {
            let snapshot = tags.get();
            if tags_first_run.get() {
                tags_first_run.set(false);
                return;
            }
            spawn_local(async move {
                // Per-tag save: iterate the list and re-save each.
                // The Tauri-side handler is idempotent on id so a
                // re-save of an unchanged tag is a no-op.
                for tag in snapshot {
                    let _ = commands::save_tag(tag).await;
                }
            });
        });

        // Subscribe to `tauri://update-available` emits. The
        // listener feeds the `UpdateManager`'s `handle_event` and
        // lifts the shared `update_info` signal — the
        // `UpdateNotification` banner reads that signal.
        spawn_local(async move {
            let mut update_mgr = UpdateManager::new();
            let listener =
                events::listen::<UpdateAvailablePayload>(UPDATE_AVAILABLE, move |payload| {
                    let skipped =
                        settings.with_untracked(|s| s.skipped_versions.contains(&payload.version));
                    if !skipped {
                        update_mgr.handle_event(payload);
                        update_info.set(update_mgr.info().clone());
                    }
                })
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
        // The sidebar carries a per-mode theme class (`focus` /
        // `break` / `longBreak`) so `style/sidebar.css`'s
        // `.sidebar.focus .sidebar-icon.active { background:
        // var(--focus-primary-btn) }` rule applies — the
        // visual-regression baseline shows the active nav button
        // with a saturated red background, which is gated on this
        // theme class. The Phase 4d cut wires only `focus` since
        // the timer engine starts in `Focus` mode and the visual
        // baseline is captured pre-start; Phase 4e attaches the
        // engine-mode → sidebar-class projection so break/long-break
        // states also flip the highlight color.
        <nav class="sidebar focus">
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

        // AuthModal is mounted at the App root (outside `.sidebar`)
        // because the sidebar carries `backdrop-filter`, which
        // establishes a containing block for `position: fixed`
        // descendants and would otherwise crop the auth overlay's
        // `width: 100vw` to the 80px sidebar column. The component
        // owns its own `position: fixed` wrapper for the avatar so
        // it still visually sits inside the sidebar bottom-left.
        // See `auth_modal.rs` rustdoc for the regression history
        // (auth.spec.js:26 "element is outside of the viewport").
        <AuthModal auth_state=auth_state/>

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
        </main>

        // Always-on overlays — the update banner is mounted at the
        // top level so navigation doesn't unmount it (the spec at
        // `update-notification.spec.js:32-34` asserts the banner's
        // dismissed flag survives Calendar → Timer round-trips).
        <UpdateNotification update_info=update_info settings=settings/>

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
