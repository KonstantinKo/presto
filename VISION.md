# presto — Vision

A modern, cross-platform Pomodoro timer for daily focused work, built as a
Tauri desktop app.

## Why

The Pomodoro Technique is one of the few productivity rituals that holds up
under scrutiny: 25 minutes on, 5 minutes off, longer break every four cycles.
It works because the timer is *external*, *deterministic*, and *not
negotiable*.

`murdercode/presto` was the most polished open-source desktop pomodoro app
out there — until the original author stopped maintaining it (the upstream
README opens with "Due to numerous commitments, the project has unfortunately
been abandoned"). This fork picks up where that left off, with the goal of
running indefinitely as a personal-grade tool that keeps pace with Tauri's
evolution and gets used every day.

## What it is

A native desktop pomodoro timer: focus / short-break / long-break cycles,
task list bound to the active session, history and weekly statistics, audio
+ system notifications, global keyboard shortcuts. Cross-platform via Tauri
(macOS / Windows / Linux). Single-user, local data. Optional Supabase sync
for accessing the same history from a second machine, opt-in.

### Core workflows

1. **Run the cycle.** Start, pause, skip, reset. Activity-monitoring smart
   pause (pauses on idle if enabled). Background-throttling-resistant
   timekeeping (the timer stays accurate when the OS suspends the
   foreground tab). Sound + system notification on transitions.
2. **Track the work.** Tasks attached to the active session, completed in
   place. Tags for context (project, area). Manual session entry for
   retroactive logging when the user worked offline.
3. **See the history.** Per-day session count, weekly view, calendar of
   completed pomodoros. Useful as a "did I actually do focused work this
   week" check.
4. **Stay updated.** Tauri auto-updater pulls signed releases. Clear
   release notes; no surprise UI changes.

### What's *not* in scope

- Multi-tenant SaaS, billing, paid tiers — this is a personal tool with
  optional cloud sync, not a product line.
- Team / collaboration features — calendars and shared boards belong in
  other apps.
- Mobile — Tauri Mobile is technically possible but the form factor
  doesn't suit a foreground-focused timer; not a near-term goal.
- AI-assisted "smart scheduling" — the value proposition is *deterministic*
  rituals; AI heuristics undermine that.

## The migration on the table

The frontend is currently vanilla JS + HTML + CSS in `src/`, built with
Vite, tested with Vitest + Playwright. The next major arc moves the
frontend to **Leptos (Rust + WASM)** with `trunk` as the build tool and
`wasm-bindgen-test` for unit tests. Rationale:

- **Type safety end-to-end.** Tauri commands already have typed Rust
  signatures. Pairing them with a Rust frontend lets the boundary be
  compile-time-checked instead of runtime-asserted.
- **Strict tooling.** `clippy --all-targets -- -D warnings -W
  clippy::pedantic`, `rustfmt`, no `--no-verify`. The codebase becomes
  pedantic-clean from day one and stays that way.
- **Smaller surface.** One language, one toolchain, one test framework.
  No more "is this an eslint problem or a tsc problem".

The migration is a **single hard cutover**: one feature, one PR, one
landing. The 14-baseline visual regression suite at
`tests/e2e/__screenshots__/visual-regression/` is the safety net —
pixel-equivalent against the current JS UI is the green-light gate. Tauri
backend (`src-tauri/`) and the Playwright e2e suite stay; only the
JS/Vite/Vitest layer is replaced.

## Domain & constraints

- **Single-user, local data.** Tauri's app-data directory holds sessions,
  tasks, tags, settings. localStorage is a bounded fallback for non-Tauri
  contexts (dev server, e2e mock).
- **Optional Supabase sync.** Guest mode is first-class; sign-in unlocks
  cross-device history but never gates the timer.
- **Optional Aptabase analytics.** Opt-in in settings, default off.
- **Update path matters.** Existing installed users surviving every
  release is a real constraint (Tauri auto-updater); upstream
  compatibility with `murdercode/presto` is not.

## Tech stack

| Layer | Today | Target |
|---|---|---|
| Frontend | Vanilla JS modules, HTML, CSS | Leptos (CSR + WASM) |
| Build | Vite | Trunk / cargo-leptos |
| Frontend tests | Vitest + happy-dom | wasm-bindgen-test |
| E2E | Playwright (chromium) | Playwright (chromium) — unchanged |
| Visual regression | 14 PNG baselines | 14 PNG baselines — unchanged |
| Backend | Rust (Tauri 2.x) | Rust (Tauri 2.x) — unchanged |
| Auth (optional) | Supabase JS SDK | Supabase Rust SDK / direct REST |
| Analytics (optional) | Aptabase (Tauri plugin) | Aptabase — unchanged |

## Roadmap (rough)

In order, each as its own focused build cycle:

1. **Spec-kit retrofit** — constitution, AGENTS.md, VISION.md (this file),
   per-feature game loop. *Currently in progress.*
2. **Leptos migration (feature 001)** — single-PR hard cutover; visual
   regression as gate.
3. **Lockfile / supply-chain hardening** — pre-commit hook for
   manifest-vs-lock drift (issue #22 wontfix proposal); after-cutover this
   becomes Cargo-only.
4. **Sync robustness** — auth-related flows, conflict resolution for
   sessions edited on two devices while offline.
5. **Theme system** — formalize the theme-loader code-gen path; document
   the contract for adding a theme.
6. **Mobile reconsideration** — only if Tauri Mobile reaches a point where
   the form factor + always-on requirement work; not a near-term goal.

Each step keeps the existing app fully usable; no flag-day breakages, no
"it works in main but the next release is rough".
