#!/usr/bin/env bash
# Hardcoded-English gate for Leptos `view! {}` blocks. Reject any
# inline English text node — a literal `>"Word..."<` shape between
# JSX-style tags — in `src/src/components/`. This is the regression
# that surfaced during feature 005 (stats view shipped Batch 2 with
# every label hardcoded in English because the spec's scope only
# listed a subset of files).
#
# All user-visible UI strings should route through
# `t!(i18n, ...)` or `t_string!(i18n, ...)` so the four-locale catalogue
# stays the single source of truth.
#
# Exemptions:
# - `#[cfg(test)]` test modules and `mod tests { ... }` (English
#   string fixtures are normal in tests; greps that span them produce
#   false positives).
# - Lines tagged with `// i18n-exempt` (language-picker self-names
#   like `"Deutsch"` for `<option value="de">`; intentionally NOT
#   localised because the dropdown shows each language in its own
#   tongue regardless of the active locale).
# - `target/` build artefacts.
#
# Detection shape:
#   `>"<Capitalised word>...<"<`  — typical text-node literal.
# This DOES NOT catch attribute strings (`title="..."` etc.) which
# the macro is happy to pass through to the DOM. Those are still
# reviewed in PR; the gate's purpose is to catch the silent class
# of inline-text regressions.

set -euo pipefail

ROOT="${1:-$(git rev-parse --show-toplevel)}"
COMPONENTS_DIR="$ROOT/src/src/components"

if [ ! -d "$COMPONENTS_DIR" ]; then
    echo "no components directory at $COMPONENTS_DIR — skipping gate"
    exit 0
fi

# Build a list of `.rs` files under components/, strip out anything
# inside a `#[cfg(test)]` / `mod tests` block, then grep for the
# inline-text shape. Awk handles the test-block stripping so the
# subsequent grep is fed only production view code.
violations=$(find "$COMPONENTS_DIR" -name '*.rs' -type f -print0 \
    | xargs -0 awk '
        BEGIN { skip = 0; depth = 0 }
        # Enter a test-only block.
        /^[[:space:]]*#\[cfg\(test\)\]/ { skip = 1; next }
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
        # When `#[cfg(test)]` precedes a fn that does not open a
        # block on the same line, we approximate by re-arming on
        # the next `}` at column 1.
        skip && depth == 0 && /^}/ { skip = 0; next }
        skip { next }
        { print FILENAME ":" FNR ":" $0 }
    ' \
    | grep -nE '>"[A-Z][a-zA-Z][a-zA-Z]+( [A-Z]?[a-z]+)*[.!?:]?"<' \
    | grep -v 'i18n-exempt' || true)

if [ -n "$violations" ]; then
    echo "FAIL: hardcoded English text nodes found in Leptos view blocks." >&2
    echo "Route the string through \`t!(i18n, ...)\` or \`t_string!(i18n, ...)\`." >&2
    echo "$violations" >&2
    exit 1
fi

echo "OK: no hardcoded English text nodes in src/src/components/ view blocks"
