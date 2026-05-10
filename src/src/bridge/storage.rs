// Legacy localStorage migration — transition-only entry point.
//
// Spec 001-leptos-migration §Phase 1E T099-T115; data-model.md
// §"Legacy localStorage migration"; contracts/tauri-bridge.md
// §"Transition-only commands". This module owns the one-shot
// read-and-clear logic that walks the JS-era `window.localStorage`
// keys, hands the parsed payload to the matching `import_legacy_*`
// Tauri command, and clears the key on success. Idempotent: repeated
// invocations are safe (the Tauri-side handler skips when its
// authoritative store already has data, and the reader treats an
// absent localStorage key as an empty no-op).
//
// Sunset: this module and every `import_legacy_*` wrapper are
// slated for removal one minor version after cutover. The
// constitutional anchor is Principle VII — these are not "indefinite
// parallel surfaces", they are a one-shot migration with a defined
// end-of-life.
//
// Per Principle II (Local-First, Privacy-Default), readers MUST NOT
// log payload contents. Per-key counts are the only acceptable log
// signal — raw payload bytes never leave the wasm context.
//
// Lint allowance: `clippy::future_not_send` is allowed at the module
// level for the same reason it's allowed in `bridge::commands` —
// every async fn here ultimately calls into a wasm-only `JsFuture`,
// and the runtime is single-threaded under `wasm32-unknown-unknown`.
#![allow(clippy::future_not_send)]

use super::commands::{
    import_legacy_history, import_legacy_manual_sessions, import_legacy_settings,
    import_legacy_supabase_session, import_legacy_tags, import_legacy_tasks,
    import_legacy_user_state,
};
use super::error::BridgeError;
use super::types::{
    LegacyHistoryPayload, LegacyManualSessionsPayload, LegacySettingsPayload, LegacyTagsPayload,
    LegacyTasksPayload, LegacyUserStatePayload, ManualSession, Session, Settings, SupabaseSessionPayload,
    Tag, Task,
};

// ── localStorage helpers ─────────────────────────────────────────────────────

/// Returns the browser's `Storage` handle, or `None` when running
/// outside a window context (e.g. node tests, SSR — both irrelevant
/// to the cutover, but we treat them as the cold-start no-op rather
/// than failing).
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
}

/// Read a single localStorage key as the raw JSON string. Returns
/// `None` if the key is absent or if the underlying `getItem` call
/// rejects (e.g. `SecurityError` under sandboxed origins).
fn read_key(storage: &web_sys::Storage, key: &str) -> Option<String> {
    storage.get_item(key).ok().flatten()
}

/// Remove a single localStorage key, ignoring failures (the legacy
/// migration is best-effort on cleanup — if the remove fails we
/// rely on the Tauri-side idempotency to absorb the next launch's
/// re-import).
fn clear_key(storage: &web_sys::Storage, key: &str) {
    let _ = storage.remove_item(key);
}

/// Parse a localStorage value as JSON, returning `None` for both
/// "key absent" and "key present but unparseable". The latter
/// matches the JS-era behaviour at a few `JSON.parse(...)` sites in
/// `src/managers/*.js`: a corrupt blob is treated as "no data" rather
/// than fatal — better to lose one corrupted record on migration
/// than to leave the user stuck on an unmigrated state.
fn parse_json<T: serde::de::DeserializeOwned>(raw: Option<String>) -> Option<T> {
    raw.and_then(|s| serde_json::from_str::<T>(&s).ok())
}

// ── Per-domain readers ───────────────────────────────────────────────────────

