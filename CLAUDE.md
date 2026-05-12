# presto

Cross-platform Pomodoro timer built with Tauri 2.x. Single-user desktop
app. Read [VISION.md](VISION.md) for product scope.

## Where to find things

- **Constitution** — [`.specify/memory/constitution.md`](.specify/memory/constitution.md).
  9 enduring principles. Read before non-trivial work.
- **AGENTS.md** — [`AGENTS.md`](AGENTS.md). Reading order, operational
  notes, lints, gate set.
- **Spec-kit artefacts** — `.specify/` (templates, hooks, extensions);
  per-feature `specs/<NNN-slug>/`.
- **Timer engine** — `src/src/engine/timer.rs`.
- **Tauri backend** — `src-tauri/src/{lib.rs,helpers.rs,main.rs}`.
- **Tauri mock for e2e** — `tests/e2e/fixtures/tauriMock.js`.
- **Visual regression baselines** —
  `tests/e2e/__screenshots__/visual-regression/`.

## Workflow

Spec-driven via [spec-kit](https://github.com/github/spec-kit). Slash
commands are installed as Claude skills under `.claude/skills/`; use the
**hyphenated** form (the public docs are wrong about dots):

`/speckit-constitution` → `/speckit-specify` → (`/speckit-clarify`) →
`/speckit-plan` → `/speckit-tasks` → (`/speckit-analyze`) →
`/speckit-implement`. Plus `/speckit-git-*` for branch/commit/PR.

The per-feature game loop (with installed gates) is in
[AGENTS.md](AGENTS.md). The full PM playbook is the
`/manage-feature` skill.

Trivial work (typos, single-call refactors, dependency bumps, config
tweaks) doesn't need a spec. Multi-file work, anything touching the
timer engine, persistence, or the Tauri bridge does.

## Stack

| Layer | Stack |
|---|---|
| Frontend | Leptos (CSR + WASM) + Trunk |
| Frontend tests | wasm-bindgen-test |
| Backend | Rust (Tauri 2.x) |
| E2E | Playwright (chromium) |

Single-user, fully local Pomodoro timer. No accounts, no sync, no
telemetry. The only outbound traffic is the auto-updater's poll for new
releases (no user data on the wire).

## Conventions

- **Heavy type safety.** `cargo clippy --all-targets -- -D warnings -W
  clippy::pedantic` for both backend and frontend crates. `cargo fmt`.
- **Test-first** for the timer engine, manager state machines, and
  Tauri-backed persistence. Not required for UI plumbing — that's the
  e2e suite's job.
- **Visual regression is the UI contract.** Pixel-equivalent (within 2%
  per `playwright.config.js`) against the baselines is the green light.
  A baseline update needs an explicit one-line PR note.
- **Lock files are first-class.** `package.json` ↔ `package-lock.json`
  drift fails CI; same for `Cargo.toml` ↔ `Cargo.lock`. CI uses
  `npm ci` and `cargo build --frozen` — never `npm install` or
  `cargo build`.
- **Comments**: explain WHY, not WHAT. Default to none. Triggers:
  non-obvious invariant, drift compensation, Tauri-platform quirk,
  principle-aware `#[allow]` justification.
- **No upstream compatibility burden.** Forked from `murdercode/presto`,
  upstream abandoned. Renames, restructures, breaking changes need only
  consider current users (via the Tauri auto-updater path), not
  upstream merges.
- **No `--no-verify`** except in genuine emergencies; the next commit
  fixes the bypass.

## Active plan

<!-- SPECKIT START -->
Active plan: [`specs/001-leptos-migration/plan.md`](specs/001-leptos-migration/plan.md)
— Leptos frontend migration (CSR + WASM via Trunk; Tauri backend unchanged).
<!-- SPECKIT END -->
