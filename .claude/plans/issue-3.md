# Implementation Plan for #3

**Issue:** Delete stale update-manager variants + tighten unsafe_code lint
**Type:** chore
**Branch:** agentex/3-delete-stale-tighten-lint

---

I have enough context. The issue's file list mentions 5 stale variants, but `update-manager-browser-compatible.js` doesn't exist — only 4 stale files do. I'll call that out in the plan and proceed with the 4 that exist. Two helper shell scripts (`verify-updates.sh`, `setup-updates.sh`) also reference the about-to-be-deleted `update-manager.js` and need to be repointed at `update-manager-global.js`.

Here is the complete plan:

```md
# Chore: Delete stale update-manager variants + tighten unsafe_code lint

## Chore Description

Two small, related cleanups bundled because they touch the same module-hygiene concern (Rust + JS).

**Part 1 — JS:** `src/index.html` only loads `src/managers/update-manager-global.js`. Four other variants exist as dead alternates and are currently excluded from ESLint as a stopgap because at least one (`update-manager-v2-corrected.js`) contains a duplicate `class UpdateManagerV2 { … }` declaration (lines 8 and 779) that would fail to parse if loaded. Delete the dead files and remove the ESLint stopgap.

**Part 2 — Rust:** `src-tauri/Cargo.toml` sets `unsafe_code = "warn"`. The only `unsafe` blocks live in macOS Cocoa/Carbon FFI inside `src-tauri/src/lib.rs` (4 blocks). Add `// SAFETY:` comments documenting the platform contract justifying each block, then tighten the lint to `deny`.

**Note on issue text vs. reality:** The issue lists five files, but `src/managers/update-manager-browser-compatible.js` does not exist on disk; the actual stale set is four files. The acceptance-criteria `git grep` regex (`update-manager-v2\|update-manager-fixed\|update-manager-browser-compatible\|update-manager-corrected`) will still pass after removing the four real files, since the missing fifth name simply has no matches to begin with.

## Relevant Files

Use these files to resolve the chore:

### Files to delete (4)

- `src/managers/update-manager.js` — 981-line dead alternate. Exports `class UpdateManager` and a `updateManager` singleton; never imported. Referenced only by the eslint ignore list and two helper scripts (`setup-updates.sh`, `verify-updates.sh`) that need to be repointed before deletion.
- `src/managers/update-manager-v2.js` — 611-line dead alternate. Exports `class UpdateManagerV2`; never imported.
- `src/managers/update-manager-v2-corrected.js` — 1465-line dead alternate. Contains **two** `export class UpdateManagerV2` declarations (lines 8 and 779) — invalid JavaScript that cannot load.
- `src/managers/update-manager-fixed.js` — 456-line dead alternate. Exports `class UpdateManager`; never imported.

### Files to modify

- `eslint.config.js` — Remove the four `src/managers/update-manager*.js` entries and the explanatory comment from the `ignores` array (lines 11–17). With the files gone, the ignore is no longer needed.
- `src-tauri/Cargo.toml` — Change `unsafe_code = "warn"` to `unsafe_code = "deny"` on line 11.
- `src-tauri/src/lib.rs` — Add a `// SAFETY:` comment immediately above each of the four `unsafe { … }` blocks at lines 1395, 1494, 1545, 1570 documenting the platform contract that justifies the block. With the lint at `deny`, an `unsafe` block missing documentation should be the only practical reason a contributor adds the corresponding `#[allow(unsafe_code)]` — so each block must be documented.
- `setup-updates.sh` — Lines 108–113 reference `src/managers/update-manager.js` to do a `sed` placeholder substitution. Repoint to `src/managers/update-manager-global.js` (the live file). Note: `update-manager-global.js` does not currently contain a `USERNAME/REPOSITORY` placeholder, so the `sed` will be a no-op on a fresh clone, which preserves the previous behavior of being optional/best-effort.
- `verify-updates.sh` — Line 79's `check_file "src/managers/update-manager.js" "Update manager"` would start failing. Repoint to `src/managers/update-manager-global.js`.

### Files to leave alone