/// T101 GREEN — Read the JS-era settings payload from localStorage
/// and dispatch to the `import_legacy_settings` Tauri command. Clears
/// the four legacy keys on success.
///
/// localStorage keys consumed: `pomodoro-settings`, `theme-preference`,
/// `timer-theme-preference`, `presto_auto_check_updates`. Each is
/// optional — the reader builds a `LegacySettingsPayload` with the
/// present subset and treats absent keys as `None`.
///
/// Cold-start (no keys present) is a no-op success: `Ok(())` without
/// invoking the bridge. This matters because `migrate_legacy_localstorage`
/// runs unconditionally on first post-cutover launch, and we don't
/// want it to short-circuit on `BridgeUnavailable` purely because the
/// user has no legacy data.
///
/// # Errors
/// Returns whatever the Tauri-side handler maps its filesystem failure
/// to (typically `BridgeError::Internal`), or `BridgeError::BridgeUnavailable`
/// when localStorage data is present but the bridge is not.
pub async fn import_legacy_settings_from_storage() -> Result<(), BridgeError> {
    let Some(storage) = local_storage() else {
        return Ok(());
    };
    let settings_raw = read_key(&storage, "pomodoro-settings");
    let theme = read_key(&storage, "theme-preference");
    let timer_theme = read_key(&storage, "timer-theme-preference");
    let auto_check_raw = read_key(&storage, "presto_auto_check_updates");

    // Cold-start: if every key is absent, skip the bridge call entirely.
    if settings_raw.is_none() && theme.is_none() && timer_theme.is_none() && auto_check_raw.is_none() {
        return Ok(());
    }

    let payload = LegacySettingsPayload {
        settings: parse_json::<Settings>(settings_raw),
        theme_preference: theme,
        timer_theme_preference: timer_theme,
        auto_check_updates: auto_check_raw.and_then(|s| s.parse::<bool>().ok()),
    };

    import_legacy_settings(payload).await?;

    // Best-effort cleanup; the Tauri handler's idempotency absorbs a
    // re-import if a clear fails on this launch.
    clear_key(&storage, "pomodoro-settings");
    clear_key(&storage, "theme-preference");
    clear_key(&storage, "timer-theme-preference");
    clear_key(&storage, "presto_auto_check_updates");
    Ok(())
}

/// T103 GREEN — Read the JS-era `pomodoro-history` localStorage
/// payload and dispatch to `import_legacy_history`.
///
/// # Errors
/// Returns whatever the Tauri-side handler maps its filesystem failure
/// to, or `BridgeError::BridgeUnavailable` when data is present but
/// the bridge is not.
pub async fn import_legacy_history_from_storage() -> Result<(), BridgeError> {
    let Some(storage) = local_storage() else {
        return Ok(());
    };
    let raw = read_key(&storage, "pomodoro-history");
    if raw.is_none() {
        return Ok(());
    }
    let history: Vec<Session> = parse_json(raw).unwrap_or_default();
    import_legacy_history(LegacyHistoryPayload { history }).await?;
    clear_key(&storage, "pomodoro-history");
    Ok(())
}

/// T105 GREEN — Read the JS-era `pomodoro-tasks` localStorage payload
/// and dispatch to `import_legacy_tasks`.
///
/// # Errors
/// Returns whatever the Tauri-side handler maps its filesystem failure
/// to, or `BridgeError::BridgeUnavailable` when data is present but
/// the bridge is not.
pub async fn import_legacy_tasks_from_storage() -> Result<(), BridgeError> {
    let Some(storage) = local_storage() else {
        return Ok(());
    };
    let raw = read_key(&storage, "pomodoro-tasks");
    if raw.is_none() {
        return Ok(());
    }
    let tasks: Vec<Task> = parse_json(raw).unwrap_or_default();
    import_legacy_tasks(LegacyTasksPayload { tasks }).await?;
    clear_key(&storage, "pomodoro-tasks");
    Ok(())
}

