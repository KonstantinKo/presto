# Implementation Plan for #5

**Issue:** Migrate from println!/console.log to a real logger
**Type:** chore
**Branch:** agentex/5-migrate-to-logger

---

I have enough context to write the plan. Now I'll output the complete plan.

````md
# Chore: Migrate from println!/console.log to a real logger

## Chore Description

Both halves of the codebase use ad-hoc print logging that PR #1 deliberately did not lint-block, because doing so would have required this migration:

- **Rust** (`src-tauri/src/lib.rs`): 14 `println!`/`eprintln!` call sites scattered across the global-shortcuts startup flow, status-bar visibility (Carbon `SetSystemUIMode`), and OAuth/menu/visibility paths.
- **JavaScript**: ~520 `console.log` / `console.warn` / `console.error` calls across 16 files in `src/`.

This chore replaces them with a real leveled logger so production builds can ship at `info`/`warn`/`error` while dev keeps `debug`/`trace`. Acceptance criteria:

- No `println!`/`eprintln!` in Rust source (excluding tests, of which there are none).
- No `console.*` in JS source (excluding `src/docs/` which ESLint already ignores).
- `clippy::print_stdout` / `clippy::print_stderr` denied in `Cargo.toml`.
- `no-console` enforced in `eslint.config.js`.
- App still produces visible logs during `npm run dev`.

We adopt the **Tauri-integrated approach (Option 2 from the issue)**: `tauri-plugin-log` on the Rust side and `@tauri-apps/plugin-log` on the JS side, so logs from both halves land in the same stream/file. This is a Tauri app — Rust always initializes first, so the "JS depends on Rust init" concern from the issue is moot. To keep the migration mechanical, we add a thin wrapper module (`src/utils/logger.js`) that accepts variadic args (matching `console.*` ergonomics) and forwards strings to the plugin. We also expose it on `window.appLog` for the two non-ES-module global scripts (`tag-manager.js`, `update-manager-global.js`) that can't `import`.

## Relevant Files

### Files to modify

#### Rust

- `src-tauri/Cargo.toml` — Add `log = "0.4"` and `tauri-plugin-log = "2"` to `[dependencies]`. Add `print_stdout = "deny"` and `print_stderr = "deny"` under `[lints.clippy]` (the existing block is at lines 15–20 with `dbg_macro` and `todo` already denied — same shape).
- `src-tauri/src/lib.rs` — Three migration locations:
  - **Three `eprintln!` calls** at lines 1119, 1123, 1132 (startup shortcut registration failures inside the `tauri::async_runtime::spawn` block in `setup()`) → `log::error!`.
  - **Status-bar Carbon flow** at lines 1444 (`println!` success log), 1451 (`eprintln!` failure log), 1514 (`println!` "🔧 Carbon API: Setting SystemUIMode…" debug trace), 1528 (`println!` "✅ Carbon API: SetSystemUIMode succeeded"), 1536 (`eprintln!` "❌ Carbon API: …"), 1549 (`eprintln!` "🔄 Primary method failed…"), 1553 (`println!` "🔄 Fallback 1…"), 1567 (`println!` "✅ Fallback 1: Retry succeeded"), 1581 (`println!` "🔄 Fallback 2…"), 1592 (`println!` "✅ Fallback 2: Conservative approach succeeded"), 1615 (`eprintln!` "❌ {detailed_error}").
  - The `pub fn run()` builder chain at lines 920–932 — add `.plugin(tauri_plugin_log::Builder::new()…)` so the plugin is registered before `setup()` is called.
