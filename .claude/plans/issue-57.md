# Implementation Plan for #57

**Issue:** Strip stale Supabase CSP allowlist entry from tauri.conf.json
**Type:** chore
**Branch:** agentex/57-strip-supabase-csp

---

Research complete. Only `src-tauri/tauri.conf.json:26` carries the URL. Doc/spec mentions are historical (constitution, e2e CLAUDE.md) and should stay.

# Chore: Strip stale Supabase CSP allowlist entry from `tauri.conf.json`

## Chore Description

`src-tauri/tauri.conf.json` line 26 still lists `https://lopgwwppinkqvttozqfx.supabase.co` in the production CSP `connect-src` directive. The entry predates the local-only pivot (#50), which removed all Supabase auth/sync code paths but did not prune the CSP allowlist that authorised them.

Per **Constitution Principle II** (no network egress for user data) and **Principle X** (Buck Stops Here — pre-existing technical debt is in scope when it conflicts with a current principle), the allowlist entry is a dormant attack surface: any future regression that re-introduces a Supabase-shaped URL would silently succeed against the runtime CSP. Removing it makes the CSP express the actual networking contract — `self` (Tauri's bundled assets), Google Fonts (style/font CDN, already declared), `tauri:` / `ipc:` / `http://ipc.localhost` (IPC), and nothing else.

Scope: one-line edit to one file. No code path change, no migration, no user-visible behaviour change. The auto-updater traffic to GitHub is whitelisted by Tauri's HTTP plugin, not by `connect-src` (HTML CSP only governs in-WebView fetches).

## Relevant Files

Use these files to resolve the chore:

- **`src-tauri/tauri.conf.json`** — the single source of truth for the runtime CSP. Line 26's `connect-src` is the only occurrence of the Supabase URL in the codebase (verified by grep for `lopgwwppinkqvttozqfx`).
- **`tests/e2e/CLAUDE.md`** — references "CDN scripts (Supabase, Google Fonts, jsDelivr)" as examples of what the `_blockExternal` fixture stops. Historical context; do not edit. Confirms that the e2e suite already blocks any Supabase egress regardless of CSP, so removing the allowlist cannot regress tests.
- **`.specify/memory/constitution.md`** — Principles II and X are the rationale; Supabase is mentioned only in historical "what changed when we pivoted to local-only" narrative. Do not edit.
- **`src/Trunk.toml`** — documents the CSP-driven post-build hook (`externalize-boot`) that pins `script-src` to `'self' 'wasm-unsafe-eval'`. Read-only — confirms that CSP changes are sensitive and must keep `script-src` and `'wasm-unsafe-eval'` untouched.
- **`tests/e2e/fixtures/blockExternal.js`** (referenced by `tests/e2e/CLAUDE.md`) — auto-fixture that blocks non-loopback HTTP. No Supabase-specific entry; nothing to update here. Verified via grep (no matches).

### New Files

None.

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom.

### 1. Verify the scope is exactly one file

- Run `Grep` for `lopgwwppinkqvttozqfx` across the repo.
- Confirm the only hit is `src-tauri/tauri.conf.json:26`. If any other file matches (e.g. a freshly added spec, fixture, or test), stop and re-scope.
- Run `Grep` for `supabase` (case-insensitive) across `src/`, `src-tauri/`, `tests/e2e/fixtures/`, and `tools/`. Confirm zero hits in runtime/test code (doc/spec hits in `.specify/`, `specs/`, `tests/e2e/CLAUDE.md`, `.claude/plans/` are historical and stay).

### 2. Remove the stale entry from the CSP

- Edit `src-tauri/tauri.conf.json` line 26.
- Old `connect-src` token list: `connect-src 'self' https://lopgwwppinkqvttozqfx.supabase.co tauri: ipc: http://ipc.localhost`
- New `connect-src` token list: `connect-src 'self' tauri: ipc: http://ipc.localhost`
- Preserve every other CSP directive (`default-src`, `script-src`, `style-src`, `font-src`) byte-for-byte. Preserve the surrounding JSON shape, the directive ordering, and the trailing whitespace inside the CSP string.

### 3. Validate JSON structure

- Run `python3 -m json.tool src-tauri/tauri.conf.json > /dev/null` (or `jq . src-tauri/tauri.conf.json > /dev/null`) to confirm the file is still valid JSON.

### 4. Build the frontend to confirm the post-build boot externaliser is unaffected

- Run `(cd src && trunk build)`.
- Inspect `src/dist/index.html` to confirm `<script src="boot.js">` is referenced (no inline `<script type="module">`). This proves that with the new tighter `connect-src`, the externalised-boot CSP contract is still consistent.

### 5. Run the strict-deny lint pass (both crates)

- Run `cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic`. CSP is a JSON-string config; clippy will not flag the change directly, but the pass confirms the workspace is clean before opening the PR.

### 6. Run host-side tests

- Run `cargo test --workspace --frozen`. None of the existing unit/integration tests assert against the CSP string, so all suites must remain green.

### 7. Run the e2e visual regression suite

- Install Playwright browsers if needed: `(cd tests/e2e && npx playwright install --with-deps chromium)`.
- Run `(cd tests/e2e && npx playwright test visual-regression.spec.js)`.
- All 12 baselines must remain within the 2% pixel-ratio gate. The CSP change cannot affect rendering (no in-app code fetches Supabase); this run is the no-regression receipt for the UI contract.

### 8. Run the broader e2e suite

- Run `(cd tests/e2e && npx playwright test)`. All 17 specs must pass. The `_blockExternal` auto-fixture has always blocked Supabase regardless of CSP, so test posture is unchanged.

### 9. Run the CI gate scripts

- Run `bash scripts/check-mock-drift.sh` — confirms `tauriMock.js` still mirrors the Tauri handler set.
- Run `bash scripts/check-engine-purity.sh` — sanity check; unrelated to CSP, but in the standard gate set.
- Run `bash scripts/check-lockfile-drift.sh` — no lock changes expected.
- Run `bash scripts/check-baseline-cap.sh` — confirms ≤2 baseline re-captures (this PR re-captures zero).

### 10. Format check

- Run `cargo fmt --all --check`. No Rust files were touched, so this must pass trivially.

### 11. Commit the change

- Stage only `src-tauri/tauri.conf.json`.
- Commit with a Conventional Commits message tied to issue #57, e.g. `chore(csp): drop stale Supabase entry from connect-src (#57)`. Body should reference Principle II and Principle X and call out that this is the local-only-pivot (#50) follow-up flagged by the memorylint gate on feature 004.

## Validation Commands

Execute every command to validate the chore is complete with zero regressions.

```bash
# Scope guard — must return zero hits after the edit
rg -n 'lopgwwppinkqvttozqfx' .
rg -n -i 'supabase' src src-tauri tools tests/e2e/fixtures

# JSON well-formedness
python3 -m json.tool src-tauri/tauri.conf.json > /dev/null

# Frontend build (also exercises the externalise-boot post-build hook)
(cd src && trunk build)

# Strict lint posture
cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic

# Formatting
cargo fmt --all --check

# Host-side tests
cargo test --workspace --frozen

# E2E + visual regression
(cd tests/e2e && npx playwright test visual-regression.spec.js)
(cd tests/e2e && npx playwright test)

# CI gate scripts
bash scripts/check-mock-drift.sh
bash scripts/check-engine-purity.sh
bash scripts/check-lockfile-drift.sh
bash scripts/check-baseline-cap.sh
```

Expected outcomes:
- `rg` for `lopgwwppinkqvttozqfx`: zero matches.
- `rg` for `supabase` in runtime/test directories: zero matches (doc/spec/plan hits are out of those paths).
- `json.tool`: silent success (exit 0).
- `trunk build`: builds successfully; `src/dist/index.html` references `boot.js` (not inline).
- `clippy`, `fmt --check`, `cargo test`: green.
- Playwright visual regression: 12/12 baselines pass within 2%.
- Playwright full suite: 17/17 specs pass.
- All four CI gate scripts: exit 0.

## Notes

- **Why not also remove `https://fonts.googleapis.com` / `https://fonts.gstatic.com`?** Those are still load-bearing — `style-src` and `font-src` permit the Google Fonts CSS and font binaries that the frontend currently references. They are *style/font* CDN traffic, not user data, and are consistent with Principle II's "no network egress for **user data**" framing. Out of scope for this chore.
- **Why is the auto-updater traffic to `github.com` not in `connect-src`?** Tauri's updater plugin runs in the Rust host, not in the WebView. HTML CSP `connect-src` only constrains in-WebView `fetch`/`XMLHttpRequest`/`WebSocket`. The updater therefore needs no CSP allowlist entry, and its egress is governed by the plugin's allowlist in the same config file (the `plugins.updater.endpoints` field), which is unrelated to this change.
- **Baseline re-capture budget.** No baselines should change — confirm zero baseline diffs in the Playwright run. If any baseline changes unexpectedly, **stop**: that would indicate a CSP-driven render regression (e.g. a font failing to load), which would be a real bug to investigate, not a baseline to refresh.
- **Memorylint follow-through.** This chore closes finding F1 from the memorylint gate on feature 004-ambient-sounds. After merge, re-run the gate against `specs/004-ambient-sounds/` to confirm F1 clears.
- **No `--no-verify`** on the commit; the pre-commit lockfile-drift hook runs but has nothing to flag (no manifest/lock changes).

---
*Generated by Agentex*
