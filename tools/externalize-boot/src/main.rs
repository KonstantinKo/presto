// Trunk post-build hook (Phase 4e R-001 fix) — extract the inline
// `<script type="module">` Trunk emits in `dist/index.html` into a
// separate `dist/boot.js` file, then rewrite the HTML to load it via
// `<script type="module" src="/boot.js"></script>`.
//
// Why this exists: Trunk's `rel="rust"` directive emits a per-build
// inline boot script whose sha-384 hash drifts every build (the body
// embeds the wasm-bindgen JS filename which itself carries a content
// hash). Pinning that hash in the production CSP at
// `src-tauri/tauri.conf.json` is unmaintainable; pinning a stale hash
// breaks the release WebView at boot. By emitting the script as an
// external file, the CSP can use `script-src 'self'` (plus
// `'wasm-unsafe-eval'` for WASM compilation, which is required by
// every browser regardless of the script source).
//
// The hook reads `TRUNK_STAGING_DIR/index.html`, finds the FIRST
// `<script type="module">…</script>` block, writes the contents to
// `boot.js` in the same directory, and rewrites the tag to a
// `src=/boot.js` reference. Idempotent: if no inline script is found
// the hook is a no-op (covers the case where Trunk's output shape
// changes in a future version and the inline script is already an
// external file).
//
// Spec 001-leptos-migration §Phase 4e R-001.
//
// Lint allowance — `clippy::print_stderr` / `clippy::print_stdout` are
// workspace-denied for production code; build tools that run under a
// Trunk hook surface progress over stdio by convention (matching the
// sibling `presto-build-themes` binary). The Trunk hook captures and
// forwards stdio to the developer terminal; no alternative logger is
// wired into this single-file binary.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

const INDEX_FILE: &str = "index.html";
const BOOT_FILE: &str = "boot.js";
const SCRIPT_OPEN: &str = "<script type=\"module\">";
const SCRIPT_CLOSE: &str = "</script>";

/// Outcome of inspecting an HTML document for an inline boot script.
enum SplitOutcome {
    /// No inline boot script found — the HTML is already in the
    /// post-cutover external-boot shape (or Trunk's output shape
    /// changed in a future version and we don't recognise it).
    NoInlineScript,
    /// Inline script found and split into:
    /// - `body`: the script body to write to `boot.js`
    /// - `rewritten_html`: the HTML with the inline script replaced
    ///   by a `<script src="/boot.js">` reference.
    Split {
        body: String,
        rewritten_html: String,
    },
    /// Recoverable error: an opening `<script type="module">` was
    /// found but no matching closing `</script>` followed it. Caller
    /// should bail out with a diagnostic.
    UnclosedScript,
}

/// Pure splitter — find the first `<script type="module">…</script>`
/// pair in `html`, return either the no-op outcome (no inline script
/// present) or the rewritten-HTML + extracted-body pair.
///
/// Pure so the unit tests can exercise the parsing without touching
/// the filesystem.
fn split_inline_boot(html: &str, boot_file: &str) -> SplitOutcome {
    let Some(open_idx) = html.find(SCRIPT_OPEN) else {
        return SplitOutcome::NoInlineScript;
    };
    let body_start = open_idx + SCRIPT_OPEN.len();
    let Some(close_offset) = html[body_start..].find(SCRIPT_CLOSE) else {
        return SplitOutcome::UnclosedScript;
    };
    let close_idx = body_start + close_offset;
    let after_close = close_idx + SCRIPT_CLOSE.len();

    let body = html[body_start..close_idx].trim().to_string();

    let mut rewritten_html = String::with_capacity(html.len() + 64);
    rewritten_html.push_str(&html[..open_idx]);
    // `write!` into a `String` is infallible — `String`'s `Write`
    // impl never returns an error — so the `Result` is intentionally
    // ignored.
    let _ = write!(
        rewritten_html,
        "<script type=\"module\" src=\"/{boot_file}\"></script>"
    );
    rewritten_html.push_str(&html[after_close..]);

    SplitOutcome::Split {
        body,
        rewritten_html,
    }
}

