// Mock @tauri-apps/plugin-log so logger.js doesn't try to talk to the Rust runtime.
// vi.mock is hoisted above imports — this must stay at the top level of the file.
// The real plugin functions return Promises; logger.js calls .catch() on the return value,
// so the mocks must also return a resolved Promise (not undefined).
vi.mock("@tauri-apps/plugin-log", () => ({
  debug: vi.fn(() => Promise.resolve()),
  info: vi.fn(() => Promise.resolve()),
  warn: vi.fn(() => Promise.resolve()),
  error: vi.fn(() => Promise.resolve()),
}));

// Stub window.__TAURI__ at module level so that pomodoro-timer.js can destructure
// `window.__TAURI__.core` at import time (before any beforeAll hook runs).
globalThis.__TAURI__ = {
  core: {
    invoke: vi.fn((cmd) => {
      if (cmd === "load_tasks") {
        return Promise.resolve([]);
      }
      return Promise.reject(new Error(`Unmocked invoke command: ${cmd}`));
    }),
  },
  notification: {
    isPermissionGranted: vi.fn(async () => true),
    requestPermission: vi.fn(async () => "granted"),
    sendNotification: vi.fn(),
  },
  event: {
    listen: vi.fn(async () => () => {}),
  },
};
