<!--
Sync Impact Report
==================
Version change: 1.0.0 → 2.0.0
Bump rationale: MAJOR bump — the Technology & Quality Constraints section is
removed (section removal = MAJOR per Governance versioning policy); several
principle bodies are materially restated to strip implementation-specifics
(tool commands, file counts, "today vs target" framings, JS-era clauses); the
"warn-now/deny-later" framing in Principle III is resolved now that the Leptos
migration has landed. Downstream PR references to "Principle III's warn-now/
deny-later" or "Principle IX's during-JS-lifetime" framings would mean
something different under the new wording; hence MAJOR.

Modified principles:
- III. Type Safety Over Defensive Code — stripped exact clippy/fmt/prettier
  invocations; replaced with "strict static analysis, warnings are errors"
  posture; removed "warn-now/deny-later" framing (migration done).
- IV. Visual Regression Is The UI Contract — removed "14 baselines" count,
  "during the Leptos migration" gate framing, and "2% pixel-ratio tolerance"
  (tool config, not domain truth); kept baseline-as-contract + update policy.
- V. Test-First For Stateful Engines — removed JS-era file paths
  (pomodoro-timer.js, tauriMock.js path); restated scope in category terms.
- VI. The Tauri Boundary Is Stable — removed "until then, JSDoc" JS-era clause;
  pruned migration-era rationale.
- IX. Lock Files Are First-Class — removed "package-lock.json (during JS
  lifetime)" framing; generalized to "committed lockfiles for all active
  package managers"; removed "after Leptos cutover" lifecycle clause.

Removed sections:
- Technology & Quality Constraints — duplicates AGENTS.md / CLAUDE.md /
  .agentex.yml. Scope, stack, and quality-gate commands live there and are
  expected to drift.

Added sections:
- (none)

Templates requiring updates:
- ✅ .specify/memory/constitution.md (this file — v2.0.0)
- ✅ .specify/templates/plan-template.md — Constitution Check III updated to
  remove "cargo clippy + cargo fmt + prettier --check" literalism; now reads
  "strict static analysis gates stay green"; IX updated to remove
  "package-lock.json" literalism; now reads "all active lockfiles".

Follow-up TODOs:
- AGENTS.md "Frontend (today)" / "Frontend (post-Leptos)" dual entries in the
  Lints section are now stale (migration landed). Flagged for a separate
  AGENTS.md cleanup task — not touched in this commit.
- CLAUDE.md "Stack" table still shows Today/Target columns; same cleanup scope.
- VISION.md "The migration on the table" section describes a future migration
  that has already landed. Flagged for a separate cleanup — not touched here.
-->

# presto Constitution

## Core Principles

### I. The Timer Is Sacred

The timer engine is the deterministic state machine that produces the user's session. Its correctness is non-negotiable.

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
- **No telemetry.** No analytics events go on the wire.
- **PII never appears in plain logs.** Identifiers (session_id, tag_id) are fine; user-typed task names, email, IP are not. Scrub at emit time, not at display time.
- **System plugins** (updater, opener, notification, dialog) run on the user's machine; they MUST NOT exfiltrate beyond what the plugin's documented behaviour requires.

Rationale: the user installed a local pomodoro timer, not a SaaS. Every network call needs a defensible reason and a user-visible toggle. PII discipline keeps GDPR posture without process.

### III. Type Safety Over Defensive Code

Use the type system; reject runtime guards the type system already excludes.

Rules:
- **Strict static analysis is non-negotiable; warnings are errors.** Tool specifics (linter invocations, flag sets) live in `.agentex.yml` and `AGENTS.md`. No blanket `#[allow]`-silencing of pedantic warnings.
- **Closed domains** (timer modes, session types, sound notification variants) are sum types — never strings or open enums.
- **Defensive validation** (null checks, "this can't happen" branches) is forbidden where the type system already excludes the case. Validate at system boundaries (Tauri command inputs, Supabase responses, file imports) only.
- **`--no-verify` is for genuine emergencies.** The next commit fixes the bypass and re-runs hooks.

Rationale: LLM-assisted coding produces working-looking code easily; strong types reject wrong-looking code at compile time, where review wouldn't catch it. The Leptos/Rust stack was chosen specifically for this leverage — we don't get the leverage if we soften the lints.

### IV. Visual Regression Is The UI Contract

The visual regression baselines define the user-facing surface. They are part of the contract.

