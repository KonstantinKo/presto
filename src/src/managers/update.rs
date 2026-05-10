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
}
