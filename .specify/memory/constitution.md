<!--
Sync Impact Report
==================
Version change: (none) → 1.0.0
Bump rationale: Initial ratification. No prior versioned constitution exists; the
file previously contained only template placeholders. Per SemVer-for-governance,
the first concrete adoption is 1.0.0 (not a MAJOR bump from a non-existent baseline).

Modified principles:
- (none — initial adoption)

Added principles:
- I. The Timer Is Sacred
- II. Local-First, Privacy-Default
- III. Type Safety Over Defensive Code
- IV. Visual Regression Is The UI Contract
- V. Test-First For Stateful Engines
- VI. The Tauri Boundary Is Stable
- VII. No Upstream Compatibility Burden
- VIII. Spec-Driven Feature Flow
- IX. Lock Files Are First-Class

Added sections:
- Technology & Quality Constraints
- Governance (with explicit amendment + versioning + compliance rules)

Removed sections:
- (none — template placeholders replaced wholesale)

Templates requiring updates:
- ✅ .specify/memory/constitution.md (this file — placeholders replaced)
- ✅ .specify/templates/plan-template.md (Constitution Check gates concretized
  to reference Principles I, III, IV, V, VI, VIII, IX by name)
- ✅ .specify/templates/spec-template.md (no constitution-coupled sections; reviewed,
  no changes needed — spec scope is product behaviour, principles surface in plan)
- ✅ .specify/templates/tasks-template.md (no constitution-coupled sections; reviewed,
  no changes needed — task categorisation is generic and stack-agnostic)
- ✅ .specify/templates/checklist-template.md (no constitution-coupled sections; reviewed,
  no changes needed — checklist is per-feature and content-driven)

Follow-up TODOs:
- AGENTS.md does not yet exist at the repo root. The memorylint pre-hook surfaced
  this as informational. AGENTS.md will be authored as part of the spec-kit retrofit
  cycle (see Principle VIII and the broader retrofit task), at which point
  Principle III (lints), Principle IX (lockfile discipline), and the `.agentex.yml`
  CI stages should be cross-referenced from AGENTS.md to keep runtime guidance
  in sync with this constitution.
- Runtime guidance docs (README.md, CLAUDE.md, VISION.md) are referenced in the
  Governance section but are not modified by this initial ratification. They will
  be aligned as part of the same retrofit commit set per the user's commit-batching
  instruction.
-->

# presto Constitution

## Core Principles

### I. The Timer Is Sacred

`src/core/pomodoro-timer.js` (and its successor in the post-migration stack) is the deterministic state machine that produces the user's session. Its correctness is non-negotiable.

Rules:
- The timer engine MUST be a pure state machine: given the same inputs (mode, durations, activity stream, system clock), it produces the same outputs. No hidden globals, no surprise side effects.
- Wall-clock drift compensation (background-throttling correction, smart-pause activity detection, max-session capping) is part of the contract — covered by tests, not patched at the call site.
- The engine emits events; UI consumes them. The engine never reads from the DOM.
- Manual session entry, retroactive logging, and "what really happened" reconciliation MUST go through the same engine path as live sessions, not bypass it.

Rationale: this is a tool people use to spend their time deliberately. A wrong-by-one-second timer or a swallowed pause silently undoes the user's effort. The state machine is small enough to be obviously correct; "obviously correct" is the bar.

### II. Local-First, Privacy-Default

presto is a single-user desktop app. Tauri's app-data directory is the authoritative store. Network egress is opt-in.

Rules:
- All session, task, tag, and settings state MUST persist locally via Tauri commands first; localStorage is the bounded fallback for pure-web contexts (e2e dev server, mocked Tauri bridge).
- **Auth (Supabase) is optional.** Guest mode is first-class — every feature reachable in guest mode is reachable, full-stop. Sign-in unlocks sync; never gates core timer functionality.
- **Analytics (Aptabase) is opt-in via `settings.analytics_enabled`.** Default off. Respect the toggle at every call site; no "but-this-event-is-anonymous" carve-outs.
- **PII never appears in plain logs or analytics events.** Identifiers (session_id, tag_id) are fine; user-typed task names, email, IP are not. Scrub at event-emit time, not at display time.
- **Updater + opener + notification + dialog plugins** run on the user's machine; they MUST NOT exfiltrate beyond what the plugin's documented behaviour requires.

Rationale: the user installed a local pomodoro timer, not a SaaS. Every network call needs a defensible reason and a user-visible toggle. PII discipline keeps GDPR posture without process.

### III. Type Safety Over Defensive Code

Modern presto runs on Rust + Leptos (frontend) and Rust (Tauri backend). Use the type system; reject runtime guards the type system already excludes.

