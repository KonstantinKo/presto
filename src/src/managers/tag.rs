// `TagManager` — the Rust port of `src/managers/tag-manager.js`.
//
// Spec 001-leptos-migration §Phase 3b (T161-T166). Owns the user's
// `Tag` list and exposes the per-tag CRUD path: create, delete, and
// the load-time list reduction that filters corrupt records out of
// the on-disk wire shape (mirrors the JS-side validation at
// `src/managers/tag-manager.js:126-146`). Per Principle VI, the
// async wrappers reach the Tauri side only through
// `bridge::commands::{load_tags, save_tag, delete_tag}` — the
// manager never touches `__TAURI_INTERNALS__` directly.
//
// Lint allowance: `clippy::future_not_send` is allowed at the module
// level for the same reason as on `bridge::commands` and
// `managers::settings` — every async path here transitively awaits a
// `JsFuture` from `bridge::commands`, and `JsValue` (and everything
// built on it) is `!Send` by construction on `wasm32-unknown-unknown`.
// The runtime is single-threaded; demanding `Send` would force a
// `!Send`-erasure shim that does nothing on the WASM target.
#![allow(clippy::future_not_send)]

use crate::bridge::commands;
use crate::bridge::error::BridgeError;
use crate::bridge::types::Tag;

/// Wrapper over the user's tag list. Phase 3b wires up the state
/// machine; per-tag CRUD lands in T162/T164/T166.
#[derive(Debug, Clone, Default)]
pub struct TagManager {
    /// Current authoritative tag list. Populated either by `load()`
    /// (cold-start path) or `from_loaded(...)` (test path / hand-fed
    /// list). `Default::default()` produces an empty `Vec`, which
    /// matches the JS-side cold-start "no tags file yet" convention.
    tags: Vec<Tag>,
}

impl TagManager {
    /// Construct an empty manager. Use `load()` to seed from disk; use
    /// `from_loaded(...)` to ingest a `bridge::commands::load_tags()`
    /// result while applying the JS-side list-reduction validation.
    #[must_use]
    pub const fn new() -> Self {
        Self { tags: Vec::new() }
    }

    /// Borrow the current tag list.
    #[must_use]
    pub fn list(&self) -> &[Tag] {
        &self.tags
    }

    /// Append a new tag to the in-memory list. Pure mutation —
    /// `id`, `icon`, `color`, and `created_at` are supplied by the
    /// caller (Phase 4 components own the `crypto.randomUUID()` /
    /// `tag-<uuid>` shape and the ISO-8601 timestamp). Mirrors the
    /// JS-side `src/managers/tag-manager.js:286-303` push-then-save
    /// flow: the manager updates state synchronously and the
    /// caller hands the same `Tag` to `save_new` to durable-store
    /// it. Spec 001-leptos-migration §Phase 3b T162.
    pub fn create(&mut self, tag: Tag) {
        self.tags.push(tag);
    }

    /// Async persist path: hand `tag` to
    /// `bridge::commands::save_tag` (per Principle VI — managers
    /// reach the Tauri side only through the typed bridge wrapper).
    /// Mirrors the JS-side `await invoke("save_tag", { tag })` at
    /// `src/managers/tag-manager.js:298`.
    ///
    /// Per the JS-era flow, the in-memory `tags` list is mutated by
    /// `create()` synchronously and the persist call is best-effort
    /// — the on-disk store catches up after the async hop. The
    /// caller pairs the two: `mgr.create(tag.clone());
    /// mgr.save_new(tag).await?`.
    ///
    /// # Errors
    /// Returns whatever `bridge::commands::save_tag` returns —
    /// `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
    /// not present (Trunk dev server, e2e mock, host tests), or
    /// whichever variant the Tauri-side handler maps its filesystem
    /// failure to.
    pub async fn save_new(&self, tag: Tag) -> Result<(), BridgeError> {
        commands::save_tag(tag).await
    }
}

#[cfg(test)]
mod tests {
    use super::TagManager;
    use crate::bridge::types::Tag;

    /// T161 [RED]: `create` mutates the manager's tag list by
    /// appending the supplied `Tag` and returns the now-current
    /// tag count, mirroring the JS-side
    /// `src/managers/tag-manager.js:286-303` flow where a new tag
    /// is pushed onto `this.tags` and rendered. The Rust port keeps
    /// `id` generation outside the manager (Phase 4 components own
    /// the `crypto.randomUUID()` / `tag-<uuid>` shape) so the
    /// manager API is pure and the test path stays off the
    /// wasm-bindgen boundary.
    ///
    /// Done-signal: this test currently fails because
    /// `TagManager::create` does not yet exist. T162 GREEN attaches
    /// the implementation and the matching async
    /// `save_new` wrapper that hands the tag to
    /// `bridge::commands::save_tag`.
    #[test]
    fn create_returns_new_tag_with_id() {
        let mut mgr = TagManager::new();
        let tag = Tag {
            id: "tag-focus".to_string(),
            name: "Focus".to_string(),
            icon: "ri-brain-line".to_string(),
            color: "#4CAF50".to_string(),
            created_at: "2026-05-10T00:00:00Z".to_string(),
        };
        mgr.create(tag);

        assert_eq!(mgr.list().len(), 1, "create must append exactly one tag");
        assert_eq!(
            mgr.list()[0].id,
            "tag-focus",
            "the appended tag must carry the supplied id",
        );
        assert_eq!(mgr.list()[0].name, "Focus");
    }
}
