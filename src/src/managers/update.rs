// `UpdateManager` — the Rust port of `src/managers/update-manager-global.js`.
//
// Spec 001-leptos-migration §Phase 3c (T183-T186). Owns the
// `UpdateInfo` enum (NoUpdate / Available { version, notes }) per
// data-model.md §`UpdateInfo`. Consumes the E10
// `tauri://update-available` event payload; the polling-cadence pin
// (1h, matching JS-era `update-manager-global.js:219`) carries the
// FR-004 update-cadence guarantee.
//
// Per Principle VI, the manager reaches the Tauri side only through
// the typed bridge surface (`bridge::events::UPDATE_AVAILABLE` for
// the listener, `bridge::types::UpdateAvailablePayload` for the
// event shape). Phase 3c wires up the state machine + the cadence
// pin; the actual subscription to the event bus lives in the
// components layer (Phase 4).

/// Update-availability state. Mirrors data-model.md §`UpdateInfo`.
///
/// Closed sum type with two variants — the `tauri-plugin-updater`
/// surface only ever emits "no newer release" or "newer release with
/// metadata", and the components layer (Phase 4) renders an
/// "update available" banner only on the `Available` branch.
///
/// `notes` is the changelog/release-notes blob (markdown), threaded
/// through unchanged from the upstream plugin emit. `version` is the
/// semver string (e.g., `"0.4.5"`); the JS-era flow at
/// `update-manager-global.js:243-260` strips a leading `v` prefix
/// before comparison, but the Rust port keeps the version as
/// emitted because `tauri-plugin-updater` already normalises.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateInfo {
    /// No newer release detected. Cold-start default and
    /// post-poll-with-no-result state. Mirrors the JS-era
    /// `updateAvailable: false` branch at
    /// `update-manager-global.js:9`.
    #[default]
    NoUpdate,
    /// `tauri-plugin-updater` reported a newer release. The
    /// components layer (Phase 4) shows the upgrade prompt; the
    /// user-driven install / skip is dispatched via separate paths
    /// (the install path goes through the plugin's `install`
    /// command, not through the manager).
    Available {
        /// Semver string of the available release (`"0.4.5"`).
        version: String,
        /// Release notes blob, markdown. `None` when the upstream
        /// emit didn't carry a body — matches the
        /// `UpdateAvailablePayload::body: Option<String>` shape on
        /// the bridge boundary.
        notes: Option<String>,
    },
}

/// Update-manager state machine. Wraps `UpdateInfo` and consumes the
/// `bridge::types::UpdateAvailablePayload` event payload published by
/// `tauri-plugin-updater` (the E10 `tauri://update-available`
/// event).
///
/// Phase 3c lands the cold-start state + the event-consumption path
/// (T183-T184); the polling-cadence pin lands in T185-T186. The
/// actual subscription to the event bus lives in the components
/// layer (Phase 4) — this manager exposes the typed
/// `handle_event(payload)` entry point that the listener trampoline
/// calls per emit, keeping the state-machine logic host-testable
/// per Principle V.
#[derive(Debug, Clone, Default)]
pub struct UpdateManager {
    info: UpdateInfo,
}

impl UpdateManager {
    /// Polling cadence between successive auto update-checks.
    /// Pinned at 1 hour to match the JS-era baseline at
    /// `src/managers/update-manager-global.js:219`
    /// (`setInterval(..., 60 * 60 * 1000)`).
    ///
    /// The components layer (Phase 4) reads this constant to drive
    /// the actual interval timer; the manager itself does not own a
    /// timer (the JS-era surface uses the browser's `setInterval`,
    /// which has no Rust equivalent that crosses the wasm boundary
    /// without a runtime — Leptos provides one in the components
    /// layer via `set_interval_with_handle`).
    ///
    /// Spec 001-leptos-migration §Phase 3c T186; FR-004
    /// update-cadence pin.
    pub const POLL_INTERVAL: core::time::Duration = core::time::Duration::from_hours(1);

