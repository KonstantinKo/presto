// Supabase auth wire types.
//
// Wire shape: `snake_case` JSON via serde defaults. Matches
// supabase-js's REST response shape directly so the Rust adapter
// (`src-tauri/src/auth.rs`) can deserialise the `/auth/v1/token`
// response into `AuthSession` without a translation layer.

use serde::{Deserialize, Serialize};

/// Supabase auth session record.
///
/// Distinct from the pomodoro `Session` by design — both types may
/// be imported at the same call site without conflict (the
/// data-model.md collision note renames this to `AuthSession`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub user: AuthUser,
}

/// Supabase auth user record embedded in `AuthSession`.
///
/// `user_metadata` is intentionally `serde_json::Value` (not a typed
/// struct) because Supabase's metadata is open-ended — apps store
/// per-tenant fields like `full_name`, `avatar_url`, OAuth-provider
/// claims, etc. The Leptos consumers (`managers/auth.rs`) read
/// specific keys via `.get("full_name")` rather than imposing a
/// closed shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub user_metadata: serde_json::Value,
}
