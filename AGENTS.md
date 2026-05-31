# Agents working on presto

If you are an AI coding assistant or autonomous agent working in this
repo, read these in order:

1. `VISION.md` — what the project is and isn't, and the roadmap.
2. `CLAUDE.md` — workflow conventions and where to find things.
3. `.specify/memory/constitution.md` — the 10 enduring principles.

## Guarantees

- The constitution is normative; plans and code conform to it.
- Spec-kit feature artefacts under `specs/<NNN-feature>/` are transient
  and per-feature. Don't merge them into root-level docs.
- The visual regression baselines at
  `tests/e2e/__screenshots__/visual-regression/` are the UI contract —
  treat them like signed PDFs, not screenshots-to-update-when-convenient.

## Operational notes

- **Active branch**: `main`. Feature branches are `NNN-<slug>` (sequential
  numbering, see `specs/`).
- **Trunk dev port**: 1420 (set in `playwright.config.js` baseURL); bound
  to localhost only — Tauri config points there.
- **Tauri dev**: `cargo tauri dev` (via trunk). Do **not** run this in
  CI/agentex worktrees — it requires GUI dependencies. e2e tests use the
  trunk dev server with `tauriMock.js` instead.
- **Tauri bridge mock**: `tests/e2e/fixtures/tauriMock.js` mirrors every
  Tauri command reachable from the frontend. Adding a command means
  extending the mock first; then the test; then the real call site.
- **Bridge availability**: frontend code MUST gracefully short-circuit
  or degrade when `window.__TAURI_INTERNALS__` is absent (vite/trunk
  dev server, e2e mock context). Don't assume the bridge is always
  there.
- **IPC is `invoke()` + `listen()` only.** No custom postMessage
  protocols, no shared `window.*` globals as channels, no DOM CustomEvent
  pseudo-RPCs. New IPC mechanisms are constitution amendments, not
  per-feature decisions.

## Lints and quality gates

- **Backend**: `cargo clippy --all-targets -- -D warnings -W clippy::pedantic`
  for `src-tauri/`. `cargo fmt --check`. `cargo build --frozen`.
- **Frontend**: same `clippy --all-targets -- -D warnings
  -W clippy::pedantic` posture for the Leptos crate. `wasm-bindgen-test`
  for unit tests. `trunk build --release` on PR.
- **E2E**: `npx playwright test` against the vite (or trunk) dev server
  with `tauriMock.js`. Visual regression suite is part of this.
- **Pre-commit**: plain bash hook (`.githooks/pre-commit`, installed via
  `scripts/install-git-hooks.sh`) runs two gates: lockfile-drift and
  engine-purity. Format and clippy are CI-only (reserved to avoid slowing
  developer flow).
- **CI**: full `.agentex.yml` qa pipeline runs on every PR.

## Spec-kit gates installed in this repo

`git`, `memorylint`, `superb` (TDD enforcement + verification gate),
`qa` (acceptance-criteria validation), `architecture-guard` (constitution
drift detection), `ripple` (post-impl side-effect scan).

Per-feature game loop:

```
specify -> [clarify] -> [memorylint.load-agents] -> plan ->
[architecture-guard.violation-detection] -> tasks ->
[superb.review, architecture-guard.refactor-generator] ->
analyze -> implement (TDD-enforced by superb.tdd) ->
[qa.run -> ripple.scan -> architecture-guard.review ->
superb.verify (mandatory)] -> commit.
```

The full hook configuration is in `.specify/extensions.yml`. Slash
commands are installed under `.claude/skills/speckit-*/SKILL.md`. Use
the **hyphenated** form (`/speckit-constitution`, `/speckit-plan`); the
public docs are wrong about dots.

## Test-first commit ordering

For Principle V scope (timer engine, manager state machines, persistence
helpers, time-keeping math): **the failing-test commit precedes the
implementation commit.** A single commit that adds both the test and
the passing implementation is rejected — the diff has to show RED
first, then GREEN. UI plumbing and trivial CRUD are out of Principle V
scope and don't need this ordering; they're covered by the e2e suite
end-to-end.

## Things not to do

- **Don't write the constitution by hand.** Use `/speckit-constitution`.
  Same for spec, plan, tasks, analyze, implement.
- **Don't update visual regression baselines without explicit visual
  review.** A failing diff is either a regression or an intended visual
  change; commit it with a one-line PR note explaining why.
- **Don't add Tauri commands without extending the mock first.**
- **Don't `--no-verify`** unless the next commit fixes the bypass.

## Things you can find quickly

- **Timer state machine** — `src/src/engine/timer.rs`.
- **Manager classes** — `src/src/managers/` (Rust modules).
- **Tauri commands** — `src-tauri/src/lib.rs`.
- **Persistence helpers** — `src-tauri/src/helpers.rs`.
- **Tauri mock** — `tests/e2e/fixtures/tauriMock.js`.
- **Visual baselines** — `tests/e2e/__screenshots__/visual-regression/`.
- **CI / agentex pipeline** — `.agentex.yml`.
