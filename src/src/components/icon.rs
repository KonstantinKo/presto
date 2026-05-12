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
    #[allow(
        clippy::missing_const_for_fn,
        reason = "RED-state stub (T004); GREEN commit (T005) replaces the body with a strip-prefix parser that is not const-fn-eligible (allocates Strings on every branch)."
    )]
    pub fn from_icon_name(_name: &str) -> Self {
        // RED-state stub: returns a sentinel value that fails most
        // test cases. The GREEN commit (T005) replaces this with the
        // real strip-prefix parser.
        Self::Glyph(String::new())
    }
}

/// Render an `IconClass` to a Leptos view per the contract:
/// - `Remix(s)` → `<i class="ri-{s}"></i>`
/// - `Phosphor(s)` → `<i class="ph ph-{s}"></i>` (both classes)
/// - `Glyph(g)` → `<i>{g}</i>` (text content)
#[must_use]
pub fn render(_class: &IconClass) -> impl IntoView {
    // RED-state stub: emits an empty `<i></i>` so test cases that
    // compare against a non-empty rendered DOM fail. The GREEN
    // commit (T005) replaces this with an exhaustive match.
    view! { <i></i> }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use super::render;
    use super::IconClass;

    // Host-side targets exercise the parser only; the render branch
    // tests run on the wasm-bindgen-test target where Leptos owns a
    // real `Document` to mount into.
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
    // The render assertions exercise the rendered-string projection
    // of the Leptos view. We render to a transient host element on
    // the wasm32 target and inspect its `outerHTML`.

    #[cfg(target_arch = "wasm32")]
    fn render_to_string(class: &IconClass) -> String {
        use leptos::tachys::dom::body;
        use wasm_bindgen::JsCast;
        use web_sys::HtmlElement;

        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("document available in wasm-bindgen-test runtime");
        let host = document
            .create_element("div")
            .expect("create_element")
            .dyn_into::<HtmlElement>()
            .expect("HtmlElement");
        let _ = body().append_child(&host);
        // Mount the view fragment into the host element so the
        // rendered DOM is observable.
        let _unmount = leptos::mount::mount_to(host.clone().unchecked_into(), || render(class));
        let html = host.inner_html();
        let _ = host.remove();
        html
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn render_remix_emits_i_with_ri_class() {
        let html = render_to_string(&IconClass::Remix("brain-line".to_string()));
        assert!(
            html.contains("ri-brain-line"),
            "expected ri-brain-line class in rendered HTML: {html}",
        );
        assert!(html.contains("<i"), "expected <i> element: {html}");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn render_phosphor_emits_i_with_ph_wrapper_and_glyph() {
        let html = render_to_string(&IconClass::Phosphor("cloud".to_string()));
        // BOTH the `ph` wrapper class AND the `ph-cloud` glyph class
        // are required per Contract 1 Tests case 6.
        assert!(
            html.contains("ph-cloud"),
            "expected ph-cloud class in rendered HTML: {html}",
        );
        assert!(
            html.contains("ph ")
                || html.contains(" ph ")
                || html.contains("\"ph ")
                || html.contains("class=\"ph "),
            "expected `ph` wrapper class in rendered HTML: {html}",
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn render_glyph_emits_text_content() {
        let html = render_to_string(&IconClass::Glyph("\u{1f9e0}".to_string()));
        assert!(
            html.contains("\u{1f9e0}"),
            "expected raw grapheme in rendered HTML: {html}",
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn render_glyph_empty_emits_empty_i() {
        let html = render_to_string(&IconClass::Glyph(String::new()));
        // Empty payload produces an `<i></i>` with no children.
        // Allow either `<i></i>` or `<i />` self-closing form.
        let trimmed = html.trim();
        assert!(
            trimmed == "<i></i>" || trimmed == "<i/>" || trimmed == "<i />",
            "expected empty <i> element, got: {html}",
        );
    }
}