Rules:
- **Frontend lints**: `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` for the Leptos crate. Pedantic warnings are visible (not blanket-`#[allow]`-silenced). Promotion of pedantic to deny is a future amendment; the codebase is pedantic-clean today.
- **Backend lints**: `cargo clippy --all-targets -- -D warnings` for `src-tauri/`. Same pedantic posture as frontend.
- **Formatting**: `cargo fmt --check` for both crates; `prettier --check` for non-Rust files (CSS, MD, JSON, YAML).
- **Closed domains** (timer modes, session types, sound notification variants) are sum types — never strings or open enums.
- **Defensive validation** (null checks, "this can't happen" branches) is forbidden where the type system already excludes the case. Validate at system boundaries (Tauri command inputs, Supabase responses, file imports) only.
- **`--no-verify` is for genuine emergencies.** The next commit fixes the bypass and re-runs hooks.

Rationale: LLM-assisted coding produces working-looking code easily; strong types reject wrong-looking code at compile time, where review wouldn't catch it. The Leptos/Rust pivot was chosen specifically for this leverage — we don't get the leverage if we soften the lints.

### IV. Visual Regression Is The UI Contract

The 14 baseline PNGs in `tests/e2e/__screenshots__/visual-regression/` define the user-facing surface. They are part of the contract.

