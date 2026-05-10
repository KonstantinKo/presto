#!/usr/bin/env bash
# Lockfile drift gate (spec 001-leptos-migration T248).
#
# Asserts that any change to a manifest file is accompanied by a matching
# lockfile change in the same commit (or PR). Per Constitution Principle IX —
# Lock Files Are First-Class — manifest-without-lock is the single most-common
# CI failure mode on this repo (issue #22 documents the pattern).
#
# Pairs enforced:
#   * Cargo.toml                  ↔ Cargo.lock              (workspace root)
#   * tests/e2e/package.json      ↔ tests/e2e/package-lock.json
#
# We DO NOT enforce member-crate Cargo.toml changes (e.g., `src/Cargo.toml`)
# directly because Cargo regenerates the workspace `Cargo.lock` whenever any
# member's dependency set changes. So we treat any `Cargo.toml` (workspace OR
# member) change as requiring `Cargo.lock` to also change.
#
# Mode 1 — pre-commit (default): inspect the staged set via
#   `git diff --cached --name-only --diff-filter=AMDR`.
#   Use case: `.git/hooks/pre-commit` invocation.
#
# Mode 2 — CI / PR: inspect the diff against a base ref via
#   `git diff --name-only --diff-filter=AMDR ${LOCKFILE_BASE_REF}...HEAD`.
#   Trigger by setting `LOCKFILE_BASE_REF` (e.g., `origin/main`) in the
#   environment.
#
# Exit codes:
#   0 — no drift, or no manifest changes in scope.
#   1 — drift detected (manifest changed without matching lockfile).
#   2 — usage / environment error.
#
# Configuration:
#   LOCKFILE_BASE_REF   — when set, switch to PR-mode and diff against this
#                          ref (default: unset → pre-commit mode on staged set).
#
# Usage: scripts/check-lockfile-drift.sh
#   (run from repo root or any subdirectory)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

BASE_REF="${LOCKFILE_BASE_REF:-}"

if [[ -n "${BASE_REF}" ]]; then
  if ! git rev-parse --verify "${BASE_REF}" >/dev/null 2>&1; then
    echo "check-lockfile-drift: base ref '${BASE_REF}' is not resolvable." >&2
    echo "check-lockfile-drift: in CI, ensure actions/checkout uses fetch-depth: 0." >&2
    exit 2
  fi
  changed=$(git diff --name-only --diff-filter=AMDR "${BASE_REF}...HEAD" || true)
  mode="ci"
else
  changed=$(git diff --cached --name-only --diff-filter=AMDR || true)
  mode="pre-commit"
fi

if [[ -z "${changed}" ]]; then
  echo "check-lockfile-drift: OK (${mode}: no staged/PR changes)"
  exit 0
fi

# Detect changes per pair. `grep -F -x` pins exact-line match; `grep -E` for
# the Cargo.toml family handles workspace-root and member manifests in one
# regex.
cargo_toml_changed=$(printf '%s\n' "${changed}" | grep -E '(^|/)Cargo\.toml$' || true)
cargo_lock_changed=$(printf '%s\n' "${changed}" | grep -Fx 'Cargo.lock' || true)

e2e_pkg_changed=$(printf '%s\n' "${changed}" | grep -Fx 'tests/e2e/package.json' || true)
e2e_lock_changed=$(printf '%s\n' "${changed}" | grep -Fx 'tests/e2e/package-lock.json' || true)

drift=0

if [[ -n "${cargo_toml_changed}" && -z "${cargo_lock_changed}" ]]; then
  echo "ERROR: Cargo.toml changed without Cargo.lock (Principle IX — lockfile discipline)." >&2
  echo "Run: cargo update --workspace (or cargo build --workspace) and stage Cargo.lock." >&2
  echo "Touched manifest(s):" >&2
  while IFS= read -r path; do
    [[ -z "${path}" ]] && continue
    echo "  - ${path}" >&2
  done <<< "${cargo_toml_changed}"
  drift=1
fi

# Symmetric guard: lockfile-only changes are also drift (the manifest dictates
# the lock; raw lockfile edits are a smell). We allow this for the unusual
# case where `cargo update` is run intentionally — it produces a Cargo.lock
# diff with no manifest change. So we DO NOT flag lock-without-manifest as
# drift; only manifest-without-lock is enforced. Same posture for npm.
# (This matches Principle IX rule 1 verbatim.)

if [[ -n "${e2e_pkg_changed}" && -z "${e2e_lock_changed}" ]]; then
  echo "ERROR: tests/e2e/package.json changed without tests/e2e/package-lock.json (Principle IX)." >&2
  echo "Run: cd tests/e2e && npm install (regenerates lock) and stage tests/e2e/package-lock.json." >&2
  drift=1
fi

if [[ ${drift} -eq 0 ]]; then
  echo "check-lockfile-drift: OK (${mode}: manifest ↔ lockfile pairs balanced)"
fi

exit ${drift}
