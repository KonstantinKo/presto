// Theme loader — Phase 5 (T223) of spec 001-leptos-migration.
//
// Applies the user-selected theme to the live DOM by setting the
// `data-theme` attribute on the document's root `<html>` element.
// CSS rules in `src/style/themes/*.css` are scoped to
// `html[data-theme="..."]` (mirrors the JS-era pattern at
// `src/managers/theme-manager.js`), so flipping the attribute
// restyles the whole tree without re-mounting components. FR-021
// pin: theme contract preserved post-cutover.
//
// Per Principle I, this is a pure DOM-binding wrapper. The
// `prefers-color-scheme` follow-system hop is in `system_theme`
// below (T224); the manager layer drives which stem to apply.
//
// Module contains both wasm-target DOM writes and a host-side
// mocked test: the body of `apply_theme` is `cfg(target_arch =
// "wasm32")`-gated so `cargo test` (host target) doesn't try to
// link `web_sys::window()`. The host-side stub is a no-op so the
// signature is reachable from manager-layer tests under the host
// target.

use crate::theme::themes;

/// Apply `theme_name` to the live DOM by setting
/// `<html data-theme="...">`. Best-effort: failures (no document —
/// host build / SSR) reduce to a silent no-op, which is fine
/// because the e2e suite always runs against a real DOM.
///
/// Mirrors the JS-era `applyTheme` body at
/// `src/managers/theme-manager.js`. Distinct from the
/// timer-theme attribute (`data-timer-theme`) — that's set by the
/// `ThemeSettings` tile click handler directly because the two
/// attributes have different scopes (color theme is global,
/// timer theme is timer-view-only).
#[cfg(target_arch = "wasm32")]
pub fn apply_theme(theme_name: &str) {
    use wasm_bindgen::JsCast as _;

    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(root) = document.document_element() else {
        return;
    };
    if let Ok(html) = root.dyn_into::<web_sys::HtmlElement>() {
        let _ = html.set_attribute("data-theme", theme_name);
    }
}

/// Host-side stub.
///
/// The binary is wasm-only, but the lib's host tests link against
/// this signature. No-op under the host target so manager-layer
/// code that funnels through `apply_theme` remains test-reachable.
#[cfg(not(target_arch = "wasm32"))]
#[allow(
    clippy::missing_const_for_fn,
    // Cannot be `const fn` because the wasm-target sibling has DOM
    // I/O; signatures must match across cfg variants.
)]
pub fn apply_theme(_theme_name: &str) {}

/// Resolve `theme_name` against the auto-generated catalogue.
///
/// If `theme_name` matches a stem in `themes::ALL_THEMES`, return
/// it unchanged; otherwise fall back to `themes::DEFAULT_THEME`.
/// Used by the manager layer to validate persisted preferences
/// before applying — a stale preference for a theme that was
/// removed from the catalogue silently maps to the default
/// rather than rendering an unstyled page.
#[must_use]
pub fn resolve_theme(theme_name: &str) -> &'static str {
    themes::ALL_THEMES
        .iter()
        .find(|stem| **stem == theme_name)
        .copied()
        .unwrap_or(themes::DEFAULT_THEME)
}

/// Apply the resolved theme.
///
/// Convenience wrapper that pairs `resolve_theme` with
/// `apply_theme`. The manager layer calls this after a settings
/// load or a tile click; the e2e `settings-theme.spec.js` flow
/// asserts on the resolved `data-theme` value rather than the raw
/// click input.
pub fn apply_resolved(theme_name: &str) {
    apply_theme(resolve_theme(theme_name));
}

/// Whether the operating system requests a dark colour scheme.
///
/// Reads `window.matchMedia("(prefers-color-scheme: dark)").matches`
/// — the JS-era follow-system hop in
/// `src/managers/theme-manager.js`. T224 of spec
/// 001-leptos-migration. Returns `false` when the bridge is absent
/// (host build / SSR) — the manager layer treats `false` as
/// "default to light theme" which matches the JS source's fallback
/// branch.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn system_prefers_dark() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    match window.match_media("(prefers-color-scheme: dark)") {
        Ok(Some(query)) => query.matches(),
        _ => false,
    }
}

/// Host-side stub for `system_prefers_dark`.
///
/// Returns `false` so manager-layer tests don't need a wasm
/// runtime. The wasm body reads the live media query.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
#[allow(
    clippy::missing_const_for_fn,
    // Cannot be `const fn` because the wasm-target sibling has DOM
    // I/O; signatures must match across cfg variants.
)]
pub fn system_prefers_dark() -> bool {
    false
}