Rules:
- Any UI change runs the visual regression suite locally before push. CI runs it on every PR.
- A failing visual diff is either a regression (fix the code) or an intended change (update the baseline AND add a one-line PR note explaining the visual change).
- Tolerance is 2% pixel-ratio (`playwright.config.js`). Tightening or loosening that tolerance is a constitution amendment, not a one-PR decision.
- **During the Leptos migration, the visual regression suite is the green-light gate.** Pixel-equivalent against the JS baselines is what makes the rewrite verifiable. Migrations that propose baseline rewrites are migrations that lost the safety net.
- Baselines are committed (chromium-linux only — that's the CI reference platform).

Rationale: a pomodoro timer's UI is small enough that "looks identical" is meaningful. The screenshots catch theme regressions, layout shifts, and accidental component swaps that pass unit tests and pass eslint. For a stack swap, they're the only honest acceptance test.

### V. Test-First For Stateful Engines

Failing tests precede code for: the timer engine, session/tag/task persistence, activity-monitoring state, and any new core engine.

Rules:
- **Test-first applies to:** `pomodoro-timer.js` successor, manager state machines (auth, session, settings, navigation, tag, team), Tauri-backed persistence helpers, time-keeping math.
- **Test-first does NOT apply to:** UI rendering, view wiring, theme loading, trivial CRUD plumbing — those are exercised by the e2e suite and visual regression.
- **Tests express behaviour the user or domain expects, not internal structure.** "Function A calls function B" assertions don't count. "After 25 minutes the timer ends in `breakReady` mode and emits `pomodoroCompleted`" does.
- **The Tauri mock in `tests/e2e/fixtures/tauriMock.js` is a test surface, not production code.** Adding a Tauri command means extending the mock first; then the test that exercises it; then the real call site.

Rationale: timer correctness is the product. Tests catch off-by-one seconds, drift on resume, and the "right number, wrong reason" class of bug. The mock-first rule keeps e2e infrastructure honest as the backend grows.

### VI. The Tauri Boundary Is Stable

The JS↔Rust contact surface is `@tauri-apps/api`'s `invoke()` for commands and `listen()` for events. That boundary is small and explicit on purpose.

Rules:
- **Tauri commands are typed sum-type interfaces.** A command's argument shape, return shape, and error shape are documented in the Rust-side handler and mirrored in the frontend caller. Drift is rejected at compile time post-Leptos-migration; until then, JSDoc on the call site.
- **Frontend never assumes the bridge is present.** Code paths that invoke Tauri MUST gracefully degrade or short-circuit when `window.__TAURI_INTERNALS__` is unavailable (e.g., pure Vite dev server, e2e with mock).
- **`tauriMock.js` mirrors the bridge surface.** Every command reachable from the frontend exists in the mock with a default return; tests override per-spec.
- **No new IPC mechanisms** (custom postMessage, raw window globals, etc.) without a constitution amendment. `invoke`+`listen` is the channel.

Rationale: the Tauri bridge is the seam where the rewrite happens. Keeping it small, typed, and mockable is what makes the JS→Leptos swap a one-feature operation instead of a multi-month porting saga.

### VII. No Upstream Compatibility Burden

presto was forked from the abandoned `murdercode/presto`. We do not maintain compatibility with upstream.

Rules:
- **Renames, restructures, schema migrations, removed features** are judged solely against current users — never against upstream merge considerations.
- **Updater compatibility** (existing installed users surviving the next release) IS a real consideration. The Tauri auto-updater path is part of every release.
- **Imported assets** (`art/`, fork-era code patterns) MAY be deleted or rewritten freely. The original upstream is dead; archaeology isn't a reason to keep code.
- **The fork itself is documented in `README.md` and `VISION.md`** — no other file needs to mention it.

Rationale: the original author abandoned the project. We don't owe them a migration path. We do owe existing presto users a working app post-update.

### VIII. Spec-Driven Feature Flow

Non-trivial features go through: spec (*what + why*) → plan (*how*) → tasks → implementation. Spec-kit is the current vehicle.

Rules:
- **Multi-file work** and any change to the timer engine, persistence layer, Tauri bridge, or auth/sync flow requires a spec under `specs/<NNN-feature>/` before implementation.
- **Trivial work** (typos, single-call refactors, dependency bumps, config tweaks, build-themes additions) does not.
- **Plans MUST reference relevant principles** by name (e.g., "I. The Timer Is Sacred — engine signature change") and pass the Constitution Check in `plan-template.md` before tasks are generated.
- **Spec-kit gates installed** are: git, memorylint, superb, qa, architecture-guard, ripple. The per-feature game loop is documented in `.specify/extensions.yml`.
- **Spec-kit itself is best-so-far, not a permanent gate.** If a step is consistently unhelpful for a class of work, raise an amendment — don't silently route around it.

Rationale: captures intent and trade-offs before code is written — disproportionately valuable in an LLM-assisted workflow where implementation is cheap and disagreement-cost is high.

### IX. Lock Files Are First-Class

`package-lock.json` (during JS lifetime) and `Cargo.lock` are committed artefacts and authoritative for reproducible builds.

Rules:
- After any `npm install` / `cargo add` / dep removal, the regenerated lock MUST be staged in the same commit as the manifest change.
- **CI runs `npm ci` and `cargo build --frozen`** — never `npm install` or `cargo build` (which mutate locks). A drift between manifest and lock fails CI loudly.
- **Pre-commit hook**: a `package.json` change without a corresponding `package-lock.json` change blocks the commit. Same for `Cargo.toml` ↔ `Cargo.lock`.
- After Leptos cutover, `package-lock.json` is removed. `Cargo.lock` becomes the single lock file.

Rationale: lockfile drift is the single most-common CI failure mode on this repo (3 occurrences in the test-infrastructure cycle alone — issue #22 documents the pattern). The hook + `npm ci` + commit-time discipline together close it.

## Technology & Quality Constraints

- **Stack today** (pre-migration): Vanilla JS managers + vite + vitest + happy-dom; Rust (Tauri 2.x) backend; Playwright for e2e + visual regression.
- **Stack target** (post-migration): Leptos (CSR + WASM) + trunk + wasm-bindgen-test; Rust (Tauri 2.x) backend unchanged; Playwright e2e + visual regression suite preserved.
- **Quality gates**: `.agentex.yml` defines `setup`/`test`/`lint`/`format` stages. CI runs the full pipeline; husky pre-commit runs format + cheap lints + lockfile-drift check on touched files.
- **Comments**: explain WHY, not WHAT. Default to none. Triggers: non-obvious invariant, drift compensation rationale, Tauri-platform quirk, principle-aware `#[allow]` justification.
- **Scope**: single-user desktop app, no server backend (Supabase is a thin sync layer, not the source of truth). Multi-user, web-only mode, paid tiers — out of scope until re-evaluated.
- **Dependencies**: prefer adding none. New runtime deps need a one-line justification in the introducing commit or spec.

## Governance

This constitution supersedes ad-hoc conventions. When a review comment, commit message, or PR description conflicts with a principle, the principle wins until amended.

**Amendments**: edit this file, bump version per the policy below, propagate to `.specify/templates/*` and root docs (`README.md`, `CLAUDE.md`, `AGENTS.md`, `VISION.md`). Commit message: `docs: amend constitution to vX.Y.Z (<summary>)`.

**Versioning** (SemVer for governance):
- **MAJOR**: principle removed, inverted, replaced incompatibly; governance changed incompatibly; principles renumbered.
- **MINOR**: principle/section added; existing principle materially expanded.
- **PATCH**: clarification, wording, typos, formatting.

**Compliance**: every change description (commit message or PR) SHOULD flag any principle brushed against — especially I (The Timer Is Sacred), III (Type Safety), IV (Visual Regression), and VI (Tauri Boundary).

**Runtime guidance** lives in `CLAUDE.md` and the codebase. Both MUST be reviewed for staleness on any amendment.

**Version**: 1.0.0 | **Ratified**: 2026-05-09 | **Last Amended**: 2026-05-09
