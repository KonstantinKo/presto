// Event payloads emitted by the Tauri backend.
//
// Spec 001-leptos-migration §Phase 1F T116-T117; contracts/tauri-bridge.md
// §"Tauri events". Most events carry a `()` payload (the activity and
// tray-menu emits) or a primitive (`String` for `global-shortcut` and
// `oauth-callback`) — those don't need a dedicated struct. The non-trivial
// event payload here is `tauri://update-available`; the other named event
// payload (`shortcuts-updated`) reuses `settings::ShortcutSettings`.

use serde::{Deserialize, Serialize};

/// Payload for the `tauri://update-available` event emitted by
/// `tauri-plugin-updater` when the auto-updater detects a newer
/// release. Mirrors the plugin's `Update` JSON shape.
///
/// Fields mirror the upstream plugin's emit; we deserialise only the
/// three the Leptos consumer (`managers/update.rs`) needs. `serde`'s
/// default-on-unknown-field behaviour silently drops anything else
/// (`available`, `current_version`, etc.) — the contract is the named
/// fields below; future plugin additions are non-breaking.
///
/// `body` is the changelog/release-notes blob (markdown). `date` is
/// the release publish date as the upstream-emitted RFC-2822-ish
/// string; we keep it as `String` to avoid pulling chrono into the
/// event-payload surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct UpdateAvailablePayload {
    pub version: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}
