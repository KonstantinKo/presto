# Contract: New Tauri Persistence Commands

**Module**: `src-tauri/src/lib.rs` (command registration) + `src-tauri/src/helpers.rs` (JSON IO).
**Pattern**: Mirrors `load_manual_sessions` / `save_manual_sessions` at `src-tauri/src/lib.rs:514-532`. Full-vec bulk re-save on every mutation.
**Touches Principles**: II. Local-Only (persistence stays on disk via Tauri commands). III. Type Safety (closed input shapes, ranged ints, boundary validation). VI. The Tauri Boundary Is Stable (typed commands via `invoke`/`listen`; `tauriMock.js` extended first per V).

All four commands are `async`, return `Result<T, BridgeError>`, and are registered in the existing `invoke_handler!` block.

Files persisted alongside existing JSON in the Tauri app-data directory:

- `quick_logs.json` (new).
- `distractions.json` (new).

Missing files deserialise to `Vec::new()` (no migration step).

---

## `save_quick_logs(quick_logs: Vec<QuickLog>) -> Result<(), BridgeError>`

### Argument shape

```rust
#[tauri::command]
async fn save_quick_logs(
    quick_logs: Vec<QuickLog>,
    app: AppHandle,
) -> Result<(), BridgeError>;
```

`QuickLog` per `data-model.md`. Serde wire shape: `camelCase` (`elapsedMinutes`, `createdAt`).

### Return shape

`Result<(), BridgeError>`. `Ok(())` on success. Errors:

- `BridgeError::InvalidArgument { field, reason }` — out-of-range validation failure (see Validation below).
- `BridgeError::Internal { msg }` — file IO failure (disk full, permissions, etc.). The `Internal { msg }` variant carries only the OS-level error message (path, syscall name, OS error code) — never the deserialised payload content. PII-scrubbing for payload-bearing errors is enforced by the existing `BridgeError::from(String)` conduit at `crates/presto-ipc/src/error.rs:72-76` (a blanket conversion to `Internal { msg }`), which receives only `format!("Failed to … : {io_error}")` strings from `helpers::write_*` — payload bytes are never passed to that formatter.

### Validation (FR-022, boundary)

For each `QuickLog` in the input vec, in order:

| Field | Rule | `field` value on failure |
|---|---|---|
| `title` | `1..=120` chars (`title.chars().count()`) | `"title"` |
| `elapsed_minutes` | `1..=720` | `"elapsedMinutes"` |
| `id` | parses via `uuid::Uuid::parse_str` | `"id"` |
| `created_at` | parses as RFC3339 / ISO-8601 | `"createdAt"` |
| `date` | matches `chrono::NaiveDate::parse_from_str(_, "%a %b %d %Y")` | `"date"` |

First failure short-circuits with `BridgeError::InvalidArgument { field, reason: <human-readable> }`. Out-of-range values are **rejected, not silently truncated** (FR-022 verbatim).

### Side effects

- Writes the full vec to `quick_logs.json` atomically. (Recommendation: write to a `.tmp` sibling, rename — matches existing `helpers.rs` write patterns.)
- No event emission.
- No engine interaction. The engine is unaware of QuickLogs.

### Mock parity

`tests/e2e/fixtures/tauriMock.js` MUST expose this command before the real handler is written. Default mock behaviour: store the vec in module-scoped state and return `Ok`. Tests may override per-spec to assert validation failures.

---

## `load_quick_logs() -> Result<Vec<QuickLog>, BridgeError>`

### Argument shape

```rust
#[tauri::command]
async fn load_quick_logs(app: AppHandle) -> Result<Vec<QuickLog>, BridgeError>;
```

### Return shape

`Result<Vec<QuickLog>, BridgeError>`.

- `Ok(vec)` — on success. Empty vec is a normal return value (e.g., file missing or empty).
- `BridgeError::Internal { msg }` — file IO failure (excluding "file not found", which returns `Ok(vec![])`).

### Side effects

