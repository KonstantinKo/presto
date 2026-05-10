// Supabase auth REST adapter — replaces the JS `supabase-js` SDK.
//
// Spec 001-leptos-migration §Phase 1D T088-T095; research.md §6 (Supabase
// auth SDK replacement). The Tauri-side adapter hits the four Supabase
// REST endpoints directly (`/auth/v1/token`, `/auth/v1/logout`,
// `/auth/v1/token?grant_type=refresh_token`) and persists the resulting
// session to the app-data directory in a single JSON file
// (`supabase-session.json`). The on-disk shape mirrors `AuthSession` so
// the supabase_get_session command reads it back without a translation
// layer.
//
// Why direct REST: per research.md §6, the supabase-rs SDK has limited
// auth-flow coverage and pulls a websocket realtime client we don't need.
// Direct REST + JWT is narrower (four endpoints, no realtime) and avoids
// adding a heavy dep for a four-command surface.
//
// Token storage: per Decision §6, the JS-era localStorage path
// (`sb-<project-ref>-auth-token`) moves Rust-side. The single-file
// approach matches the existing data layout (`session.json`,
// `tasks.json`, `tags.json`) and keeps `supabase_sign_out` a one-line
// `fs::remove_file` call.

// Lint allowance rationale — `clippy::redundant_pub_crate`: this
// module is `mod auth;` (private) at `lib.rs`, but every item below is
// referenced from `lib.rs` (the four `supabase_*` Tauri commands return
// `auth::AuthSession` and call `auth::sign_in_with_password` etc.). To
// be visible from the parent we must declare `pub(super)` (which the
// lint conflates with `pub(crate)`); making the module itself
// `pub(crate)` and the items plain `pub` then trips the *opposite* lint
// `unreachable_pub` because the `pub(crate) mod` doesn't escape the
// crate. The lints disagree about which form is correct here. We pick
// the same resolution `helpers.rs` already uses (allow-list the
// `redundant_pub_crate` complaint at the items' level), and document
// it once at the module level rather than scattering eight per-item
// `#[allow]` annotations. Spec 001 plan.md §III pedantic-deny is
// upheld because every other clippy lint is honoured.
#![allow(clippy::redundant_pub_crate)]

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::BridgeError;

// Supabase project URL + anon key. Lifted from the JS `src/utils/supabase.js`
// file (the URL and anon key are intentionally public — the anon key is
// the public-facing JWT that gates RLS on the database, not a secret).
// Source: src/utils/supabase.js:4 (URL) and src/utils/supabase.js:39-40
// (anon key). Kept as a `const` here because Phase 1D ships before the
// env-var-driven config refactor; a later phase can lift these into
// `tauri.conf.json` if needed.
const SUPABASE_URL: &str = "https://lopgwwppinkqvttozqfx.supabase.co";
const SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImxvcGd3d3BwaW5rcXZ0dG96cWZ4Iiwicm9sZSI6ImFub24iLCJpYXQiOjE3NTA2NzgxMDIsImV4cCI6MjA2NjI1NDEwMn0.DqPcwsBJdPeV5iWsMkZLMn6-xZ_A9l-Xh7R-wi7kc2k";
const SESSION_FILENAME: &str = "supabase-session.json";

/// Supabase auth user — embedded in [`AuthSession`].
///
/// `user_metadata` is a [`serde_json::Value`] (open shape) so apps can
/// carry arbitrary OAuth-provider claims without a closed-shape
/// migration. Mirrors the Leptos-side `bridge::types::AuthUser` (in the
/// `presto-web` crate) byte-for-byte on the wire (`snake_case` via
/// serde defaults).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct AuthUser {
    pub id: String,
    pub email: String,
    pub user_metadata: serde_json::Value,
}

/// Supabase auth session — returned by sign-in / refresh, persisted
/// to the app-data dir, read back by `supabase_get_session`.
///
/// Mirrors `bridge::types::AuthSession` on the Leptos side. `snake_case`
/// JSON via serde defaults so the Supabase REST `/auth/v1/token`
/// response deserialises directly into this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub user: AuthUser,
}