    /// Construct a manager rooted at `UpdateInfo::NoUpdate`. Mirrors
    /// the JS-era cold-start at `update-manager-global.js:9`
    /// (`this.updateAvailable = false`).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            info: UpdateInfo::NoUpdate,
        }
    }

    /// Borrow the current `UpdateInfo`. Used by the components layer
    /// (Phase 4) to drive the upgrade-banner render and by tests to
    /// pin the post-event state shape.
    #[must_use]
    pub const fn info(&self) -> &UpdateInfo {
        &self.info
    }

    /// Consume an `UpdateAvailablePayload` from the
    /// `tauri://update-available` event and lift the manager to
    /// `UpdateInfo::Available { version, notes }`. Mirrors the
    /// JS-era `currentUpdate = update; updateAvailable = true`
    /// pair at `update-manager-global.js:115-118` minus the DOM
    /// effects (Phase 4 components own the banner render).
    ///
    /// Idempotent: re-emit of the same version overwrites the
    /// stored `notes` blob (matches the JS-era behaviour where the
    /// updater plugin's emit always wins) — no de-dup here because
    /// the per-version skip / install decisions live above the
    /// manager (the components layer fans the state change out to
    /// the user-driven prompt).
    ///
    /// Spec 001-leptos-migration §Phase 3c T184.
    pub fn handle_event(&mut self, payload: crate::bridge::types::UpdateAvailablePayload) {
        self.info = UpdateInfo::Available {
            version: payload.version,
            notes: payload.body,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::UpdateInfo;

    /// T183 [RED]: cold-start `UpdateInfo` is `NoUpdate`. Pins the
    /// default-state contract — the `tauri-plugin-updater` event has
    /// not fired yet, so the manager must report "no update" rather
    /// than uncommitted/unknown. Mirrors the JS-era cold-start at
    /// `update-manager-global.js:9` (`this.updateAvailable = false`).
    ///
    /// Done-signal: this test currently fails because
    /// `UpdateManager` and its `info()` accessor do not yet exist.
    /// T184 GREEN attaches them alongside `UpdateManager::handle_event`
    /// for the `tauri://update-available` payload consumption path.
    #[test]
    fn updateinfo_no_update_default() {
        let mgr = super::UpdateManager::new();
        assert_eq!(*mgr.info(), UpdateInfo::NoUpdate);
    }

    /// T184 [GREEN] complement: a `handle_event(payload)` call lifts
    /// the manager from `NoUpdate → Available { version, notes }`.
    /// Pins the `UpdateAvailablePayload → UpdateInfo` projection:
    /// `version` carries through unchanged; `body` re-shapes to
    /// `notes` (matching data-model.md §`UpdateInfo`'s `notes` field
    /// name, which differs from the upstream plugin's `body` for
    /// post-cutover clarity).
    #[test]
    fn handle_event_lifts_no_update_to_available() {
        use crate::bridge::types::UpdateAvailablePayload;

        let mut mgr = super::UpdateManager::new();
        assert_eq!(*mgr.info(), UpdateInfo::NoUpdate);

        let payload = UpdateAvailablePayload {
            version: "0.4.5".to_string(),
            body: Some("- bug fixes".to_string()),
            date: None,
        };
        mgr.handle_event(payload);

        match mgr.info() {
            UpdateInfo::Available { version, notes } => {
                assert_eq!(version, "0.4.5");
                assert_eq!(notes.as_deref(), Some("- bug fixes"));
            }
            other @ UpdateInfo::NoUpdate => panic!("expected Available, got {other:?}"),
        }
    }

    /// T185 [RED]: the polling cadence between successive auto
    /// update-checks MUST match the JS-era baseline at
    /// `src/managers/update-manager-global.js:219` —
    /// `setInterval(..., 60 * 60 * 1000)` = 1 hour.
    ///
    /// The Rust port exposes the cadence as
    /// `UpdateManager::POLL_INTERVAL` (a `core::time::Duration`),
    /// pinned by this test so a future drift to a different cadence
    /// (e.g., 24h or 6h) breaks the build rather than silently
    /// drifting from the established UX baseline. The components
    /// layer (Phase 4) drives the actual interval timer; this pin
    /// is the canonical source of truth.
    ///
    /// Done-signal: this test currently fails because
    /// `UpdateManager::POLL_INTERVAL` does not yet exist. T186
    /// GREEN attaches the constant.
    #[test]
    fn polling_cadence_matches_jsbaseline() {
        assert_eq!(
            super::UpdateManager::POLL_INTERVAL,
            core::time::Duration::from_hours(1),
            "JS-era baseline at update-manager-global.js:219 is 60 * 60 * 1000 ms (= 1 hour)",
        );
    }
}
