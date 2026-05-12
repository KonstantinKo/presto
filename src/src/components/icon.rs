// Typed-prefix icon dispatch — Bundle C of feature 003.
//
// Replaces the ad-hoc `name.starts_with("ri-")` chains scattered
// across `components::timer`'s tag-rendering callsites with a closed
// sum type (`IconClass`) so the renderer's dispatch is exhaustive
// (Principle III — Closed Domains). The boundary parser
// (`from_icon_name`) is the only `&str` → `IconClass` projection in
// the app; downstream code matches on the enum.
//
// Contract: `specs/003-stats-redesign/contracts/components.md`
// §Contract 1. RED-first per FR-025.

use leptos::prelude::*;

/// Closed sum type for the three icon-rendering modes presto
/// supports.
///
/// - `Remix(suffix)` — remixicon webfont glyph, e.g. `Remix("brain-line")`
///   from input `"ri-brain-line"`. Renders as `<i class="ri-{suffix}">`.
/// - `Phosphor(suffix)` — Phosphor webfont glyph, e.g.
///   `Phosphor("cloud")` from input `"ph-cloud"`. Renders as
///   `<i class="ph ph-{suffix}">`. The outer `ph` wrapper class is
///   required for the `@font-face` to bind.
/// - `Glyph(grapheme)` — raw text content (e.g. legacy emoji icons,
///   or a corrupt/empty record). Renders as `<i>{grapheme}</i>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconClass {
    /// Remixicon class form. Payload is the glyph suffix
    /// (e.g. `"brain-line"` from `"ri-brain-line"`).
    Remix(String),
    /// Phosphor class form. Payload is the glyph suffix
    /// (e.g. `"cloud"` from `"ph-cloud"`).
    Phosphor(String),
    /// Raw grapheme fallback (legacy emoji icons, empty inputs,
    /// un-prefixed strings). Payload is rendered as text content.
    Glyph(String),
}

impl IconClass {
    /// Parse an icon-name string into the typed dispatch enum.
    ///
    /// Total over `&str`; never panics. Edge cases per
    /// `contracts/components.md` Contract 1:
    /// - `""` → `Glyph("")`
    /// - `"ri-"` / `"ph-"` (prefix-only) → `Glyph(input)`
    /// - `"phone"` (un-dashed `ph` prefix) → `Glyph(input)`
    /// - `" ri-foo"` (leading whitespace) → `Glyph(input)` (no trim)
    #[must_use]
    pub fn from_icon_name(name: &str) -> Self {
        // Require a non-empty suffix after the prefix; `"ri-"` /
        // `"ph-"` alone are data corruption and surface visibly via
        // the Glyph branch rather than rendering as silent empty
        // glyphs.
        if let Some(suffix) = name.strip_prefix("ri-") {
            if !suffix.is_empty() {
                return Self::Remix(suffix.to_string());
            }
        }
        if let Some(suffix) = name.strip_prefix("ph-") {
            if !suffix.is_empty() {
                return Self::Phosphor(suffix.to_string());
            }
        }
        Self::Glyph(name.to_string())
    }
}

/// Materialised render-decision for an `IconClass`.
///
/// Exposed so the renderer's contract (Remix → `<i class="ri-{s}">`,
/// Phosphor → `<i class="ph ph-{s}">`, Glyph → text content) is
/// host-side testable without mounting a Leptos view. The `render`
/// view function below is a one-line shim over this decision; tests
/// pin the decision directly so the four render branches don't need
/// a DOM runtime to verify.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderSpec {
    /// `<i class={class}></i>`, no text content.
    ElementWithClass(String),
    /// `<i>{text}</i>`, no class attribute.
    ElementWithText(String),
}

impl IconClass {
    /// Compute the render decision per Contract 1:
    /// - `Remix(s)` → `ElementWithClass("ri-{s}")`
    /// - `Phosphor(s)` → `ElementWithClass("ph ph-{s}")`
    /// - `Glyph(g)` → `ElementWithText(g)` (text content; may be empty)
    #[must_use]
    pub fn render_spec(&self) -> RenderSpec {
        match self {
            Self::Remix(suffix) => RenderSpec::ElementWithClass(format!("ri-{suffix}")),
            Self::Phosphor(suffix) => RenderSpec::ElementWithClass(format!("ph ph-{suffix}")),
            Self::Glyph(grapheme) => RenderSpec::ElementWithText(grapheme.clone()),
        }
    }
}

