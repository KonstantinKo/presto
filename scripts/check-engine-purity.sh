#!/usr/bin/env bash
# Engine purity grep gate (spec 001-leptos-migration T246).
#
# Asserts that the timer engine module under `src/src/engine/` never imports
# DOM-binding crates (`web_sys` / `web-sys`). Per Constitution Principle I —
# The Timer Is Sacred — the engine is a pure Rust state machine; DOM-sourced
# inputs (activity signals, etc.) enter via the bridge layer through the
# normalised `ActivitySignal` stream, never via direct `web_sys` reads.
#
# This is a mechanical fail-closed grep gate matching plan.md §CI gates verbatim:
#   grep -rE "web_sys|web-sys" src/src/engine/
#
# A match — even one — fails the build. The gate is intentionally
# zero-tolerance; legitimate DOM access belongs in the bridge layer
# (`src/src/bridge/`), not the engine.
#
# Exit codes:
#   0 — no `web_sys` / `web-sys` references anywhere under src/src/engine/.
#   1 — at least one reference found (Principle I violation).
#   2 — usage / environment error (engine directory missing).
#
# Usage: scripts/check-engine-purity.sh
#   (run from repo root or any subdirectory; resolves paths off this script's
#    location)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

ENGINE_DIR="${REPO_ROOT}/src/src/engine"

if [[ ! -d "${ENGINE_DIR}" ]]; then
  echo "check-engine-purity: missing engine directory ${ENGINE_DIR}" >&2
  exit 2
fi

# Match plan.md §CI gates verbatim. We capture matches so we can echo the
# offending file:line on stderr; an empty capture means clean.
# `--include='*.rs'` keeps the gate scoped to Rust source — comments in
# non-Rust files (e.g., a future README) shouldn't matter, but in practice
# only .rs lives under src/src/engine/.
matches=$(grep -rEn --include='*.rs' "web_sys|web-sys" "${ENGINE_DIR}" || true)

if [[ -n "${matches}" ]]; then
  echo "ERROR: engine module references web-sys (Principle I — The Timer Is Sacred)." >&2
  echo "Engine must be a pure Rust state machine; route DOM input through src/src/bridge/." >&2
  echo "Offending references:" >&2
  while IFS= read -r line; do
    [[ -z "${line}" ]] && continue
    echo "  ${line}" >&2
  done <<< "${matches}"
  exit 1
fi

echo "check-engine-purity: OK (no web-sys references under src/src/engine/)"
exit 0