fn main() -> ExitCode {
    // Trunk passes the staging dir via env per
    // `guide/src/build/hooks.md`. Fall back to `dist/` for direct
    // invocation (manual testing); the hook is configured to run at
    // `post_build` stage so the staging dir is the authoritative path.
    let staging_dir =
        env::var_os("TRUNK_STAGING_DIR").map_or_else(|| PathBuf::from("dist"), PathBuf::from);

    let index_path = staging_dir.join(INDEX_FILE);
    let boot_path = staging_dir.join(BOOT_FILE);

    let html = match fs::read_to_string(&index_path) {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!(
                "presto-externalize-boot: read {} failed: {e}",
                index_path.display()
            );
            return ExitCode::FAILURE;
        }
    };

    match split_inline_boot(&html, BOOT_FILE) {
        SplitOutcome::NoInlineScript => {
            // Already external — no-op success.
            ExitCode::SUCCESS
        }
        SplitOutcome::UnclosedScript => {
            eprintln!(
                "presto-externalize-boot: found `<script type=\"module\">` open with no closing `</script>` in {}",
                index_path.display()
            );
            ExitCode::FAILURE
        }
        SplitOutcome::Split {
            body,
            rewritten_html,
        } => {
            if let Err(e) = fs::write(&boot_path, format!("{body}\n")) {
                eprintln!(
                    "presto-externalize-boot: write {} failed: {e}",
                    boot_path.display()
                );
                return ExitCode::FAILURE;
            }
            if let Err(e) = fs::write(&index_path, rewritten_html) {
                eprintln!(
                    "presto-externalize-boot: rewrite {} failed: {e}",
                    index_path.display()
                );
                return ExitCode::FAILURE;
            }
            eprintln!(
                "presto-externalize-boot: extracted inline boot script ({} bytes) -> {}",
                body.len(),
                boot_path.display()
            );
            ExitCode::SUCCESS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{split_inline_boot, SplitOutcome};

    /// Trunk's typical output shape: a `<script type="module">…</script>`
    /// block in the head, followed by the modulepreload `<link>` tags.
    /// The hook must extract the body and replace the inline script
    /// with a `<script src="/boot.js">` tag.
    #[test]
    fn splits_inline_module_script() {
        let html = r#"<!doctype html>
<html><head><title>x</title>
<script type="module">
import init from '/x.js';
const wasm = await init();
</script>
<link rel="modulepreload" href="/x.js">
</head><body></body></html>"#;
        match split_inline_boot(html, "boot.js") {
            SplitOutcome::Split {
                body,
                rewritten_html,
            } => {
                assert!(body.contains("import init"));
                assert!(body.contains("const wasm = await init()"));
                assert!(!rewritten_html.contains("import init"));
                assert!(rewritten_html.contains(r#"<script type="module" src="/boot.js"></script>"#));
                assert!(rewritten_html.contains("modulepreload"));
            }
            SplitOutcome::NoInlineScript => panic!("expected Split, got NoInlineScript"),
            SplitOutcome::UnclosedScript => panic!("expected Split, got UnclosedScript"),
        }
    }

    /// Idempotent path: if the input HTML already carries an external
    /// `<script src="/boot.js">` reference (post-rewrite re-run), the
    /// splitter must report `NoInlineScript` so the hook is a no-op.
    #[test]
    fn no_op_when_already_external() {
        let html = r#"<!doctype html>
<html><head>
<script type="module" src="/boot.js"></script>
</head><body></body></html>"#;
        assert!(matches!(
            split_inline_boot(html, "boot.js"),
            SplitOutcome::NoInlineScript,
        ));
    }

    /// HTML with NO `<script>` at all — also a no-op success.
    #[test]
    fn no_op_when_no_script_at_all() {
        let html = "<!doctype html><html><head></head><body></body></html>";
        assert!(matches!(
            split_inline_boot(html, "boot.js"),
            SplitOutcome::NoInlineScript,
        ));
    }

    /// Unclosed `<script type="module">` — recoverable error path.
    #[test]
    fn unclosed_script_is_an_error() {
        let html = r#"<!doctype html><html><head>
<script type="module">
import init from '/x.js';
"#;
        assert!(matches!(
            split_inline_boot(html, "boot.js"),
            SplitOutcome::UnclosedScript,
        ));
    }
}