- Reads `quick_logs.json` from the app-data dir. Missing file ⇒ `Ok(vec![])` (no error). Empty file or `null` body ⇒ `Ok(vec![])` via `#[serde(default)]` on the deserialise.
- No validation on read — values written by the save command are already validated. If the file has been tampered with manually and deserialise fails, return `BridgeError::Internal` with a non-PII reason.

### Mock parity

Default mock behaviour: return the module-scoped state vec (or `[]` if never set).

---

## `save_distractions(distractions: Vec<Distraction>) -> Result<(), BridgeError>`

### Argument shape

```rust
#[tauri::command]
async fn save_distractions(
    distractions: Vec<Distraction>,
    app: AppHandle,
) -> Result<(), BridgeError>;
```

`Distraction` per `data-model.md`. Includes `parent_ref: Option<DistractionParentRef>`.

### Return shape

`Result<(), BridgeError>`. Errors as for `save_quick_logs`.

### Validation (FR-022, boundary)

For each `Distraction` in the input vec, in order:

| Field | Rule | `field` value on failure |
|---|---|---|
| `note` | `1..=120` chars | `"note"` |
| `id` | parses via `uuid::Uuid::parse_str` | `"id"` |
| `created_at` | parses as RFC3339 / ISO-8601 | `"createdAt"` |
| `date` | matches `%a %b %d %Y` | `"date"` |
| `parent_ref.parent_session_start_ts` (if `parent_ref.is_some()`) | parses as RFC3339 / ISO-8601 | `"parentRef.parentSessionStartTs"` |
| `parent_ref.parent_mode` | type system (closed sum) | n/a |
| `parent_ref.parent_title` (if `Some`) | `1..=120` chars | `"parentRef.parentTitle"` |

Note: `parent_ref.parent_title` shares the title length cap. `parent_ref.parent_tag_id` has no validation beyond non-empty if `Some` (tag IDs are UUIDs in this codebase, but the contract doesn't enforce parsing here — it's a soft check).

### Side effects

- Writes the full vec to `distractions.json` atomically.
- No event emission. No engine interaction.

### Mock parity

Default mock behaviour: store vec in module-scoped state.

---

## `load_distractions() -> Result<Vec<Distraction>, BridgeError>`

### Argument shape

```rust
#[tauri::command]
async fn load_distractions(app: AppHandle) -> Result<Vec<Distraction>, BridgeError>;
```

### Return shape

`Result<Vec<Distraction>, BridgeError>`.

- `Ok(vec)` — on success. Empty vec is normal.
- `BridgeError::Internal { msg }` — file IO failure (excluding file-not-found).

### Side effects

- Reads `distractions.json`. Missing file ⇒ `Ok(vec![])`.

### Mock parity

Default mock behaviour: return module-scoped state vec.

---

## Test obligations (Principle V)

**Mock-first**: `tests/e2e/fixtures/tauriMock.js` gets the four commands before any RED test is written. Per `AGENTS.md` "Don't add Tauri commands without extending the mock first."

**RED tests** (full enumeration in `plan.md`):

- `save_quick_logs_round_trip` — save then load returns identical vec.
- `save_quick_logs_rejects_out_of_range_minutes` — `0` and `721` both rejected with `BridgeError::InvalidArgument { field: "elapsedMinutes", … }`.
- `save_quick_logs_rejects_overlong_title` — 121-char title rejected with `field: "title"`.
- `save_quick_logs_rejects_empty_title` — empty title rejected.
- `save_distractions_round_trip`.
- `save_distractions_rejects_overlong_note`.
- `save_distractions_rejects_overlong_parent_title` — when `parent_ref.parent_title` exceeds 120.
- `load_returns_empty_when_file_missing` — both commands.
- `load_handles_corrupt_file_with_bridge_error_internal` — non-JSON content yields `BridgeError::Internal` with a scrubbed reason (no PII).

---

## `BridgeError` reuse

No new `BridgeError` variants. All validation rejections use the existing `BridgeError::InvalidArgument { field: String, reason: String }` (per `crates/presto-ipc/src/error.rs:29-65`). All IO failures use `BridgeError::Internal { msg: String }`. Decision recorded in `research.md`.