- `src/managers/update-manager-global.js` — The live, loaded module. Do not touch.
- `src/components/update-notification.js` — Consumes the global `window.updateManager` set up in `src/main.js:1625`; unaffected by the deletions.
- `src/main.js` — Instantiates `new window.UpdateManagerV2()` from the global script tag in `index.html`; unaffected.
- `src/docs/UPDATER_SETUP.md` — Mentions `UpdateManager` as a generic concept, not a specific filename; unaffected.

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom.

### Step 1: Confirm the live module is intact and nothing else imports the dead variants

- From the repo root, run `grep -rn "update-manager-v2\|update-manager-fixed\|update-manager\\.js" src/ src-tauri/ *.html *.js *.sh 2>/dev/null` (or use Grep tool) to enumerate every reference. Expected hits before deletion: `eslint.config.js` (4 lines), `setup-updates.sh:108`, `verify-updates.sh:79`. No source-code `import`/`require` of any dead variant should appear; if one does, stop and reassess.
- Confirm `src/index.html:19` still loads `update-manager-global.js`.

### Step 2: Repoint the helper shell scripts at the live module

- In `setup-updates.sh`, replace `src/managers/update-manager.js` on line 108 with `src/managers/update-manager-global.js`. The surrounding `sed` on line 110 is a placeholder substitution that will become a no-op on the live file (no `USERNAME/REPOSITORY` token there); that's fine — the conditional `if [ -f "$update_manager_file" ]` already makes this best-effort.
- In `verify-updates.sh`, change line 79 from `check_file "src/managers/update-manager.js" "Update manager"` to `check_file "src/managers/update-manager-global.js" "Update manager"` so the post-deletion verification still passes.

### Step 3: Delete the four stale JS files

Use `git rm` so the deletions are staged in one go:

```
git rm src/managers/update-manager.js \
       src/managers/update-manager-v2.js \
       src/managers/update-manager-v2-corrected.js \
       src/managers/update-manager-fixed.js
```

(Note: do NOT attempt to delete `update-manager-browser-compatible.js` — it does not exist; `git rm` would fail and abort the whole command.)

### Step 4: Remove the ESLint stopgap

- In `eslint.config.js`, delete lines 11–17 (the explanatory comment plus the four `"src/managers/update-manager*.js"` entries) from the `ignores` array. Leave the other ignores (`**/node_modules/**`, `src/styles/**`, `art/**`, `src/docs/**`) untouched. The result should be a 4-entry ignores array with no `update-manager` references.

### Step 5: Document each `unsafe` block in `src-tauri/src/lib.rs`

Add a `// SAFETY:` comment line immediately above each `unsafe {` opener. Suggested wording (adjust if you uncover additional context while reading the surrounding code):

- **Line 1395** (`set_dock_visibility_native`, NSApp activation policy):
  ```rust
  // SAFETY: NSApp() returns a raw pointer that is nil if no shared NSApplication
  // exists. We null-check against `nil` before invoking setActivationPolicy_, and
  // this entire function is only invoked from the main thread via run_on_main_thread,
  // satisfying AppKit's main-thread requirement for NSApplication mutation.
  ```
- **Line 1494** (primary `SetSystemUIMode` call): 
  ```rust
  // SAFETY: SetSystemUIMode is a pure C function from Apple's ApplicationServices
  // (Carbon) framework with no pointer parameters and no aliasing/lifetime contract.
  // The arguments (mode and options) are plain UInt32 values constructed above.
  // The call is dispatched to the main thread via run_on_main_thread upstream.
  ```
- **Line 1545** (retry `SetSystemUIMode` call): 
  ```rust
  // SAFETY: Same contract as the primary SetSystemUIMode call above — pure C ABI,
  // scalar arguments, main-thread dispatched.
  ```
- **Line 1570** (conservative two-step `SetSystemUIMode` call): 
  ```rust
  // SAFETY: Same contract as above — pure C ABI, scalar arguments, main-thread
  // dispatched. The intervening thread::sleep is safe regardless.
  ```

When writing the comments, keep them concise (one short paragraph each) and focused on *why* the unsafe is sound, not *what* the code does.

### Step 6: Tighten the `unsafe_code` lint

- In `src-tauri/Cargo.toml`, change `unsafe_code = "warn"` (line 11) to `unsafe_code = "deny"`. Leave the other `[lints.rust]` entries (`unreachable_pub`, `rust_2018_idioms`) and the entire `[lints.clippy]` block unchanged.

### Step 7: Validate

Run the validation commands in the next section. Any failure means a step above wasn't completed correctly.

