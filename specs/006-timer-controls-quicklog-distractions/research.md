# Phase 0 Research

**Feature**: 006-timer-controls-quicklog-distractions
**Date**: 2026-05-15

This document records only **irreversible external decisions** — choices that constrain implementation beyond what's already established in the repo. Everything else (engine signatures, manager shapes, UI structure, test ordering) is anchored in the in-tree precedents cited in `plan.md` and does not need its own research entry.

---

## Decision: Persistence pattern for `QuickLog` and `Distraction` reuses the `ManualSession` precedent verbatim.

**Decision**: Full-vec bulk re-save on every mutation, via `load_*` / `save_*` Tauri command pairs returning `Result<T, BridgeError>`. JSON file per entity in the Tauri app-data directory: `quick_logs.json`, `distractions.json`. Missing files deserialise to `Vec::new()` via `#[serde(default)]`.

**Rationale**: `ManualSession`'s implementation (`src-tauri/src/lib.rs:514-532`; manager pattern in `src/src/managers/session.rs:25-36`) is in production and exercises the same operation set the new entities need: create, update, delete, list-for-day. Reusing the pattern verbatim means zero new persistence-layer code, zero new test scaffolding categories. Per-row updates would require a per-entity diff protocol over the bridge, which Principle VI does not warrant for a list this small (single-user, single-machine, low cardinality).

**Alternatives considered**:

- **Per-row updates** (`update_quick_log(id, new_t)` / `delete_quick_log(id)`). Rejected: new IPC surface area without a problem to solve. The full-vec bulk re-save is fast enough at human-scale list lengths.
- **SQLite-backed store** (sqlx or similar). Rejected: no existing dependency, would breach Principle VII ("no upstream compatibility burden" framing inverted — adding a heavy dependency is its own burden), and would force a schema-migration story that doesn't otherwise exist in this codebase.

---

## Decision: Boundary validation reuses `BridgeError::InvalidArgument { field, reason }`.

**Decision**: FR-022 out-of-range rejections return the existing `BridgeError::InvalidArgument` variant from `crates/presto-ipc/src/error.rs:29-65`. No new `BridgeError` variant.

**Rationale**: The existing variant carries `field` and `reason` strings — enough semantic context for the frontend to surface a meaningful error (or, in the FR-019 flow, to reject before invoking — the modal already validates client-side). A new variant would be churn without payoff; Principle X "no laziness just to make the linter happy" cuts both ways — also no laziness adding types without need.

**Alternatives considered**:

- **`BridgeError::ValidationFailed { entity, field, reason }`**. Rejected: subset of `InvalidArgument` semantics; doesn't earn its variant. The existing variant suffices.

---

## Decision: `parent_ref` for Distraction is captured **by value** at modal-open time.

**Decision**: `Distraction::parent_ref: Option<DistractionParentRef>` where `DistractionParentRef` is a by-value snapshot struct (`parent_session_start_ts`, `parent_mode`, `parent_tag_id`, `parent_title`), populated at modal-open (not submit) time. Retroactive Inventory entries set this to `None`.

**Rationale**: Already pinned by spec Clarifications and Edge Cases ("modal-open time, not submit time, to avoid the race"). Captured here as an irreversible decision because the alternative (foreign key to a Session row) is structurally impossible for in-session captures — there is no persisted Session row yet for a still-running session.

**Alternatives considered**:

- **Foreign key to a Session row written on natural completion**. Rejected: requires holding the Distraction in some pending state until the session ends, plus a back-fill step at completion. Spec explicitly anchors the by-value snapshot.

---

## Decision: TR translations deferred to a follow-up i18n update.

**Decision**: New catalogue keys ship with EN, DE, IT translations. TR may fall back to EN as a placeholder in this PR.

**Rationale**: Anchored in spec Clarifications and the existing user memory (`project_i18n_targets`) — EN/DE/IT/TR is the target set for the post-005 i18n feature, but the brief explicitly defers TR if a TR-fluent contributor isn't available. Reference TR strings for the timer view live at the upstream `ramazanberkozbek/presto` fork (per user memory) if a contributor wants to pull them later.

**Alternatives considered**:

- **Block this PR on TR translations**. Rejected: stalls a P1 feature on a translation that has a clear follow-up path.