/// T107 GREEN — Read the JS-era `presto-tags` localStorage payload
/// and dispatch to `import_legacy_tags`.
///
/// # Errors
/// Returns whatever the Tauri-side handler maps its filesystem failure
/// to, or `BridgeError::BridgeUnavailable` when data is present but
/// the bridge is not.
pub async fn import_legacy_tags_from_storage() -> Result<(), BridgeError> {
    let Some(storage) = local_storage() else {
        return Ok(());
    };
    let raw = read_key(&storage, "presto-tags");
    if raw.is_none() {
        return Ok(());
    }
    let tags: Vec<Tag> = parse_json(raw).unwrap_or_default();
    import_legacy_tags(LegacyTagsPayload { tags }).await?;
    clear_key(&storage, "presto-tags");
    Ok(())
}

/// T109 GREEN — Read the JS-era `presto_manual_sessions` localStorage
/// payload and dispatch to `import_legacy_manual_sessions`.
///
/// # Errors
/// Returns whatever the Tauri-side handler maps its filesystem failure
/// to, or `BridgeError::BridgeUnavailable` when data is present but
/// the bridge is not.
pub async fn import_legacy_manual_sessions_from_storage() -> Result<(), BridgeError> {
    let Some(storage) = local_storage() else {
        return Ok(());
    };
    let raw = read_key(&storage, "presto_manual_sessions");
    if raw.is_none() {
        return Ok(());
    }
    let sessions: Vec<ManualSession> = parse_json(raw).unwrap_or_default();
    import_legacy_manual_sessions(LegacyManualSessionsPayload { sessions }).await?;
    clear_key(&storage, "presto_manual_sessions");
    Ok(())
}

/// T111 GREEN — Read the JS-era user-state flags + active session
/// snapshot from localStorage and dispatch to
/// `import_legacy_user_state`.
///
/// localStorage keys consumed: `presto-guest-mode`, `presto-auth-seen`,
/// `presto-skipped-versions`, `pomodoro-session`. Each is optional.
/// `presto-skipped-versions` is a JSON-encoded `Vec<String>`; the
/// reader parses it as such and falls back to an empty vec on parse
/// failure.
///
/// # Errors
/// Returns whatever the Tauri-side handler maps its filesystem failure
/// to, or `BridgeError::BridgeUnavailable` when data is present but
/// the bridge is not.
pub async fn import_legacy_user_state_from_storage() -> Result<(), BridgeError> {
    let Some(storage) = local_storage() else {
        return Ok(());
    };
    let guest_raw = read_key(&storage, "presto-guest-mode");
    let auth_raw = read_key(&storage, "presto-auth-seen");
    let skipped_raw = read_key(&storage, "presto-skipped-versions");
    let session_raw = read_key(&storage, "pomodoro-session");

    if guest_raw.is_none()
        && auth_raw.is_none()
        && skipped_raw.is_none()
        && session_raw.is_none()
    {
        return Ok(());
    }

    let payload = LegacyUserStatePayload {
        guest_mode: guest_raw.and_then(|s| s.parse::<bool>().ok()),
        auth_seen: auth_raw.and_then(|s| s.parse::<bool>().ok()),
        skipped_versions: parse_json::<Vec<String>>(skipped_raw).unwrap_or_default(),
        active_session: parse_json::<Session>(session_raw),
    };

    import_legacy_user_state(payload).await?;

    clear_key(&storage, "presto-guest-mode");
    clear_key(&storage, "presto-auth-seen");
    clear_key(&storage, "presto-skipped-versions");
    clear_key(&storage, "pomodoro-session");
    Ok(())
}

