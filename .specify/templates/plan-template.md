# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]
**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

[Extract from feature spec: primary requirement + technical approach from research]

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: [e.g., Python 3.11, Swift 5.9, Rust 1.75 or NEEDS CLARIFICATION]  
**Primary Dependencies**: [e.g., FastAPI, UIKit, LLVM or NEEDS CLARIFICATION]  
**Storage**: [if applicable, e.g., PostgreSQL, CoreData, files or N/A]  
**Testing**: [e.g., pytest, XCTest, cargo test or NEEDS CLARIFICATION]  
**Target Platform**: [e.g., Linux server, iOS 15+, WASM or NEEDS CLARIFICATION]
**Project Type**: [e.g., library/cli/web-service/mobile-app/compiler/desktop-app or NEEDS CLARIFICATION]  
**Performance Goals**: [domain-specific, e.g., 1000 req/s, 10k lines/sec, 60 fps or NEEDS CLARIFICATION]  
**Constraints**: [domain-specific, e.g., <200ms p95, <100MB memory, offline-capable or NEEDS CLARIFICATION]  
**Scale/Scope**: [domain-specific, e.g., 10k users, 1M LOC, 50 screens or NEEDS CLARIFICATION]

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

For each principle below, state explicitly: **PASS** (with one-line justification),
**N/A** (with one-line reason), or **VIOLATION** (and add an entry to *Complexity
Tracking* below). Cite principle by Roman numeral + name (e.g. "I. The Timer Is Sacred").

- **I. The Timer Is Sacred** — Does this feature touch the timer engine, its event
  surface, drift compensation, or manual/retroactive session paths? If yes, confirm
  the engine remains a pure state machine and that all session paths flow through it.
- **II. Local-First, Privacy-Default** — Does this feature add network egress, new
  analytics events, or new auth-gated capability? If yes, confirm guest mode parity,
  `settings.analytics_enabled` respect, and PII scrubbing at emit time.
- **III. Type Safety Over Defensive Code** — Are new closed domains modeled as sum
  types? Is validation confined to system boundaries (Tauri inputs, Supabase
  responses, file imports)? Do the strict static analysis gates stay green without
  new blanket `#[allow]`s? (Tool specifics in `.agentex.yml` / `AGENTS.md`.)
- **IV. Visual Regression Is The UI Contract** — Does this feature change the UI?
  If yes, confirm the visual regression suite runs locally before push and that any
  baseline updates are flagged with a one-line PR note. Do NOT propose tolerance
  changes here — those require a constitution amendment.
- **V. Test-First For Stateful Engines** — Does this feature touch the timer engine,
  manager state machines, Tauri-backed persistence, or time-keeping math? If yes,
  failing tests precede implementation and assert behaviour, not internal structure.
  If a new Tauri command is added, the mock in `tests/e2e/fixtures/tauriMock.js` is
  extended first.
- **VI. The Tauri Boundary Is Stable** — Does this feature add a new Tauri command,
  event, or IPC mechanism? If yes, confirm the channel is `invoke`/`listen` (no new
  IPC), the contract is documented on the Rust handler and mirrored in the caller,
  and the mock is updated.
- **VIII. Spec-Driven Feature Flow** — Is this work non-trivial (multi-file, or
  touching the timer engine, persistence, Tauri bridge, or auth/sync flow)? If yes,
  confirm the spec exists at `specs/<NNN-feature>/spec.md` and this plan references
  the principles it brushes against by name.
- **IX. Lock Files Are First-Class** — Does this feature add or remove dependencies?
  If yes, confirm all active lockfiles will be staged in the same commit as the
  manifest change, and that CI uses frozen/locked install commands (never the mutable
  install variant).

Principles **II (Local-First)** and **VII (No Upstream Compatibility Burden)** are
typically informational for plans (privacy posture is rarely changed per-feature;
upstream compatibility is never a consideration). Cite them only when relevant.

If any principle is marked **VIOLATION**, the plan does not proceed to Phase 0 until
either the design is revised to PASS or the violation is justified in *Complexity
Tracking* with an explicit "Simpler Alternative Rejected Because" entry.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
# [REMOVE IF UNUSED] Option 1: Single project (DEFAULT)
src/
├── models/
├── services/
├── cli/
└── lib/

tests/
├── contract/
├── integration/
└── unit/

# [REMOVE IF UNUSED] Option 2: Web application (when "frontend" + "backend" detected)
backend/
├── src/
│   ├── models/
│   ├── services/
│   └── api/
└── tests/

frontend/
├── src/
│   ├── components/
│   ├── pages/
│   └── services/
└── tests/

# [REMOVE IF UNUSED] Option 3: Mobile + API (when "iOS/Android" detected)
api/
└── [same as backend above]

ios/ or android/
└── [platform-specific structure: feature modules, UI flows, platform tests]
```

**Structure Decision**: [Document the selected structure and reference the real
directories captured above]

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
