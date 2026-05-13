#!/usr/bin/env bash
# Hardcoded-English gate for Leptos `view! {}` blocks AND
# toast / desktop-notification call sites. Reject any inline English
# string literal that should have gone through the i18n catalogue,
# anywhere under `src/src/` (excluding `engine/` and `tests/`).
#
# Three detection shapes:
#
#   1. `>"<Capitalised word>...<"<` — inline view-text node literal
#      (e.g. `view! { <span>"Hello"</span> }`). The original gate from
#      feature 005; covers the regression class that surfaced in the
#      stats view + degraded-mode banner.
#
#   2. `.show("<Capitalised word>...` — `toast.show("English")` /
#      `app_toast.show("English")`. Quickreview round flagged
#      `src/src/components/timer/mod.rs` for 9+ inline English toasts
#      that bypassed the original gate.
#
#   3. `send_notification(<arg>, "<Capitalised word>...` — Tauri
#      desktop notification body. Same regression class; the
#      `messages.rs` consts that previously held these strings have
#      been folded into the i18n catalogue.
#
# All user-visible UI strings should route through
# `t!(i18n, ...)` or `t_string!(i18n, ...)` so the four-locale catalogue
# stays the single source of truth.
#
# Exemptions:
# - `mod tests { ... }` blocks (English string fixtures are normal in
#   tests; greps that span them produce false positives). The awk
#   skip-state tracks brace depth from `mod tests {` to its matching
#   `}`.
# - Lines tagged with `// i18n-exempt` (language-picker self-names
#   like `"Deutsch"` for `<option value="de">`; intentionally NOT
#   localised because the dropdown shows each language in its own
#   tongue regardless of the active locale).
# - `target/` build artefacts.
# - `src/src/engine/` (no UI strings — pure state machine).
# - `tests/` (test fixtures contain expected English text by design).
#
# Implementation note: the previous version of this gate also had a
# bare `#[cfg(test)]` skip-arm intended to cover test fns at module
# scope, but a single-line `#[cfg(test)] mod tests { ... }` would set
# `skip=1` without entering depth-tracking, leaving production code
# below permanently skipped. Every `#[cfg(test)]` in this codebase is
# followed by `mod tests {` on the next line, so the bare-attr arm has
# been removed; the `mod tests` arm handles every real case.

set -euo pipefail

ROOT="${1:-$(git rev-parse --show-toplevel)}"
SCAN_DIR="$ROOT/src/src"

if [ ! -d "$SCAN_DIR" ]; then
    echo "no scan directory at $SCAN_DIR — skipping gate"
    exit 0
fi

# Build a list of `.rs` files under `src/src/` (excluding `engine/`
# and any `tests/` subtree), strip out anything inside a `mod tests`
# block, then grep for any of the three inline-English shapes. Awk
# handles the test-block stripping so the subsequent grep is fed only
# production code.
violations=$(find "$SCAN_DIR" -name '*.rs' -type f \
        -not -path "$SCAN_DIR/engine/*" \
        -not -path "*/tests/*" \
        -print0 \
    | xargs -0 awk '
        BEGIN { skip = 0; depth = 0 }
        # Enter a `mod tests { ... }` block; track brace depth so the
        # closing `}` (which may sit far below) restores skip=0.
        /^[[:space:]]*mod[[:space:]]+tests[[:space:]]*\{/ { skip = 1; depth = 1; next }
        skip && depth > 0 {
            for (i = 1; i <= length($0); i++) {
                c = substr($0, i, 1)
                if (c == "{") depth++
                else if (c == "}") {
                    depth--
                    if (depth == 0) { skip = 0; break }
                }
            }
            next
        }
        { print FILENAME ":" FNR ":" $0 }
    ' \
    | grep -nE '(>"[A-Z][a-zA-Z][a-zA-Z]+( [A-Z]?[a-z]+)*[.!?:]?"<)|(\.show\("[A-Z])|(send_notification\([^,]+,[[:space:]]*"[A-Z])' \
    | grep -v 'i18n-exempt' \
    | grep -vE ':[[:space:]]*//' || true)

if [ -n "$violations" ]; then
    echo "FAIL: hardcoded English string found in production Rust code." >&2
    echo "Route the string through \`t!(i18n, ...)\` or \`t_string!(i18n, ...)\`." >&2
    echo "Matched shapes: inline view text, .show(\"...\"), send_notification(_, \"...\")." >&2
    echo "$violations" >&2
    exit 1
fi

echo "OK: no hardcoded English text nodes, toast.show calls, or send_notification bodies in src/src/ (engine/ + tests/ exempt)"
