#!/usr/bin/env bash
# Baseline-cap CI gate (spec 001-leptos-migration T244).
#
# Asserts that no more than 2 visual-regression baseline PNGs change in a single
# PR. The 14 PNGs under `tests/e2e/__screenshots__/visual-regression/*.png` are
# the user-facing UI contract per Constitution Principle IV; re-captures during
# normal feature work are budgeted at 0 by default, with up to 2 allowed when
# explicitly justified in the PR. Anything above 2 is a constitution-amendment
# discussion, not a one-PR decision.
#
# Mechanism: diff the working branch against the merge base with `origin/main`
# (or the configured base ref) and count PNG path changes (added | modified |
# deleted | renamed) under the baseline directory. The gate fails-closed when
# the count exceeds the cap.
#
# Configuration:
#   BASELINE_BASE_REF   — git ref to diff against (default: origin/main).
#   BASELINE_CAP        — max permitted baseline changes (default: 2).
#
# Exit codes:
#   0 — count <= cap.
#   1 — count > cap (drift).
#   2 — usage / environment error (base ref unreachable).
#
# Usage: scripts/check-baseline-cap.sh
#   (run from repo root or any subdirectory; resolves paths off this script's
#    location)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

BASE_REF="${BASELINE_BASE_REF:-origin/main}"
CAP="${BASELINE_CAP:-2}"

# Validate cap is a non-negative integer.
if ! [[ "${CAP}" =~ ^[0-9]+$ ]]; then
  echo "check-baseline-cap: BASELINE_CAP must be a non-negative integer, got: ${CAP}" >&2
  exit 2
fi

# Confirm the base ref is resolvable. In CI, `actions/checkout@v4` with
# `fetch-depth: 0` makes origin/main visible; locally, a fresh clone may need
# `git fetch origin main` first. We surface a usage error rather than silently
# pass when the ref is missing — the gate is fail-closed by design.
if ! git rev-parse --verify "${BASE_REF}" >/dev/null 2>&1; then
  echo "check-baseline-cap: base ref '${BASE_REF}' is not resolvable." >&2
  echo "check-baseline-cap: in CI, ensure actions/checkout uses fetch-depth: 0." >&2
  echo "check-baseline-cap: locally, run: git fetch origin main" >&2
  exit 2
fi

# Use the merge-base form (`A...B`) so renames between unrelated commits on the
# base branch don't pollute the diff. `--diff-filter=AMDR` covers added,
# modified, deleted, and renamed entries — every kind of "changed baseline".
# `-z` + `tr` keeps filenames with spaces intact, though baseline filenames
# don't contain spaces today.
changed_pngs=$(
  git diff --name-only --diff-filter=AMDR "${BASE_REF}...HEAD" -- \
    'tests/e2e/__screenshots__/visual-regression/*.png' \
  | sed '/^$/d' \
  || true
)

if [[ -z "${changed_pngs}" ]]; then
  count=0
else
  count=$(printf '%s\n' "${changed_pngs}" | wc -l | tr -d ' ')
fi

if [[ "${count}" -gt "${CAP}" ]]; then
  echo "ERROR: ${count} baseline PNGs changed (max ${CAP} per PR; see Constitution Principle IV and specs/001-leptos-migration/plan.md §CI gates)." >&2
  echo "Changed baselines:" >&2
  while IFS= read -r path; do
    [[ -z "${path}" ]] && continue
    echo "  - ${path}" >&2
  done <<< "${changed_pngs}"
  exit 1
fi

echo "check-baseline-cap: OK (${count} of ${CAP} baseline changes vs ${BASE_REF})"
exit 0
