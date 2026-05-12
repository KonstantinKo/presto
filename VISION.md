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
(macOS / Windows / Linux). Single-user, fully local. No accounts, no sync,
no telemetry — your data never leaves the machine.

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

- Accounts, sign-in, cloud sync, telemetry — local-only is a feature.
- Multi-tenant SaaS, billing, paid tiers — this is a personal tool.
- Team / collaboration features — calendars and shared boards belong in
  other apps.
- Mobile — Tauri Mobile is technically possible but the form factor
  doesn't suit a foreground-focused timer; not a near-term goal.
- AI-assisted "smart scheduling" — the value proposition is *deterministic*
  rituals; AI heuristics undermine that.

## The completed migration

The frontend was migrated from vanilla JS + Vite to **Leptos (Rust + WASM)** with `trunk` as the build tool and `wasm-bindgen-test` for unit tests in feature 001. The migration landed as a single hard cutover with the 14-baseline visual regression suite as the safety gate.

Benefits realized:
- **Type safety end-to-end.** Tauri commands and Leptos frontend share the same type system; the IPC boundary is compile-time-checked.
- **Strict tooling.** `clippy --all-targets -- -D warnings -W clippy::pedantic`, `rustfmt`, no `--no-verify`. One language, one toolchain.
- **Smaller surface.** No more JS/TS/ESLint layer.

## Domain & constraints

- **Single-user, local data.** Tauri's app-data directory holds sessions,
  tasks, tags, settings. No network egress for user data.
- **Zero-account.** No sign-in, no profile, no guest-vs-authed
  distinction. Every install is one user, one machine.
- **Update path matters.** Existing installed users surviving every
  release is a real constraint (Tauri auto-updater); upstream
  compatibility with `murdercode/presto` is not.

## Tech stack

| Layer | Stack |
|---|---|
| Frontend | Leptos (CSR + WASM) |
| Build | Trunk |
| Frontend tests | wasm-bindgen-test |
| E2E | Playwright (chromium) |
| Visual regression | PNG baselines |
| Backend | Rust (Tauri 2.x) |

## Roadmap (rough)

In order, each as its own focused build cycle:

1. **Spec-kit retrofit** — constitution, AGENTS.md, VISION.md (this file),
   per-feature game loop. ✓ complete.
2. **Leptos migration (feature 001)** — ✓ complete. Single-PR hard cutover; visual regression as gate.
3. **Lockfile / supply-chain hardening** — pre-commit hook for
   manifest-vs-lock drift (issue #22 wontfix proposal); after-cutover this
   becomes Cargo-only.
4. **Theme system** — formalize the theme-loader code-gen path; document
   the contract for adding a theme.
5. **Mobile reconsideration** — only if Tauri Mobile reaches a point where
   the form factor + always-on requirement work; not a near-term goal.

Each step keeps the existing app fully usable; no flag-day breakages, no
"it works in main but the next release is rough".