Rules:
- Any UI change runs the visual regression suite locally before push. CI runs it on every PR.
- A failing visual diff is either a regression (fix the code) or an intended change (update the baseline AND add a one-line PR note explaining the visual change).
- Pixel tolerance and baseline count are configuration details; changes to either require explicit justification in the PR, not silent adjustment.
- Baselines are committed (chromium-linux only — that's the CI reference platform).

Rationale: a pomodoro timer's UI is small enough that "looks identical" is meaningful. The screenshots catch theme regressions, layout shifts, and accidental component swaps that pass unit tests and pass lints. They're the honest acceptance test for any stack swap.

### V. Test-First For Stateful Engines

Failing tests precede code for: the timer engine, session/tag/task persistence, activity-monitoring state, and any new core engine.

Rules:
- **Test-first applies to:** the timer engine, manager state machines (auth, session, settings, navigation, tag, team), Tauri-backed persistence helpers, time-keeping math.
- **Test-first does NOT apply to:** UI rendering, view wiring, theme loading, trivial CRUD plumbing — those are exercised by the e2e suite and visual regression.
- **Tests express behaviour the user or domain expects, not internal structure.** "Function A calls function B" assertions don't count. "After 25 minutes the timer ends in `breakReady` mode and emits `pomodoroCompleted`" does.
- **A new Tauri command extends the Tauri bridge mock first**; then the test that exercises it; then the real call site.

Rationale: timer correctness is the product. Tests catch off-by-one seconds, drift on resume, and the "right number, wrong reason" class of bug. The mock-first rule keeps e2e infrastructure honest as the backend grows.

### VI. The Tauri Boundary Is Stable

The frontend↔Rust contact surface is `@tauri-apps/api`'s `invoke()` for commands and `listen()` for events. That boundary is small and explicit on purpose.

Rules:
- **Tauri commands are typed interfaces.** A command's argument shape, return shape, and error shape are documented in the Rust-side handler and mirrored in the frontend caller. Type drift is rejected at compile time.
- **Frontend never assumes the bridge is present.** Code paths that invoke Tauri MUST gracefully degrade or short-circuit when `window.__TAURI_INTERNALS__` is unavailable (dev server, e2e mock context).
- **The Tauri bridge mock mirrors the bridge surface.** Every command reachable from the frontend exists in the mock with a default return; tests override per-spec.
- **No new IPC mechanisms** (custom postMessage, raw window globals, etc.) without a constitution amendment. `invoke`+`listen` is the channel.

Rationale: the Tauri bridge is the seam where stack swaps happen. Keeping it small, typed, and mockable is what makes a frontend replacement a one-feature operation instead of a multi-month porting saga.

### VII. No Upstream Compatibility Burden

presto was forked from the abandoned `murdercode/presto`. We do not maintain compatibility with upstream.

Rules:
- **Renames, restructures, schema migrations, removed features** are judged solely against current users — never against upstream merge considerations.
- **Updater compatibility** (existing installed users surviving the next release) IS a real consideration. The Tauri auto-updater path is part of every release.
- **Imported assets and fork-era code patterns** MAY be deleted or rewritten freely. The original upstream is dead; archaeology isn't a reason to keep code.
- **The fork itself is documented in `README.md` and `VISION.md`** — no other file needs to mention it.

Rationale: the original author abandoned the project. We don't owe them a migration path. We do owe existing presto users a working app post-update.

### VIII. Spec-Driven Feature Flow

Non-trivial features go through: spec (*what + why*) → plan (*how*) → tasks → implementation. Spec-kit is the current vehicle.

Rules:
- **Multi-file work** and any change to the timer engine, persistence layer, Tauri bridge, or auth/sync flow requires a spec under `specs/<NNN-feature>/` before implementation.
- **Trivial work** (typos, single-call refactors, dependency bumps, config tweaks, build-themes additions) does not.
- **Plans MUST reference relevant principles** by name (e.g., "I. The Timer Is Sacred — engine signature change") and pass the Constitution Check in `plan-template.md` before tasks are generated.
- **Spec-kit itself is best-so-far, not a permanent gate.** If a step is consistently unhelpful for a class of work, raise an amendment — don't silently route around it.

Rationale: captures intent and trade-offs before code is written — disproportionately valuable in an LLM-assisted workflow where implementation is cheap and disagreement-cost is high.

### IX. Lock Files Are First-Class

Committed lockfiles for all active package managers are authoritative for reproducible builds.

Rules:
- After any dependency add, remove, or version change, the regenerated lockfile MUST be staged in the same commit as the manifest change.
- **CI uses frozen/locked install commands** — never the mutable install variant. A drift between manifest and lockfile fails CI loudly.
- **Pre-commit hook**: a manifest change without a corresponding lockfile change blocks the commit.

Rationale: lockfile drift is the single most-common CI failure mode on this repo. The hook + frozen CI installs + commit-time discipline together close it.

## Governance

This constitution supersedes ad-hoc conventions. When a review comment, commit message, or PR description conflicts with a principle, the principle wins until amended.

**Amendments**: edit this file, bump version per the policy below, propagate to `.specify/templates/*` and root docs (`README.md`, `CLAUDE.md`, `AGENTS.md`, `VISION.md`). Commit message: `docs: amend constitution to vX.Y.Z (<summary>)`.

**Versioning** (SemVer for governance):
- **MAJOR**: principle removed, inverted, replaced incompatibly; governance changed incompatibly; principles renumbered; section removed.
- **MINOR**: principle/section added; existing principle materially expanded.
- **PATCH**: clarification, wording, typos, formatting.

**Compliance**: every change description (commit message or PR) SHOULD flag any principle brushed against — especially I (The Timer Is Sacred), III (Type Safety), IV (Visual Regression), and VI (Tauri Boundary).

**Runtime guidance** lives in `CLAUDE.md` and the codebase. Both MUST be reviewed for staleness on any amendment.

**Version**: 2.0.0 | **Ratified**: 2026-05-09 | **Last Amended**: 2026-05-10