- `src-tauri/capabilities/default.json` — Add `"log:default"` to the `"permissions"` array so the frontend (and the plugin's IPC) is allowed to use the log commands. The existing capability format is one permission per line; add it next to `"core:default"`.

#### JavaScript

- `package.json` — Add `"@tauri-apps/plugin-log": "^2"` to `"dependencies"`. Pin to the major version that matches the Rust crate (Tauri 2.x ecosystem; current `@tauri-apps/api` is `2.6.0`).
- `eslint.config.js` — Add `"no-console": ["error"]` to the `rules` block (between the existing "Stylistic" and "Security / footguns" sections). The existing `ignores` array already excludes `src/docs/**`, so the 6 console references in `src/docs/TEST_MANUAL.md` are unaffected. There are no JS test files in this project (verified — no `*.test.js`, no `vitest`/`jest` in `package.json`), so no test-mock allowlist is needed.
- `src/main.js` — Add `import { logger } from "./utils/logger.js"`, expose `window.appLog = logger` near the top of module-init so the non-module scripts can pick it up before their constructors run, and migrate the 79 console.\* calls.
- `src/managers/auth-manager.js` (2 calls), `src/managers/navigation-manager.js` (26), `src/managers/session-manager.js` (8), `src/managers/settings-manager.js` (71), `src/managers/team-manager.js` (2) — Add `import { logger } from "../utils/logger.js"`; migrate console.\*.
- `src/managers/update-manager-global.js` (64 calls) — **Non-module script**, loaded via `<script src="…" defer>` in `src/index.html:19`. Cannot `import`. Use `window.appLog` everywhere. Class is only instantiated later, so by the time its constructor runs `main.js` has already populated `window.appLog`.
- `src/managers/tag-manager.js` (13 calls) — **Non-module script**, loaded via `<script src="…" defer>` in `src/index.html:20`. Same rule: use `window.appLog`. Class definition is synchronous but instantiation happens later, after `main.js` exposes the global.
- `src/components/update-notification.js` (28 calls) — ES module; `import { logger } from "../utils/logger.js"`.
- `src/core/pomodoro-timer.js` (163 calls — by far the largest migration site).
- `src/utils/analytics.js` (2), `src/utils/common-utils.js` (9), `src/utils/supabase.js` (31), `src/utils/tag-statistics.js` (1), `src/utils/theme-loader.js` (12), `src/utils/timer-themes.js` (3) — ES modules; `import { logger } from "./logger.js"`.

### Files to leave alone

- `src/index.html` — No change to `<script>` order. The defer ordering already guarantees `main.js` (which is `type="module"` — also implicitly deferred) runs in document order relative to the two global scripts; the global scripts only **define** classes synchronously. Their **constructors** (which contain the console calls) only run when `main.js` later does `new window.UpdateManagerV2()` / instantiates `TagManager`, by which point `window.appLog` is set.
- `src/docs/TEST_MANUAL.md` — 6 mentions of `console.*` are inside a Markdown file already excluded by `eslint.config.js:6` (`src/docs/**`). Leave as-is.
- `src-tauri/src/main.rs` — Just calls `presto_lib::run()`; no logging.
- `src-tauri/build.rs` — Build script; not a runtime path.
- `tauri.conf.json` — No new permission/configuration needed at the conf level (capability change in `capabilities/default.json` is the only permission edit).
- `build-themes.js` / `debug-themes.js` — Repo-root build scripts run under Node, not in the app. Their `console.*` calls (if any) are not subject to the app's lint scope (`npm run lint` targets `src/`, see `package.json:13`).

### New Files

- `src/utils/logger.js` — Thin wrapper around `@tauri-apps/plugin-log`. Exports a `logger` object with `debug`/`info`/`warn`/`error` methods that:
  1. Accept variadic args matching `console.*` ergonomics (so migration is mostly mechanical search-and-replace).
  2. Stringify non-string args via `JSON.stringify` with a circular-safe fallback to `String(arg)` for `Error`, `DOM Node`, etc.
  3. Forward to `info`/`warn`/`error`/`debug` from `@tauri-apps/plugin-log`.
  4. Catch any rejected promise from the plugin's IPC call and swallow it (a logger that throws into the application is worse than a logger that silently drops a line).

  This is the only new source file. Keep it small (≤40 lines).

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom.

### Step 1: Add the Rust dependencies

- In `src-tauri/Cargo.toml`, add to `[dependencies]` (alphabetical placement is fine; existing Tauri plugins are grouped together):
  ```toml
  log = "0.4"
  tauri-plugin-log = "2"
  ```
````

- Run `cd src-tauri && cargo fetch` from the repo root to confirm the crates resolve before doing the source edit. (If the resolver complains about a `2.x` mismatch with the Tauri runtime, check `src-tauri/Cargo.lock` for the resolved `tauri` version and pin `tauri-plugin-log` to a compatible minor — the README troubleshooting note about "Found version mismatched Tauri packages" applies here too.)

### Step 2: Initialize `tauri-plugin-log` in `run()`

- In `src-tauri/src/lib.rs`, find the builder chain at lines 920–932 (starts with `tauri::Builder::default()` and chains `.plugin(...)` calls).
- Insert a new `.plugin(...)` call **before** the existing `.plugin(tauri_plugin_opener::init())` line so the logger is the very first plugin and is available to every plugin that initializes after it (including our own `setup()` closure):

  ```rust
  .plugin(
      tauri_plugin_log::Builder::new()
          .level(if cfg!(debug_assertions) {
              log::LevelFilter::Debug
          } else {
              log::LevelFilter::Info
          })
          .targets([
              tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
              tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: None }),
              tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
          ])
          .build()
  )
  ```

  - **`Stdout`** — keeps `npm run dev` printing to the terminal, so the goal "App still produces visible logs during `tauri dev`" holds.
  - **`LogDir`** — the production target; logs are persisted under the platform log dir (on macOS: `~/Library/Logs/com.presto.app/`).
  - **`Webview`** — bridges Rust-side `log::*!` calls back into the browser DevTools console too, so during `tauri dev` you see Rust logs in the same place as JS logs. The JS plugin already routes JS logs to the Rust logger, so this also gives a single place to read all logs from DevTools.

### Step 3: Migrate the 14 Rust print calls

In `src-tauri/src/lib.rs`, replace each occurrence as follows. Use `Edit` tool, not `sed`. Mapping:

| Line | Existing                                                            | Replacement        | Rationale                                      |
| ---- | ------------------------------------------------------------------- | ------------------ | ---------------------------------------------- |
| 1119 | `eprintln!("Failed to register global shortcuts on startup: {e}");` | `log::error!(...)` | Failure path.                                  |
| 1123 | `eprintln!("Failed to load settings on startup: {e}");`             | `log::error!(...)` | Failure path.                                  |
| 1132 | `eprintln!("Failed to register default global shortcuts: {e}");`    | `log::error!(...)` | Failure path (defaults fallback also failed).  |
| 1444 | `println!("✅ Status bar visibility successfully set to: {}", …);`  | `log::info!(...)`  | Success operational message.                   |
| 1451 | `eprintln!("❌ Failed to set status bar visibility: {e}");`         | `log::error!(...)` | Failure path.                                  |
| 1514 | `println!("🔧 Carbon API: Setting SystemUIMode to {} ({})", …);`    | `log::debug!(...)` | Internal trace; not interesting at info level. |
| 1528 | `println!("✅ Carbon API: SetSystemUIMode succeeded");`             | `log::debug!(...)` | Internal trace.                                |
| 1536 | `eprintln!("❌ Carbon API: {}", error_msg);`                        | `log::error!(...)` | Failure path.                                  |
| 1549 | `eprintln!("🔄 Primary method failed, attempting fallback…");`      | `log::warn!(...)`  | Recoverable; fallback will run.                |
| 1553 | `println!("🔄 Fallback 1: Retrying after brief delay…");`           | `log::warn!(...)`  | Same — degraded path.                          |
| 1567 | `println!("✅ Fallback 1: Retry succeeded");`                       | `log::info!(...)`  | Recovery success worth surfacing.              |
| 1581 | `println!("🔄 Fallback 2: Trying conservative hide approach…");`    | `log::warn!(...)`  | Same — degraded path.                          |
| 1592 | `println!("✅ Fallback 2: Conservative approach succeeded");`       | `log::info!(...)`  | Recovery success.                              |
| 1615 | `eprintln!("❌ {}", detailed_error);`                               | `log::error!(...)` | Failure path.                                  |

Add `use log::{debug, error, info, warn};` at the top of `src-tauri/src/lib.rs` (next to the existing `use std::*` block at lines 1–14) so the macros can be invoked unqualified, OR keep them fully qualified (`log::error!`) — pick one and be consistent. The existing codebase uses fully-qualified `tauri::*`, `std::*` paths in many places, so fully qualified `log::error!` matches the surrounding style; that's what the table assumes.

Keep the emoji prefixes (`✅`, `❌`, `🔄`, `🔧`) in the message strings — they're useful visual markers in tail-the-log debugging and the issue does not ask to strip them.

### Step 4: Add the Rust lint denies

- In `src-tauri/Cargo.toml`, add to the existing `[lints.clippy]` block (which currently contains `all`, `pedantic`, `nursery`, `dbg_macro`, `todo`):
  ```toml
  print_stdout = "deny"
  print_stderr = "deny"
  ```
- Run `cd src-tauri && cargo clippy --all-targets -- -D warnings` from the repo root. If anything still uses `println!`/`eprintln!` (including any I missed in the migration table above), the build will fail at this point — that's the desired enforcement. Fix and re-run.

### Step 5: Add the JS dependency and update permissions

- In `package.json`, add to `"dependencies"`:
  ```json
  "@tauri-apps/plugin-log": "^2"
  ```
- Run `npm install` from the repo root.
- In `src-tauri/capabilities/default.json`, add `"log:default"` to the `"permissions"` array. Place it next to `"core:default"` for grouping. Without this permission, the JS plugin's IPC calls will be rejected at runtime with a "permission denied" error and logs will silently be dropped.

### Step 6: Create `src/utils/logger.js`

Create the wrapper:

```js
// Thin variadic wrapper around @tauri-apps/plugin-log.
// Lets the rest of the codebase keep console.*-style call sites
// (multi-arg, mixed string/object) while routing through the Rust logger.
import { debug, info, warn, error } from "@tauri-apps/plugin-log";

function format(args) {
  return args
    .map((a) => {
      if (typeof a === "string") return a;
      if (a instanceof Error) return `${a.message}\n${a.stack ?? ""}`;
      try {
        return JSON.stringify(a);
      } catch {
        return String(a);
      }
    })
    .join(" ");
}

const send =
  (fn) =>
  (...args) => {
    fn(format(args)).catch(() => {
      /* never let the logger throw into the app */
    });
  };

export const logger = {
  debug: send(debug),
  info: send(info),
  warn: send(warn),
  error: send(error),
};
```

Keep this file ≤40 lines. Do **not** add side effects (no `attachConsole()` here — that would re-enable a `console.*` path and conflict with the lint).

### Step 7: Wire `window.appLog` in `src/main.js`

- Add at the top of `src/main.js`, immediately after the existing imports:
  ```js
  import { logger } from "./utils/logger.js";
  window.appLog = logger;
  ```
- This must run **before** `update-manager-global.js` and `tag-manager.js` instantiate. Their classes are _defined_ synchronously by their global scripts (which load before `main.js` per `index.html` order) but their _constructors_ run later when `main.js` does `new window.UpdateManagerV2()` / instantiates `TagManager`. Setting `window.appLog` at the top of `main.js` therefore happens before any constructor that uses it.
- During Step 9 you will also migrate `main.js`'s own 79 console calls to use `logger.*` directly (the in-module form is preferred over `window.appLog` inside ES modules).

### Step 8: Migrate console.\* calls in ES module files

For each ES module file in this list — `src/utils/analytics.js`, `src/utils/common-utils.js`, `src/utils/supabase.js`, `src/utils/tag-statistics.js`, `src/utils/theme-loader.js`, `src/utils/timer-themes.js`, `src/managers/auth-manager.js`, `src/managers/navigation-manager.js`, `src/managers/session-manager.js`, `src/managers/settings-manager.js`, `src/managers/team-manager.js`, `src/components/update-notification.js`, `src/core/pomodoro-timer.js`:

- Add the import at the top: `import { logger } from "../utils/logger.js";` (or `./logger.js` if already in `src/utils/`).
- Replace every call site:

  | Before             | After                                   |
  | ------------------ | --------------------------------------- |
  | `console.log(…)`   | `logger.info(…)` _OR_ `logger.debug(…)` |
  | `console.info(…)`  | `logger.info(…)`                        |
  | `console.warn(…)`  | `logger.warn(…)`                        |
  | `console.error(…)` | `logger.error(…)`                       |
  | `console.debug(…)` | `logger.debug(…)`                       |

  **`console.log` → `info` vs `debug` decision rule:** if the comment on the line says "Debug log" (e.g. `src/main.js:39`'s `// Debug log` markers) or the message starts with `🎨`/`🔧`/`🔍` and is purely a trace, use `logger.debug`. Otherwise use `logger.info`. When in doubt, prefer `info` — production filter is at `info` so a too-debug call is invisible in prod, which is acceptable; an over-info call is at most one extra line in the log file, also acceptable. The migration does not need to be perfect on this axis; the _correctness criterion_ is "no `console.*` remaining", and any `info`-vs-`debug` polish can happen later.

- Variadic forms migrate trivially because of the wrapper:
  - `console.log("Found", n, "buttons")` → `logger.info("Found", n, "buttons")`
  - `console.error("Failed:", err)` → `logger.error("Failed:", err)` (Error instances are pretty-printed with stack via the wrapper).
  - `console.log("State:", { foo, bar })` → `logger.info("State:", { foo, bar })`.

- Do **not** convert lines inside string literals or comments. `Grep` for `console\.` first, then `Edit` each match.
  - `src/main.js` (79 calls — the largest non-`pomodoro-timer.js` file): the file already imports several utility modules; just add the `logger` import alongside.
  - `src/core/pomodoro-timer.js` (163 calls — by far the largest migration site): expect this file alone to take roughly half the migration effort. Many calls are debug-style timer state logs; default them to `logger.debug` unless they are clearly user-impacting events (state transitions, errors).

  Per-file expected count from the existing audit (use it as a checksum): `update-notification.js: 28`, `pomodoro-timer.js: 163`, `main.js: 79`, `auth-manager.js: 2`, `navigation-manager.js: 26`, `session-manager.js: 8`, `settings-manager.js: 71`, `team-manager.js: 2`, `update-manager-global.js: 64` (Step 9), `analytics.js: 2`, `common-utils.js: 9`, `supabase.js: 31`, `tag-statistics.js: 1`, `theme-loader.js: 12`, `timer-themes.js: 3`, `tag-manager.js: 13` (Step 9). After migration, `grep -rcE 'console\.(log|warn|error|info|debug)' src --include='*.js'` should return `0` per file.

### Step 9: Migrate console.\* calls in non-module global scripts

Two files cannot use `import` because they are loaded as classic scripts:

- `src/managers/update-manager-global.js` (64 calls) — replace `console.log` → `window.appLog.info` (or `.debug`), `console.warn` → `window.appLog.warn`, `console.error` → `window.appLog.error`. Same `info`-vs-`debug` rule as Step 8.
- `src/managers/tag-manager.js` (13 calls) — same pattern.

**Why `window.appLog` is safe here:**

- `index.html` loads scripts in this defer order: `update-manager-global.js` → `tag-manager.js` → `main.js`. With `defer`, all three execute after the document is parsed, in source order.
- The two non-module files **define** classes synchronously (no constructor invocation). `update-manager-global.js` does `window.UpdateManagerV2 = class { … }` at module top-level; `tag-manager.js` does `class TagManager { … }`. Neither instantiates anything itself.
- `main.js` then sets `window.appLog = logger` at the top of its module body — _before_ it instantiates either class.
- Therefore every `window.appLog.*` call inside these classes' methods/constructors runs after `window.appLog` is defined.

If `window.appLog` is somehow not yet set (e.g. a future change that instantiates these classes from inline `<script>` in `index.html`), the call will throw `Cannot read properties of undefined`. That's a louder, more debuggable failure mode than a silent dropped log — acceptable.

### Step 10: Add the `no-console` ESLint rule

- In `eslint.config.js`, inside the `rules` block, add:
  ```js
  "no-console": ["error"],
  ```
  Place it logically; the "Correctness — real bugs" group is the closest fit. Do not add `allow: [...]` exceptions — there are no JS tests in this repo and the only `console.*` references in `src/docs/` are already excluded by the top-level `ignores` array (line 6).
- Run `npm run lint`. If the migration in Steps 7–9 missed anything, this will fail with a list of locations. Fix and re-run until clean.

### Step 11: Manual smoke test in `npm run dev`

This is mandatory because the goal explicitly states "App still produces visible logs during `tauri dev`":

- Run `npm run dev`. Wait for the Tauri window to open.
- Confirm in the **terminal** (not just DevTools): you should see Rust-side log lines (e.g. on a tray-icon click or starting a session, depending on which code paths instrumented logs run) with timestamps and level prefixes (`[INFO]`, `[DEBUG]`, etc.). If you see **nothing** in the terminal, `tauri-plugin-log` is misconfigured (likely the `Stdout` target is missing).
- Open DevTools (right-click → Inspect Element). Trigger an action that logs from JS — e.g. start a Pomodoro session, click the tag dropdown. Confirm log lines appear in the terminal (forwarded by the Rust logger). They may also appear in DevTools if `Webview` target is set.
- Trigger a known-error path if available (e.g. submit invalid form, force a notification permission deny) and confirm the line shows up at `ERROR` level.
- Quit the app cleanly.
- Open the production log file location (macOS: `~/Library/Logs/com.presto.app/presto.log`). Confirm it exists and contains the lines you saw in terminal.

### Step 12: Validate

Run the validation commands in the next section. Any failure means a step above wasn't completed correctly.

## Validation Commands

Execute every command to validate the chore is complete with zero regressions. Run from the repo root unless noted.

```bash
# 1. No println!/eprintln! in Rust source (excluding tests; there are none in this repo).
git grep -nE '\b(eprintln|println)!' src-tauri/src/ \
  && { echo "FAIL: Rust prints remain"; exit 1; } \
  || echo "OK: no Rust prints"

# 2. Clippy lints catch any future regression.
grep -E 'print_stdout = "deny"' src-tauri/Cargo.toml \
  && grep -E 'print_stderr = "deny"' src-tauri/Cargo.toml \
  && echo "OK: clippy print lints denied"

# 3. tauri-plugin-log is wired up.
grep -nE 'tauri_plugin_log::Builder' src-tauri/src/lib.rs \
  && echo "OK: tauri-plugin-log builder registered"
grep -nE '"log:default"' src-tauri/capabilities/default.json \
  && echo "OK: log capability granted"

# 4. Rust build + lint + test.
cd src-tauri && cargo clippy --all-targets -- -D warnings
cd src-tauri && cargo test
cd src-tauri && cargo build
cd ..

# 5. No console.* in JS source. The eslint ignore for src/docs handles the
#    Markdown file; the regex below is restricted to the linted directories.
git grep -nE 'console\.(log|warn|error|info|debug)' \
  src/utils/ src/managers/ src/core/ src/components/ src/main.js \
  && { echo "FAIL: console calls remain"; exit 1; } \
  || echo "OK: no console.* in JS"

# 6. ESLint enforces the rule.
grep -E '"no-console"' eslint.config.js && echo "OK: no-console rule present"
npm run lint

# 7. Logger module exists and exports what we expect.
test -f src/utils/logger.js && echo "OK: logger module present"
node -e "import('./src/utils/logger.js').then(m => { \
  const ok = m.logger && ['debug','info','warn','error'].every(k => typeof m.logger[k] === 'function'); \
  process.exit(ok ? 0 : 1); \
})" \
  && echo "OK: logger surface validated" \
  || echo "NOTE: node import will fail because @tauri-apps/plugin-log is browser-only; skip if so and rely on lint+manual smoke."

# 8. The plugin dependency is installed.
node -e "require.resolve('@tauri-apps/plugin-log'); console.log('OK: plugin-log resolvable');"

# 9. Format and typecheck still pass.
npm run format
npm run typecheck

# 10. Manual smoke test (see Step 11). Cannot be automated — must be run by hand.
npm run dev
```

## Notes

- **Why Option 2 (Tauri-integrated) over Option 1 (loglevel):** the issue explicitly notes Option 2 is "better for production debugging" because all logs land in one file. The "dev dependency on Rust side initializing first" caveat from the issue does not apply to a Tauri app, where the Rust runtime is always initialized before the webview loads. The variadic-args ergonomic gap that loglevel would have given us for free is closed by the small wrapper in `src/utils/logger.js`. Net cost: ~40 lines of code in exchange for unified logs and a tail-able `presto.log` in `~/Library/Logs/com.presto.app/`.

- **Why we don't use `attachConsole()`:** `@tauri-apps/plugin-log` exports `attachConsole()` which captures `console.*` and forwards to the Rust logger. Tempting because it would be zero-migration. But it conflicts with the goal "`no-console` enforced in `eslint.config.js`" — once the lint is on, `attachConsole` has nothing to capture. So we do the explicit migration.

- **`info`/`debug`/`warn`/`error` choices are subjective.** The migration's hard requirement is "no `console.*` remaining"; the level choice for individual call sites is best-effort. Future PRs can downgrade noisy lines to `debug` without controversy. The `cfg!(debug_assertions)` filter in Step 2 plus the JS plugin's level filter mean dev builds see everything and prod builds suppress `debug` automatically — so even imperfect level choices won't spam production logs.

- **No tests exist** in this repo. The Rust crate has no `#[cfg(test)]` modules and the JS side has no test framework configured (verified — no `vitest`/`jest` in `package.json`, no `*.test.js` files). The validation surface is `cargo clippy` + `cargo test` (which compiles even with zero tests), `npm run lint` + `npm run format` + `npm run typecheck`, and the manual `npm run dev` smoke test.

- **The two non-module global scripts (`update-manager-global.js`, `tag-manager.js`) are a known wart**, called out in the existing `update-manager-global.js` file comment ("per compatibilità massima"). Converting them to ES modules is out of scope for this chore — pure logger migration only. They get `window.appLog`.

- **Out of scope:**
  - Restructuring the two non-module scripts into ES modules.
  - Adding rotation/retention policy to the log file (`tauri-plugin-log` defaults to ~10MB rotated; revisit only if it becomes a problem).
  - Touching `build-themes.js` / `debug-themes.js` (Node-side scripts not in the linted set).
  - Migrating any TypeScript files (none exist; project is JS with `checkJs: false`).

```

---
*Generated by Agentex*
```