/// Resolve a settings-level color-mode preference to a concrete
/// `data-theme` token.
///
/// `pref` is the JS-era selector value (`"auto"` / `"light"` /
/// `"dark"`); `system_dark` is the result of `system_prefers_dark`.
/// Returns `"dark"` or `"light"` — the literal token the JS-era
/// `<html data-theme="...">` carries. Unknown `pref` values map to
/// `"light"` (matches the JS-era default branch at
/// `src/managers/theme-manager.js`).
#[must_use]
pub fn resolve_color_mode(pref: &str, system_dark: bool) -> &'static str {
    match pref {
        "dark" => "dark",
        "auto" if system_dark => "dark",
        // "light" and "auto" with !system_dark and any unknown
        // value all map to "light" — collapsed via wildcard so
        // clippy's `match_same_arms` lint is satisfied.
        _ => "light",
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_resolved, apply_theme, resolve_theme};
    use crate::theme::themes;

    /// T223: the loader has an `apply_theme` entrypoint that the
    /// manager layer can call. Host-side this is a no-op stub;
    /// the contract pin is the signature (`fn(&str)`), the
    /// resolve-then-apply flow, and the catalogue-fallback
    /// behaviour. The DOM-write half is exercised by the e2e
    /// suite.
    #[test]
    fn apply_theme_signature_pinned() {
        // Pure signature pin — host stub is a no-op so this only
        // verifies the call type-checks. The wasm-target body sets
        // `<html data-theme="...">` per
        // `src/managers/theme-manager.js`.
        apply_theme("dark");
        apply_theme("light");
    }

    /// `resolve_theme` falls back to `DEFAULT_THEME` for unknown
    /// stems. A persisted preference referencing a since-removed
    /// theme silently maps to the default rather than rendering
    /// unstyled. The auto-generated catalogue's first stem
    /// alphabetically is the default — this test uses the live
    /// catalogue rather than a fixture so a code-gen drift surfaces
    /// here.
    #[test]
    fn resolve_unknown_theme_falls_back_to_default() {
        let resolved = resolve_theme("a-theme-that-cannot-exist-xyz123");
        assert_eq!(resolved, themes::DEFAULT_THEME);
    }

    /// Known stems round-trip unchanged. Iterating the catalogue
    /// rather than hard-coding stems keeps this test stable across
    /// theme additions / removals.
    #[test]
    fn resolve_known_theme_returns_input() {
        for stem in themes::ALL_THEMES {
            assert_eq!(resolve_theme(stem), *stem);
        }
    }

    /// `apply_resolved` is the wrapper the manager layer calls;
    /// it accepts an arbitrary string and either applies the
    /// matching catalogue stem or the default. Host-side it's a
    /// no-op — this test pins the call signature.
    #[test]
    fn apply_resolved_signature_pinned() {
        apply_resolved("dark");
        apply_resolved("a-theme-that-cannot-exist-xyz123");
    }

    /// T224: `resolve_color_mode` maps the JS-era settings-level
    /// preference (`auto` / `light` / `dark`) to a concrete
    /// `data-theme` token. `auto` resolves against the `system_dark`
    /// hint (which the wasm body fills via the
    /// `prefers-color-scheme: dark` media query); `light` and
    /// `dark` round-trip; unknown values default to `light`.
    #[test]
    fn resolve_color_mode_auto_follows_system_hint() {
        assert_eq!(super::resolve_color_mode("auto", true), "dark");
        assert_eq!(super::resolve_color_mode("auto", false), "light");
    }

    #[test]
    fn resolve_color_mode_explicit_overrides_system() {
        assert_eq!(super::resolve_color_mode("dark", false), "dark");
        assert_eq!(super::resolve_color_mode("light", true), "light");
    }

    #[test]
    fn resolve_color_mode_unknown_defaults_to_light() {
        assert_eq!(super::resolve_color_mode("", false), "light");
        assert_eq!(super::resolve_color_mode("nonsense", true), "light");
    }

    /// Host-side `system_prefers_dark` is a no-op stub returning
    /// `false`. Pin the signature so the manager layer's call
    /// remains type-checked.
    #[test]
    fn system_prefers_dark_signature_pinned() {
        let _ = super::system_prefers_dark();
    }
}
