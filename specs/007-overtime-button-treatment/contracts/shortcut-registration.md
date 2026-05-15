# Contract — `register_global_shortcuts` (widened) + `"abort"` event payload

> No new Tauri command. No new event channel. This contract documents the **shape widening** of the existing `register_global_shortcuts` command and the **new payload value** `"abort"` on the existing `global-shortcut` event channel.

## Existing command — widened argument

**Command name**: `register_global_shortcuts` (unchanged).

**Handler**: `src-tauri/src/lib.rs:430-473` (unchanged location; loop body widened by one line).

**Argument** (today):

```rust
async fn register_global_shortcuts(
    app: AppHandle,
    shortcuts: ShortcutSettings,
) -> Result<(), BridgeError>;
```

The `ShortcutSettings` struct (`crates/presto-ipc/src/settings.rs:113-127`) widens by one optional field — see `../data-model.md` § `ShortcutSettings — extended`. The command signature's parameter type name is unchanged; the type itself grows.

**Behaviour change**: the registration loop today iterates a 3-tuple slice:

```rust
for (action, shortcut_str) in [
    ("start-stop", &shortcuts.start_stop),
    ("reset", &shortcuts.reset),
    ("skip", &shortcuts.skip),
] {
    if let Some(ref shortcut_str) = shortcut_str {
        let shortcut: Shortcut = shortcut_str.parse().map_err(…)?;
        let app_handle = app.clone();
        let action_owned = action.to_string();
        app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
            if !should_debounce_shortcut(&action_owned) {
                let _ = app_handle.emit("global-shortcut", action_owned.as_str());
            }
        }).map_err(…)?;
    }
}
```

After widening:

```rust
for (action, shortcut_str) in [
    ("start-stop", &shortcuts.start_stop),
    ("reset", &shortcuts.reset),
    ("skip", &shortcuts.skip),
    ("abort", &shortcuts.abort),  // Feature 007
] {
    /* … unchanged body … */
}
```

The `unregister_all` call at line 437 unregisters all four bindings before re-registering. The `shortcuts-updated` emit at line 467 carries the widened `ShortcutSettings` payload (the frontend listener for that channel deserialises into `Settings` and the type widening propagates).

**Return**: `Result<(), BridgeError>` — unchanged.

**Errors** — unchanged:

- `BridgeError::Internal { msg: "Failed to unregister shortcuts: …" }` if the plugin's `unregister_all` fails.
- `BridgeError::Internal { msg: "Invalid {action} shortcut '{shortcut_str}': …" }` if `s.parse::<Shortcut>()` rejects the spec. The new value of `{action}` is `"abort"` when the abort shortcut is invalid; otherwise unchanged.
- `BridgeError::Internal { msg: "Failed to register {action} shortcut: …" }` if `on_shortcut` registration fails. Same widening of `{action}`.
- `BridgeError::Internal { msg: "Failed to emit shortcuts update: …" }` if the `shortcuts-updated` emit fails.

## Existing event — new payload value

**Channel name**: `"global-shortcut"` (unchanged). Frontend-side const at `src/src/bridge/events.rs:55`: `pub const GLOBAL_SHORTCUT: &str = "global-shortcut";`.

**Payload shape**: primitive `String` (unchanged). The existing payload values are `"start-stop"`, `"reset"`, `"skip"`. Feature 007 adds `"abort"` to the set.

**Listener-side dispatch** (`src/src/app.rs:613-624`, today a no-op):

```rust
let listener = events::listen::<String>(GLOBAL_SHORTCUT, |_name| {
    // Phase 4c routes _name into the engine.
}).await;
```

After feature 007 (feature 006 did NOT wire the existing three names; feature 007 implements the full four-arm dispatch — see plan.md `[CONFIRMED]` #5). Wire names are kebab-case per `src-tauri/src/lib.rs:442-446`; matches Tauri emitter:

```rust
let listener = events::listen::<String>(GLOBAL_SHORTCUT, move |name| {
    match name.as_str() {
        "start-stop" => /* feature-006 routing */,
        "reset"      => /* feature-006 routing */,
        "skip"       => /* feature-006 routing */,
        "abort"      => /* engine.abort(clock) via the same dispatch
                          surface the UI ✕ Abort button uses */,
        _ => {}  // ignore unknown payloads — forward compatibility
    }
}).await;
```

The `_ => {}` arm is **intentional** (Principle III note): unknown payload names are silently ignored so a future feature can add a fifth name without breaking this listener. The `match` is non-exhaustive on `&str` by nature; the wildcard is the only legal exhaustive arm.

## Mock contract

`tests/e2e/fixtures/tauriMock.js:127` already accepts `register_global_shortcuts` and returns `Ok(())` without inspecting the payload. The widened argument is absorbed transparently. **No mock change required.**

The mock's `global-shortcut` event-emit surface accepts an arbitrary `String` payload. To test the new `"abort"` payload (and the newly-wired `"start-stop"`, `"reset"`, `"skip"` arms), the e2e spec calls the emit helper with each kebab-case name as the payload.

If on inspection the mock does NOT have a `global-shortcut` emit helper (i.e., feature 006 deferred event-emission testing), this feature adds one as a small mock-first step before writing `tests/e2e/timer-overtime.spec.js`'s "exit-via-Abort-keyboard" test case. The helper signature would be:

```js
// Hypothetical addition, only if absent:
mock.emit("global-shortcut", "abort");
```

## Backward compatibility

- **Pre-feature settings.json on disk**: `shortcuts.{}` lacks the `abort` key. Serde deserialises `ShortcutSettings.abort` to `None` (Option's missing-key default). The user sees the new Abort row in Settings > Shortcuts as unbound. They opt in.
- **Pre-feature `register_global_shortcuts` callers** (frontend code that constructs a `ShortcutSettings` from user input): all use the IPC-shared type, so the widening propagates at compile time. Type drift cannot occur.
- **Updater path** (Principle VII): existing presto users on the current release get the new `abort: None` default on first read. No data migration. No back-compat work.

## Test surface

| Test | Lives in | Asserts |
|---|---|---|
| `register_global_shortcuts_widened_arg_accepts_abort` | `src-tauri/src/lib.rs:#[cfg(test)] mod tests` (or equivalent integration test) | A `ShortcutSettings { …, abort: Some("CommandOrControl+Alt+W") }` is accepted by the command, registration succeeds, and the global-shortcut listener fires `"abort"` on the bound key press. |
| `register_global_shortcuts_widened_arg_skips_unbound_abort` | same | A `ShortcutSettings { …, abort: None }` is accepted, no abort binding registered, no listener fires. |
| `register_global_shortcuts_widened_arg_invalid_abort_returns_internal_error` | same | A `ShortcutSettings { …, abort: Some("not-a-shortcut") }` returns `BridgeError::Internal { msg: <contains "abort"> }`. |

These three tests are the only Tauri-bridge contract tests this feature adds. They are NOT Principle V scope (no engine state involved), but they are wire-format contract tests and benefit from RED-before-GREEN ordering by convention.