/// Render an `IconClass` to a Leptos view per the contract:
/// - `Remix(s)` → `<i class="ri-{s}"></i>`
/// - `Phosphor(s)` → `<i class="ph ph-{s}"></i>` (both classes)
/// - `Glyph(g)` → `<i>{g}</i>` (text content)
#[must_use]
pub fn render(class: &IconClass) -> impl IntoView {
    match class.render_spec() {
        RenderSpec::ElementWithClass(cls) => view! { <i class=cls></i> }.into_any(),
        RenderSpec::ElementWithText(text) => view! { <i>{text}</i> }.into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::{IconClass, RenderSpec};

    // Both targets exercise parser + render-spec via the typed enum;
    // mounting an actual Leptos view is the e2e suite's job, not the
    // unit-test layer (no `Document` is available under
    // `wasm-pack test --node`).
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    // ---------- Parser branches (4 canonical) ----------

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn parser_remix_branch() {
        assert_eq!(
            IconClass::from_icon_name("ri-brain-line"),
            IconClass::Remix("brain-line".to_string()),
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn parser_phosphor_branch() {
        assert_eq!(
            IconClass::from_icon_name("ph-cloud"),
            IconClass::Phosphor("cloud".to_string()),
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn parser_glyph_branch_emoji() {
        assert_eq!(
            IconClass::from_icon_name("\u{1f9e0}"),
            IconClass::Glyph("\u{1f9e0}".to_string()),
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn parser_glyph_branch_empty() {
        assert_eq!(
            IconClass::from_icon_name(""),
            IconClass::Glyph(String::new())
        );
    }

    // ---------- Parser edge cases (4) ----------

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn parser_remix_prefix_only() {
        // `"ri-"` has no suffix; must fall to `Glyph("ri-")` per
        // contracts/components.md edge-case dispatch table.
        assert_eq!(
            IconClass::from_icon_name("ri-"),
            IconClass::Glyph("ri-".to_string()),
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn parser_phosphor_prefix_only() {
        assert_eq!(
            IconClass::from_icon_name("ph-"),
            IconClass::Glyph("ph-".to_string()),
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn parser_undashed_ph_prefix() {
        // `starts_with("ph-")` requires the dash separator; `"phone"`
        // is a plain glyph.
        assert_eq!(
            IconClass::from_icon_name("phone"),
            IconClass::Glyph("phone".to_string()),
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn parser_leading_whitespace() {
        // The parser does not trim; leading whitespace prevents
        // prefix matching.
        assert_eq!(
            IconClass::from_icon_name(" ri-foo"),
            IconClass::Glyph(" ri-foo".to_string()),
        );
    }

    // ---------- Render branches (4) ----------
    //
    // The render contract is tested via `render_spec` — a pure
    // function projection of the render decision (class string vs.
    // text content). This pins the typed-dispatch contract without
    // requiring a `Document` runtime (`wasm-pack test --node` has no
    // DOM); the Leptos `render` view function is a one-line shim
    // over `render_spec`, so by induction it emits the same DOM.

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn render_remix_emits_i_with_ri_class() {
        assert_eq!(
            IconClass::Remix("brain-line".to_string()).render_spec(),
            RenderSpec::ElementWithClass("ri-brain-line".to_string()),
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn render_phosphor_emits_i_with_ph_wrapper_and_glyph() {
        // BOTH the `ph` wrapper AND the `ph-cloud` glyph class are
        // required per Contract 1 Tests case 6. The class string is
        // `"ph ph-cloud"` (space-separated tokens; ordered for visual
        // parity with the Phosphor docs).
        assert_eq!(
            IconClass::Phosphor("cloud".to_string()).render_spec(),
            RenderSpec::ElementWithClass("ph ph-cloud".to_string()),
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn render_glyph_emits_text_content() {
        assert_eq!(
            IconClass::Glyph("\u{1f9e0}".to_string()).render_spec(),
            RenderSpec::ElementWithText("\u{1f9e0}".to_string()),
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn render_glyph_empty_emits_empty_i() {
        // Empty payload produces an `<i></i>` with no children — the
        // ElementWithText variant with an empty payload.
        assert_eq!(
            IconClass::Glyph(String::new()).render_spec(),
            RenderSpec::ElementWithText(String::new()),
        );
    }
}
