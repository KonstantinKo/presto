# Implementation Plan for #2

**Issue:** Full TypeScript migration of frontend JS
**Type:** chore
**Branch:** agentex/2-typescript-frontend-migration

---

Now I have enough understanding to produce a thorough plan. Let me write it.

---

# Chore: Full TypeScript migration of frontend JS

## Chore Description

The strict-tooling PR (#1) scaffolded TypeScript for `src/**/*.js` but left `checkJs: false` because turning it on against unannotated vanilla JS produces ~1512 errors (verified locally with `npx tsc --noEmit -p . --checkJs`). The bulk of those errors fall into a handful of categories:

| Code           | Count | Cause                                                                                                                         |
| -------------- | ----- | ----------------------------------------------------------------------------------------------------------------------------- |
| TS2339         | 480   | `Property 'X' does not exist on type 'Window'` — i.e. `window.__TAURI__`, `window.settingsManager`, `window.tagManager`, etc. |
| TS7006         | 371   | `Parameter 'X' implicitly has an 'any' type` — function args without annotations, including catch params                      |
| TS2531/TS18047 | 277   | `Object is possibly 'null'` — DOM `getElementById` / `querySelector` returns                                                  |
| TS7053         | 32    | Index signature missing for object literals indexed by string                                                                 |
| Other          | ~352  | Misc: implicit any returns, Date overloads, Timeout vs `null`, `EventTarget` vs `HTMLElement` casting, etc.                   |

The work is to:

1. Author `globals.d.ts` declaring `window.__TAURI__` and the various manager singletons that the codebase pins onto `window`. This alone wipes out ~600+ errors.
2. Add a `toError(unknown)` helper for narrowing unknown caught errors.
3. Walk file-by-file (smallest first) adding `// @ts-check` and JSDoc annotations to fix remaining errors.
4. Flip `tsconfig.json` `checkJs` to `true` and remove all per-file `// @ts-check` pragmas.
5. Restore the `.agentex.yml` lint comment so it no longer references "checkJs is off".
6. (Optional, deferred) rename `.js` → `.ts`.

There is also one preexisting bug surfaced by the migration: `src/core/pomodoro-timer.js:2` imports a non-existent `_TimeUtils` symbol (the leading underscore makes ESLint treat it as intentionally unused; it's not used in the file). It should simply be deleted from the import.

The acceptance bar: `npx tsc --noEmit -p .` exits 0 with `"checkJs": true` in `tsconfig.json`, the `wontfix` label is removed from issue #2, and CI's `npx tsc --noEmit -p .` step in `.agentex.yml` actually enforces type correctness.

## Relevant Files

Use these files to resolve the chore:

### Configuration

- **`tsconfig.json`** — currently has `"checkJs": false` and a leading `"//"` comment explaining the staged migration. We flip `checkJs` to `true` and delete the migration comment once done. `include` already covers `src/**/*.js`; nothing else needs to change.
- **`.agentex.yml`** — lines 34-36 contain a comment claiming "tsc currently runs with checkJs disabled. Files opt in via `// @ts-check`; once enough are annotated, flip checkJs to true in tsconfig.json." Once the migration is done this comment is wrong and should be removed.
- **`eslint.config.js`** — already configured with `caughtErrorsIgnorePattern: "^_"`. JSDoc `@param`/`@returns` annotations don't affect ESLint. No changes needed unless we add a custom no-untyped-catch rule (out of scope).
- **`package.json`** — already has `"typecheck": "tsc --noEmit -p ."`. No changes needed.

### Source files (in migration order, lowest error count first)

These are listed in the order tasks should tackle them. Each one currently has the listed number of errors when `--checkJs` is forced on.

- **`src/version.js`** — 0 errors. No changes.
- **`src/utils/logger.js`** — 6 errors. Variadic logger; needs JSDoc on `format`, `send`, and a window-typing fix for `window.__TAURI__` (handled by globals.d.ts).
- **`src/utils/theme-loader.js`** — 10 errors. Auto-edited by `build-themes.js`; annotations should be tolerant of regeneration (keep them at module level, not inside the array literal). May warrant updating `build-themes.js` to preserve a `// @ts-check` header it accidentally strips.
- **`src/utils/timer-themes.js`** — 13 errors. Object literal `TIMER_THEMES` typed via JSDoc `@type {Record<string, ThemeConfig>}` to fix index-signature errors.
- **`src/utils/tag-statistics.js`** — 16 errors. Pure utility module.
- **`src/utils/analytics.js`** — 23 errors. Static-method class; mostly catch-block typing and `window.settingsManager` access.
- **`src/managers/team-manager.js`** — 23 errors. Demo-data manager with simple shapes.
- **`src/components/update-notification.js`** — 41 errors. Mix of DOM null-narrowing and `window.__TAURI__` access.
- **`src/utils/supabase.js`** — 44 errors. Heavy `window.supabase` (UMD bundle) reliance; needs `globals.d.ts` entry typing it as `{ createClient: (...) => ... }` (loose `any`-shape is acceptable since the upstream UMD doesn't ship its own `.d.ts` here).
- **`src/managers/auth-manager.js`** — 60 errors. Closely coupled to Supabase types.
- **`src/utils/common-utils.js`** — 61 errors. Holds `NotificationUtils`, `TimeUtils`, `StorageUtils`, `DOMUtils`, `KeyboardUtils`. Note: `pomodoro-timer.js:2` mistakenly imports `_TimeUtils` (not exported) — fix that to `TimeUtils` or remove since it's unused.
- **`src/managers/session-manager.js`** — 68 errors.
- **`src/managers/update-manager-global.js`** — 79 errors. Assigns `window.UpdateManagerV2` and `window.updateManagerV2Debug`.
- **`src/managers/tag-manager.js`** — 66 errors. Assigns `window.tagManager`. Class is non-exported.
- **`src/core/pomodoro-timer.js`** — 171 errors. Largest core file. The first line `const { invoke } = window.__TAURI__.core;` runs at module load — once `window.__TAURI__` is declared in globals.d.ts this stops erroring.
- **`src/managers/navigation-manager.js`** — 204 errors. Includes the XLSX-via-`window.XLSX` codepath and the Tauri dialog/save flow.
- **`src/main.js`** — 214 errors. Top-level entry; assigns many `window.*` properties and contains `showCustomConfirm` with a string-keyed colors lookup.
- **`src/managers/settings-manager.js`** — 319 errors. Largest manager; the bulk of errors are DOM-element nulls and option-object index signatures.

### Tests

- **`tests/setup.js`** — already stubs `globalThis.__TAURI__` for vitest. Should not need changes; the runtime stub is unaffected by compile-time globals.
- **`tests/core/pomodoro-timer.test.js`**, **`tests/utils/common-utils.test.js`**, **`tests/utils/tag-statistics.test.js`**, **`tests/utils/timer-themes.test.js`** — existing JS test files. They are excluded from `tsconfig.json`'s `include` (which targets `src/**/*.js` only), so they stay as-is.

### New Files

- **`src/globals.d.ts`** — Ambient declaration file. Lives alongside source so `tsc -p .` picks it up automatically (ambient `.d.ts` files are always included regardless of `include` patterns when located under the project root). Declares:
  - `Window.__TAURI__` (use loose `any` for inner namespaces — `core`, `event`, `dialog`, `notification`, `updater`, `shell`, plus an `invoke` shortcut method that some code paths use).
  - `Window.supabase` (UMD-loaded from CDN; declare as `{ createClient(url: string, key: string, opts?: any): any } | undefined`).
  - `Window.XLSX` (UMD; `any`).
  - The various manager singletons: `window.app`, `window.appLog`, `window.pomodoroTimer`, `window.settingsManager`, `window.sessionManager`, `window.navigationManager`, `window.teamManager`, `window.tagManager`, `window.authManager`, `window.updateManager`, `window.updateManagerInstance`, `window.UpdateManagerV2`, `window.updateManagerV2Debug`, `window.updateNotification`.
  - The flag globals: `window._appInitializing`, `window._appFullyInitialized`, `window.avatarListenersSetup`.
  - Function globals assigned in main.js: `window.saveSettings`, `window.resetToDefaults`, `window.confirmTotalReset`, `window.performTotalReset`.
  - All declared as optional/loose to minimize cascading null-narrowing churn — the goal of this file is to silence the `TS2339 Property 'X' does not exist` errors, not to retype the whole runtime. Manager singletons can be typed as `any` for the v1 migration; tightening them is out of scope.
- **`src/utils/to-error.js`** — Single-purpose helper:
  ```js
  /** @param {unknown} value @returns {Error} */
  export function toError(value) {
    return value instanceof Error ? value : new Error(String(value));
  }
  ```
  Used at every `catch (err) { ...err.message... }` site to satisfy `--strict`. Importing the helper is preferred over per-file JSDoc casts because it's much shorter at every call site and self-documents intent.

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom.

### 1. Establish baseline & branch hygiene

- Confirm current branch is `agentex/2-typescript-frontend-migration` (already created).
- Run `npm ci` to ensure deterministic deps.
- Run `npm run typecheck` and confirm exit 0 (current state with `checkJs: false`).
- Run `npx tsc --noEmit -p . --checkJs 2>&1 | wc -l` and record the baseline count (~1512). After every file's migration step the count must drop monotonically.

### 2. Create `src/globals.d.ts`

- Create the file with ambient declarations covering every `window.*` surface listed in the New Files section.
- Type `__TAURI__` as `any` rather than the full set of nested namespaces (`core`, `event`, `dialog`, `notification`, `updater`, `shell`). Tightening these is out of scope; the goal is to unblock `tsc`.
- Type `supabase` and `XLSX` as `any`.
- Type each manager singleton as `any` (e.g. `settingsManager?: any;`). They are mutated cross-file and tightening shapes will explode the diff.
- Type the function-globals assigned in main.js (e.g. `saveSettings`, `resetToDefaults`, `confirmTotalReset`, `performTotalReset`) as `() => Promise<void>` or `() => void` based on their actual signatures.
- Verify `npx tsc --noEmit -p . --checkJs 2>&1 | wc -l` now drops by several hundred (target: ~600+ removed).

### 3. Add `src/utils/to-error.js`

- Create the helper exactly as specified in the New Files section.
- This is just additive — no errors yet, no errors removed.

### 4. Fix the preexisting `_TimeUtils` import bug

- In `src/core/pomodoro-timer.js:2`, change `import { NotificationUtils, _TimeUtils, KeyboardUtils }` → `import { NotificationUtils, KeyboardUtils }`. The `_TimeUtils` symbol does not exist in `common-utils.js` (only `TimeUtils` does), and verifiable grep shows it's never referenced in that file. ESLint hides this bug because its `^_` prefix matches `caughtErrorsIgnorePattern`/`varsIgnorePattern`.

### 5. Migrate utility files (smallest first)

Each sub-step follows the same pattern: add `// @ts-check` at the top, fix every error, run `npx tsc --noEmit -p . --checkJs` (or per-file `npx tsc --noEmit --allowJs --checkJs <file>`), commit individually. The goal is small reviewable commits.

#### 5a. `src/utils/logger.js`

- Add `// @ts-check` at top.
- Add JSDoc to `format(args)` → `@param {unknown[]} args @returns {string}`.
- Add JSDoc to `send(fn, consoleFn)` → `@param {(msg: string) => Promise<void>} fn @param {(...args: unknown[]) => void} consoleFn`.
- Annotate the rest-args parameter inside the returned function.

#### 5b. `src/utils/timer-themes.js`

- Add `// @ts-check`.
- Define a JSDoc typedef `@typedef {{ name: string; description: string; supports: ('light'|'dark')[]; isDefault: boolean; preview: { focus: string; break: string; longBreak: string } }} ThemeConfig`.
- Annotate `TIMER_THEMES` as `@type {Record<string, ThemeConfig>}` to fix the index-signature errors on `TIMER_THEMES[themeId]`.
- Annotate function parameters with `@param {string} themeId` etc.

#### 5c. `src/utils/theme-loader.js`

- Add `// @ts-check`.
- Note that `build-themes.js` regenerates a section of this file; if the regenerator strips the pragma, update `build-themes.js` so it preserves the leading `// @ts-check` line. (Look at lines 50-58 of `build-themes.js` to see the rewrite logic.)

#### 5d. `src/utils/tag-statistics.js`

- Add `// @ts-check` and fix any remaining annotation errors.

#### 5e. `src/utils/analytics.js`

- Add `// @ts-check`.
- Replace `catch (error)` access of `error.message` with `toError(error).message` from the new helper, OR type the catch via `/** @type {unknown} */ error` plus a narrow check. Prefer the helper for brevity.

#### 5f. `src/utils/common-utils.js`

- Add `// @ts-check`.
- Many DOM lookup sites need null guards. For elements created by `document.createElement` no guard is needed (return type is non-null). For `document.querySelector` / `getElementById` callsites, either:
  - assert non-null with `@type` JSDoc casts where the markup guarantees presence, or
  - branch with `if (!el) return;` where genuine absence is possible.
- `webkitAudioContext` access on line 264 should use a `@ts-ignore` or, better, declare it on Window in `globals.d.ts`.

#### 5g. `src/utils/supabase.js`

- Add `// @ts-check`.
- `window.supabase` is now `any`-typed; this should clear most errors.

### 6. Migrate components & smaller managers

#### 6a. `src/managers/team-manager.js`

- Add `// @ts-check`. JSDoc-annotate the `Team` and `Member` shapes if helpful, but typing `this.teams` as `any[]` is acceptable.

#### 6b. `src/components/update-notification.js`

- Add `// @ts-check`.
- Big classes of fixes:
  - Cast `EventTarget` to `HTMLElement` at `e.target` sites where dataset/parentElement are accessed: `const target = /** @type {HTMLElement} */ (e.target);`.
  - Null-guard the various `this.container.querySelector(...)` returns or assert non-null when the markup guarantees them.
- Replace catch-param usage of `err.message` with `toError(err)`.

#### 6c. `src/managers/auth-manager.js`

- Add `// @ts-check`. `this.supabase` and `this.authHelpers` can be typed `any` since the underlying client is.

#### 6d. `src/managers/session-manager.js`

- Add `// @ts-check`. `invoke` typed as `any | null` from globals.d.ts.

#### 6e. `src/managers/update-manager-global.js`

- Add `// @ts-check`. The class assigned to `window.UpdateManagerV2` will need to match the loose shape declared in `globals.d.ts` — keeping that ambient type as `any` avoids a circular constraint.

#### 6f. `src/managers/tag-manager.js`

- Add `// @ts-check`.

### 7. Migrate the three large files

These are the heavy lifts. Apply the same pattern: `// @ts-check`, JSDoc-annotate the public surface (constructor, methods), null-guard DOM lookups, narrow event targets, swap untyped catches to `toError`.

#### 7a. `src/core/pomodoro-timer.js` (~171 errors)

- The early `const { invoke } = window.__TAURI__.core;` should now type-check with globals.d.ts in place.
- For `setTimeout`/`clearTimeout` ID fields like `this.timerInterval` declared as `null` and reassigned to a `Timeout`, type the field as `@type {ReturnType<typeof setTimeout> | null}`.
- For methods on the class, add `@param`/`@returns` JSDoc as needed.

#### 7b. `src/managers/navigation-manager.js` (~204 errors)

- Includes the XLSX export flow at lines 1700-1780. `window.XLSX` is typed `any` in globals.d.ts so most of those errors clear automatically.
- Same patterns as pomodoro-timer for DOM nulls and event handler types.

#### 7c. `src/main.js` (~214 errors)

- The `colors` lookup on line 119-123 is keyed by an untyped `type` string — annotate `type` as `'warning' | 'error'` and cast the lookup, or annotate `colors` as `@type {Record<string, {bg: string; border: string; text: string}>}`.
- For `error.message` accesses in `catch (error)`, swap to `toError(error).message`.
- Several direct `window.foo = ...` assignments; the assignments themselves now type-check because globals.d.ts declares those properties.

### 8. Migrate `src/managers/settings-manager.js` (~319 errors)

- This is the biggest single file. Same patterns; expect heavy DOM-null narrowing.
- Add `// @ts-check`.
- Resolve every error to zero.

### 9. Flip `checkJs` to `true` and remove pragmas

- Edit `tsconfig.json`:
  - Set `"checkJs": true`.
  - Remove the leading `"//"` comment that explains the migration.
- Remove the `// @ts-check` pragma from the top of every `src/**/*.js` file (it's now redundant). Use a sweep:
  - `grep -rln "^// @ts-check" src` to list all of them.
  - Edit each file to drop the line.
- Run `npx tsc --noEmit -p .` and confirm exit 0.

### 10. Update the agentex config comment

- In `.agentex.yml`, remove lines 34-35 (the comment block explaining checkJs is disabled). The `npx tsc --noEmit -p .` line on line 36 stays — it will now actually enforce type correctness.

### 11. Run the full validation suite

- Run every command in the **Validation Commands** section. Every command must exit 0.

### 12. (Out of scope, do not perform) Rename `.js` → `.ts`

- The chore explicitly lists this as an "optional next step." Doing it touches every import path, every test, every CI command, and forces a Vite/Tauri build-pipeline switch. Defer to a follow-up issue.

## Validation Commands

Execute every command to validate the chore is complete with zero regressions.

```bash
# 1. Clean install
npm ci

# 2. The headline goal: type check passes with checkJs ON
npx tsc --noEmit -p .

# 3. Run the same command via npm script wrapper to mirror CI
npm run typecheck

# 4. Lint must still pass — TS migration must not break ESLint rules
npx eslint src

# 5. All unit tests must still pass — JSDoc annotations must not change runtime behavior
npm test

# 6. Format check — ensure no formatting regressed during edits
npx prettier --check .

# 7. Confirm no `// @ts-check` pragmas were left behind (they're redundant once checkJs is on)
test -z "$(grep -rln '^// @ts-check' src)" || (echo 'Stale @ts-check pragmas found' && exit 1)

# 8. Confirm `checkJs` is on in tsconfig
node -e "process.exit(JSON.parse(require('fs').readFileSync('tsconfig.json', 'utf8')).compilerOptions.checkJs === true ? 0 : 1)"

# 9. Confirm theme-builder still produces valid output (regenerates theme-loader.js)
npm run build-themes
npx tsc --noEmit -p .

# 10. Confirm Rust side still builds and tests pass — orthogonal but required by .agentex.yml
cd src-tauri && cargo build --all-targets && cargo test && cd ..

# 11. Confirm Rust lint still passes
cd src-tauri && cargo clippy --all-targets -- -D warnings && cd ..

# 12. Confirm Rust formatting still passes
cd src-tauri && cargo fmt -- --check && cd ..
```

## Notes

- **Why JSDoc and not direct `.ts` rename?** The chore's primary acceptance criterion is `npx tsc --noEmit -p .` clean with `checkJs: true`. The `.ts` rename is explicitly listed as an optional follow-up. Renaming requires touching the bundler config (Tauri uses Vite under the hood; Vite handles `.ts` natively but every import path needs updating, plus the test config). That's a separate bounded chunk of work and should be a follow-up issue once the type-check baseline is enforced.

- **Why `any` for manager singletons in globals.d.ts?** These are cross-cut with circular references (e.g. `pomodoro-timer.js` references `window.tagManager`, `tag-manager.js` references `window.pomodoroTimer`). Tightening their types means defining the public surface of every manager class up front, which is itself a multi-day project. `any` lets us claim the acceptance bar of "tsc clean with checkJs on" today and leaves room to tighten later.

- **The `_TimeUtils` import bug** at `src/core/pomodoro-timer.js:2` is a real bug masked by ESLint's `^_` ignore convention combined with `checkJs: false`. Fixing it as part of this migration is appropriate — leaving it would surface as a TS2724 error mid-migration. Verify with `grep -n "TimeUtils" src/core/pomodoro-timer.js` that the import is unused after the fix and that no runtime call sites depend on it.

- **`build-themes.js` rewrites `src/utils/theme-loader.js`.** Confirm the regenerator preserves any `// @ts-check` line we might add (only relevant during migration; after step 9 the pragma is removed everywhere). After step 9 there's no special handling needed since `checkJs` is global.

- **CI enforcement.** The `.agentex.yml` already runs `npx tsc --noEmit -p .` in the lint section (line 36). Once `checkJs: true` is set, this command actually catches type regressions. The misleading comment on lines 34-35 should be removed in step 10 because future readers will be confused by a stale "checkJs is off" claim.

- **Test files are excluded.** `tsconfig.json` includes only `src/**/*.js`, not `tests/**/*.js`. We do not need to type-check tests as part of this chore. If we want to extend coverage to tests later, that's a follow-up.

- **Vitest mock at `tests/setup.js:14`** stubs `globalThis.__TAURI__` for the test runtime. That's a runtime concern unaffected by ambient `.d.ts` declarations. No changes needed there.

- **No new runtime dependencies.** `to-error.js` is a 3-line pure helper; everything else is type-only authoring. The diff should be entirely additive (new `globals.d.ts`, new `to-error.js`) plus annotation/comment additions inside existing files plus the eventual two-line tsconfig flip.

- **Estimated scope.** ~1512 errors across 17 files. Expect ~16 commits (one per file plus the foundational globals.d.ts/to-error.js commit plus the final `checkJs: true` flip commit). Each per-file commit should drop the error count monotonically; if a commit raises the count, something regressed and the commit should be revisited before proceeding.

---

_Generated by Agentex_