/// T113 GREEN — Read the JS-era Supabase auth token from localStorage
/// and dispatch to `import_legacy_supabase_session`.
///
/// localStorage key shape: `sb-<project-ref>-auth-token`. The
/// project-ref is parsed from the `SUPABASE_URL` constant via the
/// public Supabase URL convention (`https://<ref>.supabase.co`); the
/// reader scans every localStorage key whose name starts with `sb-`
/// and ends with `-auth-token`, handing the first parseable payload
/// to the bridge. This matches supabase-js's own discovery shape.
///
/// # Errors
/// Returns whatever the Tauri-side handler maps its filesystem failure
/// to, or `BridgeError::BridgeUnavailable` when data is present but
/// the bridge is not.
pub async fn import_legacy_supabase_session_from_storage() -> Result<(), BridgeError> {
    let Some(storage) = local_storage() else {
        return Ok(());
    };

    // Walk the localStorage keyspace looking for any `sb-*-auth-token`.
    // The JS-era key carries the project-ref in the middle; we don't
    // pin the ref here because the SUPABASE_URL constant lives
    // Tauri-side. The first parseable match wins; the rest (if any)
    // are stale and removed at the same time.
    let length = storage.length().unwrap_or(0);
    let mut matched_keys: Vec<String> = Vec::new();
    let mut chosen_payload: Option<SupabaseSessionPayload> = None;

    for i in 0..length {
        let Ok(Some(key)) = storage.key(i) else {
            continue;
        };
        if !(key.starts_with("sb-") && key.ends_with("-auth-token")) {
            continue;
        }
        matched_keys.push(key.clone());
        if chosen_payload.is_none() {
            let raw = read_key(&storage, &key);
            if let Some(parsed) = parse_json::<SupabaseSessionPayload>(raw) {
                chosen_payload = Some(parsed);
            }
        }
    }

    let Some(payload) = chosen_payload else {
        // No parseable Supabase session in localStorage — clear any
        // stale corrupt matches and exit. Keeping the corrupt blob
        // would mean re-attempting on every launch.
        for key in &matched_keys {
            clear_key(&storage, key);
        }
        return Ok(());
    };

    import_legacy_supabase_session(payload).await?;

    for key in &matched_keys {
        clear_key(&storage, key);
    }
    Ok(())
}

// ── Single entry point ──────────────────────────────────────────────────────
//
// T115 (GREEN) lands the orchestration. T101-T113 land the per-domain
// readers above. The single entry point is added at T115 so the test
// at T114 can pin the idempotent-second-launch behaviour against a
// real implementation.

