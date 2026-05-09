// TODO(stack-swap): tests in this file that call loadSettings / saveSettings couple to the
// "load_settings" and "save_settings" Tauri command names. After the Leptos/WASM swap,
// replace those tests with equivalents targeting the new persistence API.
import { SettingsManager } from "../../src/managers/settings-manager.js";
import { resetTauriMock, withInvokeHandler } from "../setup/tauri-mock.js";

// Minimal DOM satisfying populateSettingsUI()'s required getInputById / getCheckboxById calls.
const SETTINGS_DOM = `
  <input id="start-stop-shortcut" type="text" />
  <input id="reset-shortcut" type="text" />
  <input id="skip-shortcut" type="text" />
  <input id="focus-duration" type="number" />
  <input id="break-duration" type="number" />
  <input id="long-break-duration" type="number" />
  <input id="total-sessions" type="number" />
  <input id="desktop-notifications" type="checkbox" />
  <input id="sound-notifications" type="checkbox" />
  <input id="auto-start-timer" type="checkbox" />
  <input id="allow-continuous-sessions" type="checkbox" />
  <input id="smart-pause" type="checkbox" />
  <input id="smart-pause-timeout" type="number" />
`;

describe("SettingsManager – pure functions (no DOM, no Tauri)", () => {
  let manager;

  beforeEach(() => {
    manager = new SettingsManager();
  });

  it("getDefaultSettings returns expected load-bearing values", () => {
    const defaults = manager.getDefaultSettings();
    expect(defaults.timer.focus_duration).toBe(25);
    expect(defaults.timer.break_duration).toBe(5);
    expect(defaults.timer.long_break_duration).toBe(20);
    expect(defaults.timer.total_sessions).toBe(10);
    expect(defaults.notifications.desktop_notifications).toBe(true);
    expect(defaults.notifications.smart_pause).toBe(false);
    expect(defaults.appearance.theme).toBe("auto");
    expect(defaults.appearance.timer_theme).toBe("espresso");
    expect(defaults.analytics_enabled).toBe(true);
    expect(defaults.status_bar_display).toBe("default");
  });

  it("mergeWithDefaults({}) equals getDefaultSettings()", () => {
    expect(manager.mergeWithDefaults({})).toEqual(manager.getDefaultSettings());
  });

  it("mergeWithDefaults overrides only the specified timer field", () => {
    const merged = manager.mergeWithDefaults({ timer: { focus_duration: 50 } });
    expect(merged.timer.focus_duration).toBe(50);
    expect(merged.timer.break_duration).toBe(5);
    expect(merged.timer.long_break_duration).toBe(20);
  });

  it("mergeWithDefaults overrides a top-level scalar", () => {
    const merged = manager.mergeWithDefaults({ analytics_enabled: false });
    expect(merged.analytics_enabled).toBe(false);
    expect(merged.timer.focus_duration).toBe(25);
  });
});

describe("SettingsManager – loadSettings / saveSettings (Tauri-mocked)", () => {
  let manager;

  beforeEach(() => {
    resetTauriMock();
    document.body.innerHTML = SETTINGS_DOM;
    manager = new SettingsManager();
  });

  afterEach(() => {
    // Cancel any pending auto-save timer to avoid cross-test DOM access.
    if (manager.autoSaveTimeout) {
      clearTimeout(manager.autoSaveTimeout);
      manager.autoSaveTimeout = null;
    }
  });

  it("loads settings, merges with defaults, and populates UI (happy path)", async () => {
    withInvokeHandler({
      load_settings: () => ({ timer: { focus_duration: 45 } }),
      save_settings: () => undefined,
      register_global_shortcuts: () => undefined,
      is_autostart_enabled: () => false,
    });

    await manager.loadSettings();

    expect(manager.settings.timer.focus_duration).toBe(45);
    expect(manager.settings.timer.break_duration).toBe(5);
  });

  it("falls back to default settings when invoke rejects (failure path)", async () => {
    withInvokeHandler({
      load_settings: () => {
        throw new Error("missing file");
      },
      is_autostart_enabled: () => false,
    });

    await manager.loadSettings();

    expect(manager.settings).toEqual(manager.getDefaultSettings());
  });

  it("migrates hide_status_bar to status_bar_display='icon-only'", async () => {
    withInvokeHandler({
      load_settings: () => ({ hide_status_bar: true }),
      save_settings: () => undefined,
      register_global_shortcuts: () => undefined,
      is_autostart_enabled: () => false,
    });

    await manager.loadSettings();

    expect(manager.settings.status_bar_display).toBe("icon-only");
  });

  it("serialises in-memory settings and calls save_settings (happy path)", async () => {
    // Pre-populate settings so populateSettingsUI and collectSettingsFromUI work consistently.
    withInvokeHandler({
      load_settings: () => ({}),
      save_settings: () => undefined,
      register_global_shortcuts: () => undefined,
      is_autostart_enabled: () => false,
    });
    await manager.loadSettings();

    // Reset call count so we only observe the explicit saveSettings call below.
    globalThis.__TAURI__.core.invoke.mockClear();

    await manager.saveSettings();

    expect(globalThis.__TAURI__.core.invoke).toHaveBeenCalledWith(
      "save_settings",
      expect.objectContaining({
        settings: expect.objectContaining({
          timer: expect.objectContaining({ focus_duration: 25 }),
        }),
      })
    );
  });
});
