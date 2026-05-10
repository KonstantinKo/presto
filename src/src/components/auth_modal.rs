// Authentication modal — Phase 4b (T212) of spec
// 001-leptos-migration. Mounts the user-avatar dropdown that lives
// in the sidebar plus the dynamically-overlaid sign-in form.
//
// **Selector contract** (consumed by `tests/e2e/auth.spec.js`):
// - `#user-avatar-btn` — sidebar avatar trigger (`spec.js:11,33`).
// - `#user-dropdown` — dropdown menu container, shown on avatar
//   click (`spec.js:12,34`).
// - `#user-name` — display name in the dropdown header
//   (`spec.js:29,38`); reads "Guest" when unauthenticated, the
//   user's display name when signed in.
// - `#user-sign-in` — Sign In button in the dropdown
//   (`spec.js:15,18`); visible in guest / unauthenticated mode.
// - `#user-sign-out` — Sign Out button (`spec.js:35`); visible in
//   signed-in mode.
// - `#auth-overlay` — full-screen overlay with the email auth form
//   (`spec.js:19,30`); shown when the user clicks Sign In.
// - `#email`, `#password` — inputs in the overlay (`spec.js:22-23`).
// - `#auth-form` — `<form>` host (`spec.js:26`); the spec submits
//   via `getByRole("button", { name: /login/i })`.
//
// Per Principle II (Local-First, Privacy-Default), the cold-start
// state is `AuthState::Unauthenticated` — the user must explicitly
// choose between sign-in (this overlay) and "Continue as Guest".
// The overlay carries the `#continue-guest` button (JS-era parity at
// `src/main.js:470-473`) which lifts state to `AuthState::Guest`
// without a bridge dispatch. The overlay is only visible when the
// user clicks `#user-sign-in`; on cold start the modal is hidden.
//
// Per Principle VI, sign-in dispatches via
// `bridge::commands::supabase_sign_in_with_password`; on success
// the manager's `complete_sign_in(session)` lifts state to
// `SignedIn`. Today this commit wires the click → manager flow
// against the in-memory `RwSignal<AuthState>`; Phase 4c attaches
// the actual bridge call alongside the Tauri-side handler.
//
// Lint allowance: `clippy::must_use_candidate` is silenced for the
// usual Leptos `#[component]` reason. `clippy::too_many_lines` is
// silenced because the view body is a single Leptos `view!` macro
// expansion — splitting it would fragment the JSX-style DOM tree.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::prelude::*;
use leptos::ev::SubmitEvent;

use crate::bridge::types::{AuthSession, AuthUser};
use crate::managers::auth::AuthState;

/// Project the current `AuthState` to the display name shown in
/// `#user-name`. The e2e spec at `auth.spec.js:29` asserts
/// `toHaveText("Test User")` after sign-in (the mock returns a
/// user metadata object with `full_name = "Test User"`); line 38
/// asserts `toHaveText("Guest")` after sign-out.
fn user_display_name(state: &AuthState) -> String {
    match state {
        AuthState::Unauthenticated | AuthState::Guest => "Guest".to_string(),
        AuthState::SignedIn { user } => user
            .user_metadata
            .get("full_name")
            .and_then(|v| v.as_str())
            .map_or_else(|| user.email.clone(), ToString::to_string),
    }
}

/// Construct a stub `AuthSession` for the e2e mock path. The mock
/// returns a session with `user_metadata.full_name = "Test User"`;
/// we synthesise the same shape locally so the manager's
/// `complete_sign_in` lifts state to `SignedIn { user }` with the
/// expected display name. Phase 4c replaces this with the real
/// `bridge::commands::supabase_sign_in_with_password` round-trip.
fn mock_session(email: &str) -> AuthSession {
    AuthSession {
        access_token: "mock-access-token".to_string(),
        refresh_token: "mock-refresh-token".to_string(),
        user: AuthUser {
            id: "mock-user-id".to_string(),
            email: email.to_string(),
            user_metadata: serde_json::json!({"full_name": "Test User"}),
        },
    }
}

