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
#![allow(
    clippy::future_not_send,
    reason = "Manager async paths await wasm32 Tauri bridge futures that carry !Send JsValue."
)]

use crate::bridge::commands;
use crate::bridge::types::BridgeError;
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

    /// Remove the tag with the matching `id` from the in-memory
    /// list. Pure mutation — delete-of-unknown-id is a no-op
    /// (mirrors the JS-side `this.tags.filter(t => t.id !== id)`
    /// semantics at `src/managers/tag-manager.js:332`). The
    /// matching async `delete_persisted` hop reaches the Tauri
    /// side through `bridge::commands::delete_tag`. Spec
    /// 001-leptos-migration §Phase 3b T164.
    pub fn delete(&mut self, tag_id: &str) {
        self.tags.retain(|t| t.id != tag_id);
    }

    /// Look up a tag by its `id`. Returns `None` if no such tag is
    /// in the list. Used by the components layer (Phase 4) to
    /// resolve `currentTags` membership and by the
    /// `delete_removes_from_list` test pin.
    #[must_use]
    pub fn find_by_id(&self, id: &str) -> Option<&Tag> {
        self.tags.iter().find(|t| t.id == id)
    }

    /// Async persist-delete path: hand `tag_id` to
    /// `bridge::commands::delete_tag` (per Principle VI — managers
    /// reach the Tauri side only through the typed bridge wrapper).
    /// Mirrors the JS-side `await invoke("delete_tag", { tag_id })`
    /// at `src/managers/tag-manager.js:329`.
    ///
    /// Per the JS-era flow, the in-memory `tags` list is mutated by
    /// `delete()` synchronously and the persist call is best-effort
    /// — the on-disk store catches up after the async hop.
    ///
    /// # Errors
    /// Returns whatever `bridge::commands::delete_tag` returns —
    /// `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
    /// not present, or whichever variant the Tauri-side handler
    /// maps its filesystem failure to.
    pub async fn delete_persisted(&self, tag_id: String) -> Result<(), BridgeError> {
        commands::delete_tag(tag_id).await
    }

    /// Build a manager from a loaded tag list, applying the
    /// JS-side `_loadTagsFromLocalStorage` validation at
    /// `src/managers/tag-manager.js:126-146`:
    ///
    /// - Each record must carry non-empty `id` and non-empty `name`;
    ///   entries that fail either check are dropped silently
    ///   (per-record strictness, distinct from the JS-era surface
    ///   that resets the entire list on any invalid entry — a
    ///   single corrupt record poisoning the whole list is hostile
    ///   to user data).
    /// - Duplicate ids are de-duplicated, first occurrence wins.
    ///   The JS-side `tags.some(t => t.id === ct.id)` lookups at
    ///   lines 160 and 185 implicitly assume each id appears at
    ///   most once.
    ///
    /// Empty input reduces to an empty manager — the JS-era default
    /// `default-focus` seed at lines 148-159 is a first-run UX
    /// concern that lives in Phase 4 components, not the manager
    /// state machine.
    ///
    /// Spec 001-leptos-migration §Phase 3b T166.
    #[must_use]
    pub fn list_reduction(loaded: Vec<Tag>) -> Self {
        let mut tags: Vec<Tag> = Vec::with_capacity(loaded.len());
        for tag in loaded {
            if tag.id.is_empty() || tag.name.is_empty() {
                continue;
            }
            if tags.iter().any(|t| t.id == tag.id) {
                continue;
            }
            tags.push(tag);
        }
        Self { tags }
    }

    /// Build a manager from the result of
    /// `bridge::commands::load_tags()` (or any equivalent loader),
    /// applying the JS-side list-reduction validation and falling
    /// back to an empty list on error. Mirrors the JS-side
    /// catch-and-default at
    /// `src/managers/tag-manager.js:168-196`: persistence failures
    /// (missing file, deserialise error, bridge unavailable) must
    /// not poison the manager's state.
    ///
    /// The reduction is applied unconditionally on success so a
    /// corrupted record on disk doesn't poison the in-memory list.
    #[must_use]
    pub fn from_loaded_or_default(loaded: Result<Vec<Tag>, BridgeError>) -> Self {
        loaded.map_or_else(|_| Self::new(), Self::list_reduction)
    }

    /// Async cold-start path: ask the bridge for the persisted
    /// tags, fall back to an empty list on any error (cold start,
    /// bridge unavailable, corrupted file). Mirrors the JS-side
    /// `loadTags` flow at `src/managers/tag-manager.js:168-196`,
    /// minus the localStorage fallback (Phase 1E `import_legacy_tags`
    /// already migrated those records to the Rust-side store).
    ///
    /// The reduction is applied unconditionally so a corrupted
    /// record on disk doesn't poison the in-memory list.
    pub async fn load() -> Self {
        Self::from_loaded_or_default(commands::load_tags().await)
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

    /// T163 [RED]: `delete(tag_id)` removes the matching record
    /// from the in-memory list. Mirrors the JS-side
    /// `src/managers/tag-manager.js:332-333` filter pattern. The
    /// test seeds two tags, deletes the first by id, and asserts
    /// only the second survives. Also pins that
    /// `find_by_id` returns `Some(_)` for the surviving tag and
    /// `None` for the deleted one — the JS-era `tags.some(t =>
    /// t.id === id)` callsites at lines 160, 185, and 332 all
    /// reduce to this lookup.
    ///
    /// Done-signal: this test currently fails because
    /// `TagManager::delete` and `find_by_id` do not yet exist.
    /// T164 GREEN attaches both alongside the async
    /// `delete_persisted` wrapper that hands the id to
    /// `bridge::commands::delete_tag`.
    #[test]
    fn delete_removes_from_list() {
        let mut mgr = TagManager::new();
        mgr.create(Tag {
            id: "tag-focus".to_string(),
            name: "Focus".to_string(),
            icon: "ri-brain-line".to_string(),
            color: "#4CAF50".to_string(),
            created_at: "2026-05-10T00:00:00Z".to_string(),
        });
        mgr.create(Tag {
            id: "tag-meeting".to_string(),
            name: "Meeting".to_string(),
            icon: "ri-team-line".to_string(),
            color: "#3b82f6".to_string(),
            created_at: "2026-05-10T00:01:00Z".to_string(),
        });
        assert_eq!(mgr.list().len(), 2, "two tags seeded");

        mgr.delete("tag-focus");

        assert_eq!(mgr.list().len(), 1, "one tag remains after delete");
        assert_eq!(
            mgr.list()[0].id,
            "tag-meeting",
            "the surviving tag is the one that was NOT deleted",
        );
        assert!(
            mgr.find_by_id("tag-focus").is_none(),
            "find_by_id must return None for the deleted tag",
        );
        assert!(
            mgr.find_by_id("tag-meeting").is_some(),
            "find_by_id must return Some(_) for the surviving tag",
        );

        // Deleting a missing id is a no-op (matches the JS-side
        // `filter(t => t.id !== id)` semantics — no error if the
        // id wasn't in the list).
        mgr.delete("tag-nope");
        assert_eq!(mgr.list().len(), 1, "delete of unknown id is a no-op");
    }

    /// T165 [RED]: `list_reduction(loaded)` is the load-time filter
    /// that mirrors the JS-side `_loadTagsFromLocalStorage` validation
    /// at `src/managers/tag-manager.js:126-146`. Each loaded record
    /// must carry a non-empty `id` and a non-empty `name`; entries
    /// that fail either check are dropped (the JS-era surface
    /// resets the entire list to `[]` on any invalid record, but the
    /// Rust port is stricter on the per-record level so a single
    /// corrupt entry doesn't poison the survivors). The reduction
    /// also de-duplicates on `id` — the JS-side `tags.some(t =>
    /// t.id === ct.id)` callsites at lines 160 and 185 implicitly
    /// rely on each id appearing at most once.
    ///
    /// Done-signal: this test currently fails because
    /// `TagManager::list_reduction` does not yet exist.
    /// T166 GREEN attaches it.
    #[test]
    fn list_reduction_handles_loaded_set() {
        // Mixed input: two valid, one with empty id, one with empty
        // name, one duplicate id (the second occurrence is dropped).
        let valid_a = Tag {
            id: "tag-a".to_string(),
            name: "Alpha".to_string(),
            icon: "ri-a-line".to_string(),
            color: "#111111".to_string(),
            created_at: "2026-05-10T00:00:00Z".to_string(),
        };
        let valid_b = Tag {
            id: "tag-b".to_string(),
            name: "Beta".to_string(),
            icon: "ri-b-line".to_string(),
            color: "#222222".to_string(),
            created_at: "2026-05-10T00:01:00Z".to_string(),
        };
        let invalid_empty_id = Tag {
            id: String::new(),
            name: "EmptyId".to_string(),
            icon: String::new(),
            color: String::new(),
            created_at: String::new(),
        };
        let invalid_empty_name = Tag {
            id: "tag-c".to_string(),
            name: String::new(),
            icon: String::new(),
            color: String::new(),
            created_at: String::new(),
        };
        let duplicate_a = Tag {
            id: "tag-a".to_string(),
            name: "AlphaDup".to_string(),
            icon: "ri-dup-line".to_string(),
            color: "#999999".to_string(),
            created_at: "2026-05-10T00:02:00Z".to_string(),
        };

        let loaded = vec![
            valid_a,
            invalid_empty_id,
            valid_b,
            invalid_empty_name,
            duplicate_a,
        ];
        let mgr = TagManager::list_reduction(loaded);

        assert_eq!(
            mgr.list().len(),
            2,
            "reduction must keep exactly the two valid, non-duplicate records",
        );
        assert_eq!(mgr.list()[0].id, "tag-a", "first survivor is tag-a");
        assert_eq!(
            mgr.list()[0].name,
            "Alpha",
            "first occurrence wins on duplicate id (AlphaDup is dropped)",
        );
        assert_eq!(mgr.list()[1].id, "tag-b", "second survivor is tag-b");

        // Empty input reduces to an empty list (no default seeding —
        // the JS-side `default-focus` seed at lines 148-159 is a
        // first-run UX concern that lives in Phase 4 components,
        // not the manager state machine).
        let empty = TagManager::list_reduction(Vec::new());
        assert_eq!(
            empty.list().len(),
            0,
            "empty input must reduce to an empty list",
        );
    }
}
