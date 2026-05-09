// TODO(stack-swap): this entire helper mocks the Tauri JS bridge. After the Leptos/WASM
// swap, replace with a Rust-side test harness; the public methods we drive
// (manager.loadSettings(), manager.addSession(...), etc.) are the stable contract
// this helper exists to support.
import { vi } from "vitest";

function makeDefaultInvokeImpl() {
  return (cmd) => {
    switch (cmd) {
      case "load_tasks":
        return Promise.resolve([]);
      case "load_manual_sessions":
        return Promise.resolve([]);
      case "load_settings":
        return Promise.resolve({});
      case "save_manual_sessions":
      case "save_settings":
      case "register_global_shortcuts":
        return Promise.resolve();
      case "is_autostart_enabled":
        return Promise.resolve(false);
      default:
        return Promise.reject(new Error(`Unmocked invoke command: ${cmd}`));
    }
  };
}

/**
 * Installs a fresh globalThis.__TAURI__ stub with sensible defaults. Returns the installed
 * object so tests can mutate per-call behavior via mock.core.invoke.mockImplementationOnce(...).
 * @param {{ core?: { invoke?: Function }, dialog?: object, event?: object, notification?: object }} [overrides]
 */
export function installTauriMock(overrides = {}) {
  const invokeImpl =
    typeof overrides.core?.invoke === "function" ? overrides.core.invoke : makeDefaultInvokeImpl();

  const mockInvoke = vi.fn(invokeImpl);

  const mock = {
    core: {
      invoke: mockInvoke,
    },
    dialog: {
      save: vi.fn(async () => null),
      open: vi.fn(async () => null),
      message: vi.fn(async () => {}),
      ...(overrides.dialog ?? {}),
    },
    event: {
      listen: vi.fn(async () => () => {}),
      emit: vi.fn(async () => {}),
      ...(overrides.event ?? {}),
    },
    notification: {
      isPermissionGranted: vi.fn(async () => true),
      requestPermission: vi.fn(async () => "granted"),
      sendNotification: vi.fn(),
      ...(overrides.notification ?? {}),
    },
  };

  globalThis.__TAURI__ = mock;
  return mock;
}

/**
 * Clears all mock call records and restores core.invoke to the default implementation.
 * Call in beforeEach to prevent mock state from leaking across tests.
 *
 * Does NOT replace the core.invoke fn reference, so module-level captures like
 * `const { invoke } = window.__TAURI__.core` in session-manager.js remain valid.
 */
export function resetTauriMock() {
  const tauri = globalThis.__TAURI__;
  if (!tauri) return;

  for (const ns of Object.values(tauri)) {
    if (ns && typeof ns === "object") {
      for (const fn of Object.values(ns)) {
        if (fn && typeof fn.mockClear === "function") {
          fn.mockClear();
        }
      }
    }
  }

  // Restore default implementation without swapping the fn reference.
  if (tauri.core?.invoke?.mockImplementation) {
    tauri.core.invoke.mockImplementation(makeDefaultInvokeImpl());
  }
}

/**
 * Sets core.invoke to dispatch by command name to the provided handlers map.
 * Unknown commands reject. Synchronous throws in handlers become rejected promises.
 * @param {Record<string, (args?: any) => any>} handlers
 */
export function withInvokeHandler(handlers) {
  globalThis.__TAURI__.core.invoke.mockImplementation((cmd, args) => {
    if (Object.prototype.hasOwnProperty.call(handlers, cmd)) {
      try {
        return Promise.resolve(handlers[cmd](args));
      } catch (err) {
        return Promise.reject(err);
      }
    }
    return Promise.reject(new Error(`Unmocked invoke command: ${cmd}`));
  });
}