/// Supabase REST `/auth/v1/token` request body (`grant_type=password`).
#[derive(Serialize)]
struct PasswordGrantBody<'a> {
    email: &'a str,
    password: &'a str,
}

/// Supabase REST `/auth/v1/token` request body (`grant_type=refresh_token`).
#[derive(Serialize)]
struct RefreshGrantBody<'a> {
    refresh_token: &'a str,
}

/// Build the JSON request body and POST it to the Supabase token endpoint.
///
/// `grant_type` is either `"password"` or `"refresh_token"`; the body
/// shape differs per grant per Supabase REST docs but the response
/// shape is the same [`AuthSession`]-compatible JSON.
///
/// `B: Sync` is required so the resulting future is `Send` (clippy's
/// `future_not_send` lint at the workspace level otherwise rejects the
/// monomorphisation; the closure that holds `&body` across the `await`
/// requires `B` to be `Sync`).
async fn post_token<B: Serialize + Sync + ?Sized>(
    grant_type: &str,
    body: &B,
) -> Result<AuthSession, BridgeError> {
    let url = format!("{SUPABASE_URL}/auth/v1/token?grant_type={grant_type}");
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| BridgeError::Internal {
            msg: format!("Supabase REST request failed: {e}"),
        })?;
    let status = response.status();
    if !status.is_success() {
        // Distinguish 400/401 (bad creds / expired token) from 5xx.
        let body_text = response.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::BAD_REQUEST
            || status == reqwest::StatusCode::UNAUTHORIZED
        {
            return Err(BridgeError::InvalidArgument {
                field: "credentials".to_string(),
                reason: format!("Supabase rejected the request ({status}): {body_text}"),
            });
        }
        return Err(BridgeError::Internal {
            msg: format!("Supabase REST returned {status}: {body_text}"),
        });
    }
    response
        .json::<AuthSession>()
        .await
        .map_err(|e| BridgeError::SerdeRoundtrip {
            command: "supabase_token".to_string(),
            error: format!("decode AuthSession: {e}"),
        })
}

/// Persist `session` to `<app_data_dir>/supabase-session.json`. Idempotent
/// (overwrites any existing file). Used by sign-in + refresh.
pub(super) fn persist_session(
    app_data_dir: &Path,
    session: &AuthSession,
) -> Result<(), BridgeError> {
    std::fs::create_dir_all(app_data_dir).map_err(|e| BridgeError::Internal {
        msg: format!("create app data dir: {e}"),
    })?;
    let path = app_data_dir.join(SESSION_FILENAME);
    let bytes = serde_json::to_vec_pretty(session).map_err(|e| BridgeError::Internal {
        msg: format!("serialise session: {e}"),
    })?;
    std::fs::write(&path, bytes).map_err(|e| BridgeError::Internal {
        msg: format!("write {}: {e}", path.display()),
    })
}

/// Read the persisted session, returning `None` for the cold-start
/// (no-file) case. A malformed file is treated as `Internal` (the
/// admin-side fix is to remove the file; the wrapper does not silently
/// drop a corrupted record).
pub(super) fn read_session(app_data_dir: &Path) -> Result<Option<AuthSession>, BridgeError> {
    let path = app_data_dir.join(SESSION_FILENAME);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(BridgeError::Internal {
                msg: format!("read {}: {e}", path.display()),
            });
        }
    };
    let session: AuthSession =
        serde_json::from_slice(&bytes).map_err(|e| BridgeError::SerdeRoundtrip {
            command: "supabase_get_session".to_string(),
            error: format!("decode session: {e}"),
        })?;
    Ok(Some(session))
}

/// Remove the persisted session file. Idempotent: a `NotFound` error is
/// silently absorbed (sign-out when no session exists is a no-op, not an
/// error — matches the JS-era behaviour).
pub(super) fn clear_session(app_data_dir: &Path) -> Result<(), BridgeError> {
    let path = app_data_dir.join(SESSION_FILENAME);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(BridgeError::Internal {
            msg: format!("remove {}: {e}", path.display()),
        }),
    }
}

