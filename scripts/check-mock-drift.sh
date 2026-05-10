#!/usr/bin/env bash
# Mock drift gate (spec 001-leptos-migration T021).
#
# Asserts that every #[tauri::command] handler declared in src-tauri/src/lib.rs
# has a matching `case "<name>":` in tests/e2e/fixtures/tauriMock.js, and vice
# versa, after subtracting:
#   * Plugin-injected commands the mock services for completeness (e.g.,
#     `plugin:updater|check`, `plugin:opener|open_url`) — these are not
#     declared as #[tauri::command] in our crate.
#
# Exit codes:
#   0 — every surviving handler has a mock entry, every mock case has a
#       handler (or is a documented plugin/transition exemption).
#   1 — drift detected. Missing entries are listed on stderr.
#
# Usage: scripts/check-mock-drift.sh
#   (run from repo root or any subdirectory; resolves paths off this script's
#    location)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

LIB_RS="${REPO_ROOT}/src-tauri/src/lib.rs"
MOCK_JS="${REPO_ROOT}/tests/e2e/fixtures/tauriMock.js"

if [[ ! -f "${LIB_RS}" ]]; then
  echo "check-mock-drift: missing ${LIB_RS}" >&2
  exit 1
fi
if [[ ! -f "${MOCK_JS}" ]]; then
  echo "check-mock-drift: missing ${MOCK_JS}" >&2
  exit 1
fi

# Plugin-injected and OS-level commands that the mock services for fidelity but
# that are not declared as #[tauri::command] in our crate. These are anything
# beginning with `plugin:` or that targets the Tauri runtime directly.
# Pattern: anything containing a colon, plus `tauri:close`-style runtime hooks.
is_plugin_command() {
  local name="$1"
  [[ "${name}" == *:* ]]
}

# Extract handler names: every `async fn <name>(` (or `fn <name>(`) immediately
# following `#[tauri::command]`. We use grep -A1 to capture the function line
# after each attribute, then sed out the name.
handler_names=$(grep -E -A1 '^\s*#\[tauri::command\]\s*$' "${LIB_RS}" \
  | grep -E '^\s*(async\s+)?fn\s+[a-z_][a-z0-9_]*' \
  | sed -E 's/^[[:space:]]*(async[[:space:]]+)?fn[[:space:]]+([a-z_][a-z0-9_]*).*/\2/' \
  | sort -u)

# Extract mock case names: every `case "<name>":` line in the mock fixture.
# Matches both single and double quoted cases (we currently use double).
mock_cases=$(grep -E '^\s*case\s+"[^"]+"\s*:' "${MOCK_JS}" \
  | sed -E 's/^[[:space:]]*case[[:space:]]+"([^"]+)"[[:space:]]*:.*/\1/' \
  | sort -u)

# Filter mock_cases: drop plugin-injected ones (no matching handler by design).
filtered_mock_cases=""
while IFS= read -r name; do
  [[ -z "${name}" ]] && continue
  if is_plugin_command "${name}"; then
    continue
  fi
  filtered_mock_cases="${filtered_mock_cases}${name}"$'\n'
done <<< "${mock_cases}"
filtered_mock_cases=$(printf '%s' "${filtered_mock_cases}" | sed '/^$/d' | sort -u)

# All declared handlers must have a mock case — no exemptions post-Phase-6.
filtered_handlers=""
while IFS= read -r name; do
  [[ -z "${name}" ]] && continue
  filtered_handlers="${filtered_handlers}${name}"$'\n'
done <<< "${handler_names}"
filtered_handlers=$(printf '%s' "${filtered_handlers}" | sed '/^$/d' | sort -u)

# Diff the two sets.
missing_in_mock=$(comm -23 \
  <(printf '%s\n' "${filtered_handlers}") \
  <(printf '%s\n' "${filtered_mock_cases}"))
missing_in_handlers=$(comm -13 \
  <(printf '%s\n' "${filtered_handlers}") \
  <(printf '%s\n' "${filtered_mock_cases}"))

drift=0
if [[ -n "${missing_in_mock}" ]]; then
  echo "check-mock-drift: handlers without a mock case in tauriMock.js:" >&2
  while IFS= read -r name; do
    [[ -z "${name}" ]] && continue
    echo "  - ${name}" >&2
  done <<< "${missing_in_mock}"
  drift=1
fi
if [[ -n "${missing_in_handlers}" ]]; then
  echo "check-mock-drift: mock cases without a #[tauri::command] handler in lib.rs:" >&2
  while IFS= read -r name; do
    [[ -z "${name}" ]] && continue
    echo "  - ${name}" >&2
  done <<< "${missing_in_handlers}"
  drift=1
fi

if [[ ${drift} -eq 0 ]]; then
  handler_count=$(printf '%s\n' "${filtered_handlers}" | sed '/^$/d' | wc -l | tr -d ' ')
  echo "check-mock-drift: OK (${handler_count} handlers ↔ mock cases reconciled)"
fi

exit ${drift}