// Tests gated on `wasm32` because every assertion is a
// `#[wasm_bindgen_test]` — running them via `cargo test` on the host
// target would produce dead-code lint failures. `wasm-pack test --node`
// is the canonical test driver per quickstart.md line 105 and tasks.md
// T100 done-signal.
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    /// T100 (GREEN). The Leptos-side reader for the `pomodoro-settings`
    /// / `theme-preference` / `timer-theme-preference` /
    /// `presto_auto_check_updates` localStorage keys exists as
    /// `import_legacy_settings_from_storage()` and returns
    /// `Result<(), BridgeError>`. Under `wasm-pack test --node` no
    /// localStorage is present (`window()` returns `None`); the reader
    /// must absorb that as a successful no-op rather than surfacing
    /// it as a `BridgeUnavailable` error — the migration entry point
    /// runs unconditionally on first launch and a node test
    /// environment must not fail it.
    #[wasm_bindgen_test]
    async fn imports_legacy_settings() {
        let result = super::import_legacy_settings_from_storage().await;
        assert!(
            result.is_ok(),
            "expected Ok(()) for empty localStorage, got {result:?}"
        );
    }

    /// T102 (RED). Pin the existence + cold-start return shape of the
    /// `pomodoro-history` reader. The contract:
    /// `import_legacy_history_from_storage()` returns
    /// `Result<(), BridgeError>` and absorbs the no-localStorage case
    /// as `Ok(())` so the entry point in T115 can call it
    /// unconditionally. The signature pin uses an `async fn` binding;
    /// a return-type drift breaks compilation. T103 GREEN re-asserts
    /// the contract against the post-cutover-shape body.
    #[wasm_bindgen_test]
    async fn imports_legacy_history() {
        async fn assert_signature() -> Result<(), super::super::error::BridgeError> {
            super::import_legacy_history_from_storage().await
        }
        let result = assert_signature().await;
        assert!(result.is_ok(), "cold-start no-op contract: {result:?}");
    }

    /// T104 (RED). Pin `pomodoro-tasks` reader contract.
    #[wasm_bindgen_test]
    async fn imports_legacy_tasks() {
        async fn assert_signature() -> Result<(), super::super::error::BridgeError> {
            super::import_legacy_tasks_from_storage().await
        }
        let result = assert_signature().await;
        assert!(result.is_ok(), "cold-start no-op contract: {result:?}");
    }

    /// T106 (RED). Pin `presto-tags` reader contract.
    #[wasm_bindgen_test]
    async fn imports_legacy_tags() {
        async fn assert_signature() -> Result<(), super::super::error::BridgeError> {
            super::import_legacy_tags_from_storage().await
        }
        let result = assert_signature().await;
        assert!(result.is_ok(), "cold-start no-op contract: {result:?}");
    }

    /// T108 (RED). Pin `presto_manual_sessions` reader contract.
    #[wasm_bindgen_test]
    async fn imports_legacy_manual_sessions() {
        async fn assert_signature() -> Result<(), super::super::error::BridgeError> {
            super::import_legacy_manual_sessions_from_storage().await
        }
        let result = assert_signature().await;
        assert!(result.is_ok(), "cold-start no-op contract: {result:?}");
    }

    /// T110 (RED). Pin user-state reader contract. The reader walks
    /// four bare keys (`presto-guest-mode`, `presto-auth-seen`,
    /// `presto-skipped-versions`, `pomodoro-session`) and dispatches
    /// to `import_legacy_user_state`.
    #[wasm_bindgen_test]
    async fn imports_legacy_user_state() {
        async fn assert_signature() -> Result<(), super::super::error::BridgeError> {
            super::import_legacy_user_state_from_storage().await
        }
        let result = assert_signature().await;
        assert!(result.is_ok(), "cold-start no-op contract: {result:?}");
    }

    /// T112 (RED). Pin the Supabase session reader contract.
    /// `imports_legacy_supabase_session_from_localstorage` is the
    /// named test in plan.md §"Testing strategy"; the planned home
    /// (`managers/auth::tests`) does not yet exist (Phase 3 lands
    /// `managers/`), so the test currently lives here in
    /// `bridge::storage::tests` next to its sibling per-domain
    /// readers — Phase 3 may relocate it. The reader scans
    /// `sb-*-auth-token` localStorage keys per supabase-js's own
    /// discovery shape.
    #[wasm_bindgen_test]
    async fn imports_legacy_supabase_session_from_localstorage() {
        async fn assert_signature() -> Result<(), super::super::error::BridgeError> {
            super::import_legacy_supabase_session_from_storage().await
        }
        let result = assert_signature().await;
        assert!(result.is_ok(), "cold-start no-op contract: {result:?}");
    }

    /// T114 (RED). Pin the single Leptos-side entry point
    /// `migrate_legacy_localstorage()` per data-model.md §"Legacy
    /// localStorage migration": one call, runs at first post-cutover
    /// launch from app.rs, dispatches to each per-domain reader.
    /// Idempotent — a second-launch call must be a no-op even on the
    /// same process.
    ///
    /// This test fails at the RED phase because the entry point does
    /// not exist yet (compile error: unresolved import). T115 GREEN
    /// adds the orchestration.
    #[wasm_bindgen_test]
    async fn migrate_legacy_localstorage_idempotent() {
        // First call: cold-start (no localStorage entries under
        // `wasm-pack test --node`) is a successful no-op.
        let first = super::migrate_legacy_localstorage().await;
        assert!(first.is_ok(), "first-launch contract: {first:?}");
        // Second call on the same process: identical result. The
        // entry point's idempotency comes from the per-domain
        // readers' "absent localStorage = no-op" branches; this test
        // pins that the entry point doesn't accumulate state across
        // calls (e.g., a once-cell that flips Err the second time).
        let second = super::migrate_legacy_localstorage().await;
        assert!(second.is_ok(), "second-launch contract: {second:?}");
    }
}