### Step 8: Smoke-test the dev app

- `npm run dev` (which is `tauri dev` per `package.json`). Wait for the window to appear.
- In the running app, open Settings → Updates section (or whichever UI surfaces "Check for updates") and trigger a manual update check. The check should run without console errors. The relevant module is `update-manager-global.js`, which is unchanged, so this should pass — but the smoke test confirms no regression.
- Close the dev app cleanly.

## Validation Commands

Execute every command to validate the chore is complete with zero regressions. Run from the repo root unless noted.

```
# 1. The four stale files are gone.
test ! -e src/managers/update-manager.js && \
test ! -e src/managers/update-manager-v2.js && \
test ! -e src/managers/update-manager-v2-corrected.js && \
test ! -e src/managers/update-manager-fixed.js && \
echo "OK: stale files removed"

# 2. The live module is still present.
test -f src/managers/update-manager-global.js && echo "OK: live module present"

# 3. Acceptance-criteria grep returns nothing (run from a tracked git worktree).
git grep -l 'update-manager-v2\|update-manager-fixed\|update-manager-browser-compatible\|update-manager-corrected'
# Expected: no output, exit code 1 (git grep returns 1 when no matches).

# 4. No remaining unqualified references to the deleted update-manager.js either.
git grep -nE 'update-manager\.js' || echo "OK: no stale references to update-manager.js"

# 5. ESLint passes with the ignores removed (and now actually lints update-manager-global.js).
npm run lint

# 6. Prettier and typecheck still pass.
npm run format
npm run typecheck

# 7. Rust lints and tests pass with unsafe_code = "deny".
cd src-tauri && cargo clippy --all-targets -- -D warnings
cd src-tauri && cargo test
cd src-tauri && cargo build

# 8. Confirm Cargo.toml change is in place.
grep 'unsafe_code = "deny"' src-tauri/Cargo.toml && echo "OK: lint tightened"

# 9. Confirm every unsafe block in lib.rs has a SAFETY comment on the line directly above it.
#    This is a coarse check: count of `unsafe {` openers should equal count of `// SAFETY:` markers.
unsafe_count=$(grep -cE '^\s*(let [^=]+= )?unsafe \{' src-tauri/src/lib.rs)
safety_count=$(grep -cE '// SAFETY:' src-tauri/src/lib.rs)
[ "$unsafe_count" -le "$safety_count" ] && echo "OK: SAFETY comments cover all unsafe blocks ($safety_count >= $unsafe_count)"

# 10. Smoke test the dev server (manual, see Step 8).
npm run dev
```

## Notes

- **Issue text vs. reality:** The issue lists 5 stale files, but only 4 exist on disk. `update-manager-browser-compatible.js` is not present (verified via `find src -name "update-manager*"`). Don't try to `git rm` it — that would abort the whole `git rm` invocation and leave the other deletions unstaged. The acceptance-criteria grep regex still passes because a non-existent filename trivially has zero references.
- **Why repoint the shell scripts before deleting:** `verify-updates.sh:79` does `check_file "src/managers/update-manager.js"` and prints a hard error (`❌ Update manager not found at …`) when the file is missing. Repointing to `update-manager-global.js` keeps the verifier accurate. `setup-updates.sh:108` is wrapped in `if [ -f "$update_manager_file" ]`, so it would silently skip — but updating it together is cheaper than leaving a future contributor to find this twice.
- **Why `unsafe_code = "deny"` is safe to enable:** The only `unsafe` in this crate is in `lib.rs`, all four occurrences are documented in this chore, and they are gated behind `#[cfg(target_os = "macos")]`. Cross-platform CI builds will compile without those blocks at all and so won't trip the lint regardless. macOS builds will compile them with `// SAFETY:` comments in place.
- **No JS tests exist** in this project (`grep`-confirmed: no `*.test.js`, no `vitest`/`jest` in `package.json`), so the JS validation surface is `npm run lint` + `npm run format` + `npm run typecheck` + the manual dev-server smoke test. There is nothing to add or update test-wise.
- **Out of scope:** Do not touch `update-manager-global.js` itself, `update-notification.js`, or `main.js`. The chore is strictly file deletion, lint config edits, and `// SAFETY:` documentation — no behavioral changes.
```

---
*Generated by Agentex*