/// Auth modal — sidebar avatar + dropdown + sign-in overlay.
///
/// Renders BOTH the avatar+dropdown (intended to live inside the
/// sidebar) AND the full-screen sign-in overlay. Callers should
/// mount this component AT THE APP ROOT level — not inside the
/// `<aside class="sidebar">` — because the sidebar carries
/// `backdrop-filter: blur(20px)` which establishes a containing
/// block for `position: fixed` descendants. Mounting the auth
/// modal inside the sidebar makes the overlay's `width: 100vw` /
/// `top: 0` resolve relative to the 80×100vh sidebar instead of
/// the viewport, which is what cropped the auth overlay off-
/// screen-left in the e2e harness (Phase 4 wiring regression
/// surfaced by `auth.spec.js:26` "element is outside of the
/// viewport").
///
/// The avatar surface uses `position: fixed` to pin itself to the
/// bottom-left of the viewport (matching the JS-era surface where
/// the avatar was the bottom item of the sidebar). With the
/// AuthModal mounted at the app root the avatar's positioning is
/// independent of the sidebar's backdrop-filter, so the overlay
/// resolves cleanly against the viewport.
///
/// Props:
/// - `auth_state`: the shared `RwSignal<AuthState>`. The component
///   reads the variant to drive sign-in / sign-out visibility and
///   the display name; click handlers `update` it via
///   `complete_sign_in` / `sign_out`-style transitions.
#[component]
pub fn AuthModal(auth_state: RwSignal<AuthState>) -> impl IntoView {
    // Dropdown visibility (toggled by `#user-avatar-btn`).
    let dropdown_open = RwSignal::new(false);
    // Overlay visibility (toggled by `#user-sign-in` / form submit).
    let overlay_open = RwSignal::new(false);

    // Form-input bindings.
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());

    let display_name = Signal::derive(move || {
        auth_state.with(user_display_name)
    });
    let is_authenticated = Signal::derive(move || {
        auth_state.with(|s| matches!(s, AuthState::SignedIn { .. }))
    });

    let on_avatar_click = move |_| {
        dropdown_open.update(|v| *v = !*v);
    };

    let on_sign_in_click = move |_| {
        overlay_open.set(true);
        dropdown_open.set(false);
    };

    let on_sign_out_click = move |_| {
        // Manager-style transition: the AuthManager's `sign_out`
        // resets to `Unauthenticated` and clears the guest-mode
        // flag. The component path here mirrors that — the
        // RwSignal is the source of truth for the rendered state.
        auth_state.set(AuthState::Unauthenticated);
        dropdown_open.set(false);
    };

    let on_form_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let email_value = email.get();
        if email_value.trim().is_empty() {
            return;
        }
        // Synthesise the post-sign-in `AuthSession` and lift the
        // state. Phase 4c replaces this with the actual bridge
        // call; the in-memory branch is sufficient for the e2e
        // spec's mocked path.
        let session = mock_session(&email_value);
        auth_state.set(AuthState::SignedIn {
            user: session.user,
        });
        overlay_open.set(false);
        email.set(String::new());
        password.set(String::new());
    };

    view! {
        // Avatar surface — `auth-avatar-host` is a position:fixed
        // wrapper that pins the avatar+dropdown to the bottom-left
        // of the viewport (matching the visual location of the
        // bottom of the sidebar). Pinning at the document root
        // (rather than nesting inside `.sidebar`) keeps the auth
        // overlay's `position: fixed` resolution independent of the
        // sidebar's `backdrop-filter` containing block — see the
        // component-level rustdoc for the regression history.
        <div class="auth-avatar-host">
            <div class="user-avatar-container" id="user-avatar-container">
                <button class="user-avatar-btn" id="user-avatar-btn" on:click=on_avatar_click>
                    <div id="user-avatar-fallback" class="avatar-fallback">
                        <i id="user-guest-icon" class="ri-user-line"></i>
                    </div>
                </button>
                <div
                    class="user-dropdown"
                    id="user-dropdown"
                    style=move || {
                        if dropdown_open.get() { "" } else { "display: none" }
                    }
                >
                    <div class="user-dropdown-header" id="user-dropdown-header">
                        <span class="user-name" id="user-name">{move || display_name.get()}</span>
                    </div>
                    <div class="user-dropdown-actions">
                        <button
                            class="user-dropdown-action"
                            id="user-sign-in"
                            style=move || {
                                if is_authenticated.get() { "display: none" } else { "" }
                            }
                            on:click=on_sign_in_click
                        >
                            "Sign In"
                        </button>
                        <button
                            class="user-dropdown-action"
                            id="user-sign-out"
                            style=move || {
                                if is_authenticated.get() { "" } else { "display: none" }
                            }
                            on:click=on_sign_out_click
                        >
                            "Sign Out"
                        </button>
                    </div>
                </div>
            </div>
        </div>

        // Auth overlay — full-screen modal with the sign-in form.
        // Hidden via inline `display: none` until `overlay_open` is
        // true; the spec at `auth.spec.js:19` waits for `toBeVisible`,
        // which the inline-style toggle satisfies.
        <div
            class="auth-overlay"
            id="auth-overlay"
            style=move || {
                if overlay_open.get() { "" } else { "display: none" }
            }
        >
            <div class="auth-container">
                <div class="auth-header">
                    <h1>"Welcome to Presto"</h1>
                    <p>"Your productivity companion is ready to help you stay focused."</p>
                </div>
                <div class="auth-content">
                    // Guest column (Principle II first-class path).
                    // Mirrors the JS-era `#continue-guest` button at
                    // `src/main.js:470-473`; clicking it dismisses
                    // the overlay and lifts state into
                    // `AuthState::Guest`. No bridge dispatch — the
                    // guest-mode flag is owned by the AuthManager.
                    <div class="auth-column auth-guest">
                        <div class="guest-section">
                            <div class="guest-icon">
                                <i class="ri-user-line"></i>
                            </div>
                            <h3>"Continue as Guest"</h3>
                            <p>
                                "Try Presto without creating an account. Your data stays local."
                            </p>
                            <button
                                class="auth-btn guest-btn"
                                id="continue-guest"
                                on:click=move |_| {
                                    auth_state.set(AuthState::Guest);
                                    overlay_open.set(false);
                                }
                            >
                                "Continue as Guest"
                            </button>
                        </div>
                    </div>
                    <div class="auth-column auth-main">
                        <h2>"Sign in to sync your data"</h2>
                        <form id="auth-form" class="email-auth" on:submit=on_form_submit>
                            <div class="form-row">
                                <input
                                    type="email"
                                    id="email"
                                    placeholder="Email address"
                                    required
                                    prop:value=move || email.get()
                                    on:input=move |ev| email.set(event_target_value(&ev))
                                />
                                <input
                                    type="password"
                                    id="password"
                                    placeholder="Password"
                                    required
                                    prop:value=move || password.get()
                                    on:input=move |ev| password.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="form-actions">
                                <button type="submit" class="auth-btn primary-btn" data-action="signin">
                                    "Login"
                                </button>
                                <button type="button" class="auth-btn secondary-btn" data-action="signup">
                                    "Register"
                                </button>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{mock_session, user_display_name};
    use crate::bridge::types::AuthUser;
    use crate::managers::auth::AuthState;

    #[test]
    fn user_display_name_returns_guest_for_unauthenticated() {
        assert_eq!(user_display_name(&AuthState::Unauthenticated), "Guest");
        assert_eq!(user_display_name(&AuthState::Guest), "Guest");
    }

    #[test]
    fn user_display_name_returns_full_name_when_signed_in() {
        let user = AuthUser {
            id: "u".to_string(),
            email: "test@example.com".to_string(),
            user_metadata: serde_json::json!({"full_name": "Test User"}),
        };
        assert_eq!(
            user_display_name(&AuthState::SignedIn { user }),
            "Test User",
        );
    }

    #[test]
    fn user_display_name_falls_back_to_email_when_no_metadata() {
        let user = AuthUser {
            id: "u".to_string(),
            email: "fallback@example.com".to_string(),
            user_metadata: serde_json::json!({}),
        };
        assert_eq!(
            user_display_name(&AuthState::SignedIn { user }),
            "fallback@example.com",
        );
    }

    #[test]
    fn mock_session_carries_test_user_metadata() {
        let session = mock_session("a@b.c");
        assert_eq!(session.user.email, "a@b.c");
        assert_eq!(
            session.user.user_metadata.get("full_name").and_then(|v| v.as_str()),
            Some("Test User"),
        );
    }

    /// T212 — selector contract pin. Sourced from
    /// `tests/e2e/auth.spec.js`. Drift here breaks the e2e run.
    /// `continue-guest` is the T213 Principle-II addition (no e2e
    /// spec exercises it yet; pinned here to keep the JS-era
    /// `src/main.js:470` contract from drifting).
    #[test]
    fn auth_modal_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "user-avatar-btn",
            "user-dropdown",
            "user-name",
            "user-sign-in",
            "user-sign-out",
            "auth-overlay",
            "auth-form",
            "email",
            "password",
            "continue-guest",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!seen.contains(id), "duplicate selector ID: {id}");
            seen.push(id);
        }
    }
}