/// REST: POST `/auth/v1/token?grant_type=password`.
pub(super) async fn sign_in_with_password(
    email: &str,
    password: &str,
) -> Result<AuthSession, BridgeError> {
    if email.is_empty() {
        return Err(BridgeError::InvalidArgument {
            field: "email".to_string(),
            reason: "email is empty".to_string(),
        });
    }
    if password.is_empty() {
        return Err(BridgeError::InvalidArgument {
            field: "password".to_string(),
            reason: "password is empty".to_string(),
        });
    }
    post_token("password", &PasswordGrantBody { email, password }).await
}

/// REST: POST `/auth/v1/logout` (requires `refresh_token` in the
/// `Authorization` header per Supabase docs). Network failures are
/// tolerated — we still clear the local persisted session so the user
/// is signed out client-side even if the server-side revocation
/// roundtrip fails.
pub(super) async fn sign_out(refresh_token: &str) -> Result<(), BridgeError> {
    if refresh_token.is_empty() {
        return Err(BridgeError::InvalidArgument {
            field: "refresh_token".to_string(),
            reason: "refresh_token is empty".to_string(),
        });
    }
    let url = format!("{SUPABASE_URL}/auth/v1/logout");
    let client = reqwest::Client::new();
    // Best-effort: log network failure but do not fail the command. The
    // local session is cleared by the caller regardless.
    let _ = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {refresh_token}"))
        .send()
        .await;
    Ok(())
}

/// REST: POST `/auth/v1/token?grant_type=refresh_token`.
pub(super) async fn refresh_session(refresh_token: &str) -> Result<AuthSession, BridgeError> {
    if refresh_token.is_empty() {
        return Err(BridgeError::InvalidArgument {
            field: "refresh_token".to_string(),
            reason: "refresh_token is empty".to_string(),
        });
    }
    post_token("refresh_token", &RefreshGrantBody { refresh_token }).await
}

#[cfg(test)]
mod tests {
    use super::{AuthSession, AuthUser};
    use tempfile::tempdir;

    fn sample() -> AuthSession {
        AuthSession {
            access_token: "tok".to_string(),
            refresh_token: "rt".to_string(),
            user: AuthUser {
                id: "uid".to_string(),
                email: "u@e.com".to_string(),
                user_metadata: serde_json::json!({"full_name": "U"}),
            },
        }
    }

    #[test]
    fn auth_session_round_trips_snake_case_on_wire() {
        // Pins the wire shape against the supabase-js `/auth/v1/token`
        // response. Drift fails this test loud rather than silently
        // breaking the cross-bridge contract.
        let s = sample();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains(r#""access_token":"tok""#));
        assert!(json.contains(r#""refresh_token":"rt""#));
        assert!(json.contains(r#""email":"u@e.com""#));
        let decoded: AuthSession = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.access_token, "tok");
        assert_eq!(decoded.user.id, "uid");
    }

    #[test]
    fn persist_then_read_round_trips_session() {
        let dir = tempdir().unwrap();
        let s = sample();
        super::persist_session(dir.path(), &s).unwrap();
        let read = super::read_session(dir.path()).unwrap().unwrap();
        assert_eq!(read.access_token, "tok");
        assert_eq!(read.user.email, "u@e.com");
    }

    #[test]
    fn read_session_returns_none_when_file_missing() {
        let dir = tempdir().unwrap();
        let read = super::read_session(dir.path()).unwrap();
        assert!(read.is_none());
    }

    #[test]
    fn clear_session_is_idempotent_when_file_missing() {
        let dir = tempdir().unwrap();
        // First call: file does not exist; absorbed.
        super::clear_session(dir.path()).unwrap();
        // Persist then clear.
        super::persist_session(dir.path(), &sample()).unwrap();
        super::clear_session(dir.path()).unwrap();
        // After clear the read should be None again.
        assert!(super::read_session(dir.path()).unwrap().is_none());
    }
}
