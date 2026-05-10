#!/usr/bin/env bash
# Install repo-tracked git hooks (spec 001-leptos-migration T248).
#
# Sets `core.hooksPath = .githooks` for this clone so the hooks under
# `.githooks/` (committed) replace the per-clone `.git/hooks/` set
# (gitignored). One-time per clone.
#
# Idempotent — re-running is a no-op.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

if [[ ! -d .githooks ]]; then
  echo "install-git-hooks: missing .githooks/ directory at ${REPO_ROOT}" >&2
  exit 1
fi

# Ensure each tracked hook is executable (git only honours executable hooks).
find .githooks -maxdepth 1 -type f ! -name '*.md' -print0 \
  | xargs -0 -r chmod +x

git config core.hooksPath .githooks
echo "install-git-hooks: core.hooksPath set to .githooks (run once per clone)."
