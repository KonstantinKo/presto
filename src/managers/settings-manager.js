/** @param {string} cmd @param {any} [args] @returns {Promise<any>} */
const invoke = (cmd, args) => {
  const tauriInvoke = window.__TAURI__?.core?.invoke;
  if (!tauriInvoke) {
    return Promise.reject(new Error("Tauri bridge not available"));
  }
  return tauriInvoke(cmd, args);
};
import { NotificationUtils } from "../utils/common-utils.js";
import { logger } from "../utils/logger.js";
import { toError } from "../utils/to-error.js";
import {
  getThemeById,
  getAllThemes,
  getCompatibleThemes,
  isThemeCompatible,
} from "../utils/timer-themes.js";
import { initializeAutoThemeLoader } from "../utils/theme-loader.js";

/**
 * @param {string} id
 * @returns {HTMLInputElement}
 */
function getInputById(id) {
  const el = document.getElementById(id);
  if (!el) {
    throw new Error(`Missing element: ${id}`);
  }
  if (!(el instanceof HTMLInputElement)) {
    throw new Error(`Element with id ${id} is not an HTMLInputElement`);
  }
  return el;
}

/**
 * @param {string} id
 * @returns {HTMLInputElement}
 */
function getCheckboxById(id) {
  const el = document.getElementById(id);
  if (!el) {
    throw new Error(`Missing element: ${id}`);
  }
  if (!(el instanceof HTMLInputElement)) {
    throw new Error(`Element with id ${id} is not an HTMLInputElement`);
  }
  if (el.type !== "checkbox") {
    throw new Error(`Element with id ${id} is not an input[type=checkbox]`);
  }
  return el;
}

export class SettingsManager {
  constructor() {
    /** @type {any} */
    this.settings = null;
    this.isRecordingShortcut = false;
    this.currentRecordingField = null;
    /** @type {string[]} */
    this.recordedKeys = [];
    this.autoSaveTimeout = null;
    this.autoSaveDelay = 1000; // 1 second delay for auto-save
  }

  async init() {
    this.cleanupOldNotificationElements();
    await this.loadSettings();
    await this.initializeAutoThemeLoader();

    this.setupEventListeners();
    await this.registerGlobalShortcuts();
    this.setupGlobalShortcutHandlers();
    this.setupSettingsNavigation();
    await this.initializeTheme();
    await this.initializeTimerTheme();
  }

  async initializeAutoThemeLoader() {
    try {
      logger.debug("🎨 Starting auto theme discovery...");
      const loadedThemes = await initializeAutoThemeLoader();
      logger.debug(`🎨 Auto-loaded ${loadedThemes.length} themes:`, loadedThemes);

      // Refresh theme selector if it exists and settings are loaded
      if (document.getElementById("timer-theme-grid") && this.settings) {
        this.initializeTimerThemeSelector();
      }

      return loadedThemes;
    } catch (error) {
      logger.error("❌ Failed to initialize auto theme loader:", error);
      return [];
    }
  }

  cleanupOldNotificationElements() {
    const oldFeedback = document.getElementById("auto-save-feedback");
    if (oldFeedback) {
      oldFeedback.remove();
    }
  }

  async loadSettings() {
    try {
      const loadedSettings = /** @type {any} */ (await invoke("load_settings"));
      logger.debug("📋 Raw loaded settings:", loadedSettings);
      this.settings = this.mergeWithDefaults(loadedSettings);

      // Migrate old hide_status_bar setting to new status_bar_display setting
      if (
        loadedSettings.hide_status_bar !== undefined &&
        loadedSettings.status_bar_display === undefined
      ) {
        this.settings.status_bar_display = loadedSettings.hide_status_bar ? "icon-only" : "default";
        // Schedule save to persist the migrated setting
        this.scheduleAutoSave();
        logger.debug(
          "🔄 Migrated hide_status_bar setting to status_bar_display:",
          this.settings.status_bar_display
        );
      }

      logger.debug("📋 Final merged settings:", this.settings);
      this.populateSettingsUI();
    } catch (error) {
      logger.error("Failed to load settings:", error);
      this.settings = this.getDefaultSettings();
      this.populateSettingsUI();
    }
  }

  /** @param {any} loadedSettings */
  mergeWithDefaults(loadedSettings) {
    const defaultSettings = this.getDefaultSettings();

    return {
      shortcuts: { ...defaultSettings.shortcuts, ...loadedSettings.shortcuts },
      timer: { ...defaultSettings.timer, ...loadedSettings.timer },
      notifications: { ...defaultSettings.notifications, ...loadedSettings.notifications },
      appearance: { ...defaultSettings.appearance, ...loadedSettings.appearance },
      advanced: { ...defaultSettings.advanced, ...loadedSettings.advanced },
      autostart: loadedSettings.autostart ?? defaultSettings.autostart,
      analytics_enabled: loadedSettings.analytics_enabled ?? defaultSettings.analytics_enabled,
      hide_icon_on_close: loadedSettings.hide_icon_on_close ?? defaultSettings.hide_icon_on_close,
      status_bar_display: loadedSettings.status_bar_display ?? defaultSettings.status_bar_display,
    };
  }

  getDefaultSettings() {
    return {
      shortcuts: {
        start_stop: "CommandOrControl+Alt+Space",
        reset: "CommandOrControl+Alt+R", // Delete Session (focus) / Undo (break)
        skip: "CommandOrControl+Alt+S", // Save Session
      },
      timer: {
        focus_duration: 25,
        break_duration: 5,
        long_break_duration: 20,
        total_sessions: 10,
        weekly_goal_minutes: 125,
        max_session_time: 120, // 2 hours in minutes
      },
      notifications: {
        desktop_notifications: true,
        sound_notifications: true,
        auto_start_timer: true, // Renamed from auto_start_breaks
        allow_continuous_sessions: false, // Allow sessions to continue beyond timer
        smart_pause: false,
        smart_pause_timeout: 30, // default 30 seconds
      },
      appearance: {
        theme: "auto", // auto, light, dark
        timer_theme: "espresso",
      },
      advanced: {
        debug_mode: false, // Debug mode with 3-second timers
      },
      autostart: false,
      analytics_enabled: true,
      hide_icon_on_close: false,
      status_bar_display: "default", // Status bar display mode: 'default' or 'icon-only'
    };
  }

  populateSettingsUI() {
    logger.debug("🔧 Populating shortcuts UI with:", this.settings.shortcuts);
    getInputById("start-stop-shortcut").value = this.settings.shortcuts.start_stop || "";
    getInputById("reset-shortcut").value = this.settings.shortcuts.reset || "";
    getInputById("skip-shortcut").value = this.settings.shortcuts.skip || "";

    getInputById("focus-duration").value = String(this.settings.timer.focus_duration);
    getInputById("break-duration").value = String(this.settings.timer.break_duration);
    getInputById("long-break-duration").value = String(this.settings.timer.long_break_duration);
    getInputById("total-sessions").value = String(this.settings.timer.total_sessions);

    const maxSessionTimeField = /** @type {HTMLInputElement | null} */ (
      document.getElementById("max-session-time")
    );
    if (maxSessionTimeField) {
      maxSessionTimeField.value = String(this.settings.timer.max_session_time || 120);
    }

    const weeklyGoalField = /** @type {HTMLInputElement | null} */ (
      document.getElementById("weekly-goal-minutes")
    );
    if (weeklyGoalField) {
      weeklyGoalField.value = String(this.settings.timer.weekly_goal_minutes || 125);
    }

    const themeSelect = /** @type {HTMLSelectElement | null} */ (
      document.getElementById("theme-select")
    );
    if (themeSelect) {
      themeSelect.value = this.settings.appearance?.theme || "auto";
    }

    this.initializeThemeSelector();
    this.initializeTimerThemeSelector();

    // Always show the user's setting preference, regardless of system permission
    getCheckboxById("desktop-notifications").checked =
      this.settings.notifications.desktop_notifications;
    getCheckboxById("sound-notifications").checked =
      this.settings.notifications.sound_notifications;
    getCheckboxById("auto-start-timer").checked = this.settings.notifications.auto_start_timer;

    logger.debug(
      "🔧 PopulateUI - Raw continuous sessions value:",
      this.settings.notifications.allow_continuous_sessions
    );
    const continuousValue = this.settings.notifications.allow_continuous_sessions || false;
    logger.debug("🔧 PopulateUI - Final continuous sessions value:", continuousValue);

    getCheckboxById("allow-continuous-sessions").checked = continuousValue;
    getCheckboxById("smart-pause").checked = this.settings.notifications.smart_pause;

    const timeoutValue = this.settings.notifications.smart_pause_timeout || 30;
    getInputById("smart-pause-timeout").value = String(timeoutValue);
    const timeoutDisplay = document.getElementById("timeout-value");
    if (timeoutDisplay) {
      timeoutDisplay.textContent = String(timeoutValue);
    }

    this.toggleTimeoutSetting(this.settings.notifications.smart_pause);
    this.setupSliderEventListener();

    const debugModeCheckbox = /** @type {HTMLInputElement | null} */ (
      document.getElementById("debug-mode")
    );
    if (debugModeCheckbox) {
      debugModeCheckbox.checked = this.settings.advanced?.debug_mode || false;
    }

    this.loadAutostartSetting();
    this.loadAnalyticsSetting();
    this.loadHideIconOnCloseSetting();
    this.loadStatusBarDisplaySetting();
  }

  setupEventListeners() {
    const shortcutInputs = document.querySelectorAll(".shortcut-input");
    shortcutInputs.forEach((input) => {
      input.addEventListener("click", (e) => this.startRecordingShortcut(e.target));
      input.addEventListener("keydown", (e) => this.handleShortcutKeydown(e));
      input.addEventListener("blur", () => this.stopRecordingShortcut());
    });

    window.addEventListener("keydown", (e) => {
      if (this.isRecordingShortcut) {
        this.handleShortcutKeydown(e);
      }
    });

    const smartPauseCheckbox = document.getElementById("smart-pause");
    if (smartPauseCheckbox) {
      smartPauseCheckbox.addEventListener("change", async (e) => {
        const checkbox = /** @type {HTMLInputElement} */ (e.target);
        const checked = checkbox.checked;
        this.toggleTimeoutSetting(checked);

        if (window.pomodoroTimer) {
          try {
            await window.pomodoroTimer.enableSmartPause(checked);
            window.pomodoroTimer.updateSettingIndicators();
            this.scheduleAutoSave();
          } catch (error) {
            logger.error("Failed to apply smart pause setting:", error);
            checkbox.checked = !checked;
            this.toggleTimeoutSetting(!checked);
          }
        } else {
          this.scheduleAutoSave();
        }
      });
    }

    const continuousSessionsCheckbox = document.getElementById("allow-continuous-sessions");
    if (continuousSessionsCheckbox) {
      continuousSessionsCheckbox.addEventListener("change", async (e) => {
        const checkbox = /** @type {HTMLInputElement} */ (e.target);
        const checked = checkbox.checked;

        if (window.pomodoroTimer) {
          try {
            await window.pomodoroTimer.enableContinuousSessions(checked);
            window.pomodoroTimer.updateSettingIndicators();
            this.scheduleAutoSave();
          } catch (error) {
            logger.error("Failed to apply continuous sessions setting:", error);
            checkbox.checked = !checked;
          }
        } else {
          this.scheduleAutoSave();
        }
      });
    }

    const autoStartCheckbox = document.getElementById("auto-start-timer");
    if (autoStartCheckbox) {
      autoStartCheckbox.addEventListener("change", async (e) => {
        const checkbox = /** @type {HTMLInputElement} */ (e.target);
        const checked = checkbox.checked;

        if (window.pomodoroTimer) {
          try {
            await window.pomodoroTimer.enableAutoStart(checked);
            window.pomodoroTimer.updateSettingIndicators();
            this.scheduleAutoSave();
          } catch (error) {
            logger.error("Failed to apply auto-start setting:", error);
            checkbox.checked = !checked;
          }
        } else {
          this.scheduleAutoSave();
        }
      });
    }

    this.setupAutoSaveListeners();
  }

  /** @param {boolean} enabled */
  toggleTimeoutSetting(enabled) {
    const timeoutSetting = document.getElementById("smart-pause-timeout-setting");
    timeoutSetting?.classList.toggle("visible", enabled);
  }

  setupGlobalShortcutHandlers() {
    // Debounce mechanism to prevent repeated triggering
    const lastShortcutTime = /** @type {Record<string, any>} */ ({});
    const debounceDelay = 500; // 500ms debounce

    window.__TAURI__?.event?.listen("global-shortcut", (event) => {
      const action = event.payload;
      const now = Date.now();

      if (lastShortcutTime[action] && now - lastShortcutTime[action] < debounceDelay) {
        logger.debug(`Debounced global shortcut: ${action}`);
        return;
      }

      lastShortcutTime[action] = now;
      logger.debug(`Global shortcut triggered: ${action}`);

      switch (action) {
        case "start-stop":
          if (window.pomodoroTimer) {
            if (
              window.pomodoroTimer.isRunning &&
              !window.pomodoroTimer.isPaused &&
              !window.pomodoroTimer.isAutoPaused
            ) {
              window.pomodoroTimer.pauseTimer();
            } else {
              window.pomodoroTimer.startTimer();
            }
          }
          break;
        case "reset":
          if (window.pomodoroTimer) {
            if (window.pomodoroTimer.currentMode === "focus") {
              window.pomodoroTimer.resetTimer();
            } else {
              window.pomodoroTimer.undoLastSession();
            }
          }
          break;
        case "skip":
          if (window.pomodoroTimer) {
            window.pomodoroTimer.skipSession();
          }
          break;
      }
    });

    window.__TAURI__?.event?.listen("shortcuts-updated", (event) => {
      logger.info("Shortcuts updated:", event.payload);
      this.settings.shortcuts = event.payload;

      if (window.pomodoroTimer) {
        window.pomodoroTimer.updateKeyboardShortcuts(this.settings.shortcuts);
      }
    });
  }

  /** @param {any} input */
  startRecordingShortcut(input) {
    if (this.isRecordingShortcut) {
      return;
    }

    this.isRecordingShortcut = true;
    this.currentRecordingField = input;
    this.recordedKeys = [];

    input.classList.add("recording");
    input.value = "Press keys...";
    input.focus();
  }

  stopRecordingShortcut() {
    if (!this.isRecordingShortcut) {
      return;
    }

    this.isRecordingShortcut = false;

    if (this.currentRecordingField) {
      this.currentRecordingField.classList.remove("recording");

      if (this.recordedKeys.length > 0) {
        const shortcut = this.formatShortcut(this.recordedKeys);
        this.currentRecordingField.value = shortcut;
      } else {
        this.currentRecordingField.value = "";
      }
    }

    this.currentRecordingField = null;
    this.recordedKeys = [];
  }

  /** @param {any} e */
  handleShortcutKeydown(e) {
    if (!this.isRecordingShortcut) {
      return;
    }

    e.preventDefault();
    e.stopPropagation();

    const key = e.key;
    const modifiers = [];

    if (e.metaKey || e.ctrlKey) {
      modifiers.push("CommandOrControl");
    }
    if (e.altKey) {
      modifiers.push("Alt");
    }
    if (e.shiftKey) {
      modifiers.push("Shift");
    }

    // Don't record modifier keys alone
    if (["Meta", "Control", "Alt", "Shift"].includes(key)) {
      return;
    }

    this.recordedKeys = [...modifiers, key];

    if (this.currentRecordingField) {
      this.currentRecordingField.value = this.formatShortcut(this.recordedKeys);
    }

    // Auto-finish recording after a short delay
    setTimeout(() => {
      this.stopRecordingShortcut();
      this.scheduleAutoSave();
    }, 500);
  }

  /** @param {any} keys */
  formatShortcut(keys) {
    return keys.join("+");
  }

  /**
   * @param {string} value
   * @param {number} fallback
   * @returns {number}
   */
  parseNumberOrDefault(value, fallback) {
    const n = parseInt(value, 10);
    return Number.isFinite(n) ? n : fallback;
  }

  collectSettingsFromUI() {
    this.settings.shortcuts.start_stop = getInputById("start-stop-shortcut").value || null;
    this.settings.shortcuts.reset = getInputById("reset-shortcut").value || null;
    this.settings.shortcuts.skip = getInputById("skip-shortcut").value || null;

    this.settings.timer.focus_duration = this.parseNumberOrDefault(
      getInputById("focus-duration").value,
      this.settings.timer.focus_duration
    );
    this.settings.timer.break_duration = this.parseNumberOrDefault(
      getInputById("break-duration").value,
      this.settings.timer.break_duration
    );
    this.settings.timer.long_break_duration = this.parseNumberOrDefault(
      getInputById("long-break-duration").value,
      this.settings.timer.long_break_duration
    );
    this.settings.timer.total_sessions = this.parseNumberOrDefault(
      getInputById("total-sessions").value,
      this.settings.timer.total_sessions
    );

    const maxSessionTimeField = /** @type {HTMLInputElement | null} */ (
      document.getElementById("max-session-time")
    );
    if (maxSessionTimeField) {
      this.settings.timer.max_session_time = this.parseNumberOrDefault(
        maxSessionTimeField.value,
        this.settings.timer.max_session_time
      );
    }

    const weeklyGoalField = /** @type {HTMLInputElement | null} */ (
      document.getElementById("weekly-goal-minutes")
    );
    if (weeklyGoalField) {
      this.settings.timer.weekly_goal_minutes = this.parseNumberOrDefault(
        weeklyGoalField.value,
        this.settings.timer.weekly_goal_minutes
      );
    }

    const themeSelect = /** @type {HTMLSelectElement | null} */ (
      document.getElementById("theme-select")
    );
    if (themeSelect) {
      this.settings.appearance.theme = themeSelect.value;
    }

    this.settings.notifications.desktop_notifications =
      getCheckboxById("desktop-notifications").checked;
    this.settings.notifications.sound_notifications =
      getCheckboxById("sound-notifications").checked;
    this.settings.notifications.auto_start_timer = getCheckboxById("auto-start-timer").checked;
    this.settings.notifications.allow_continuous_sessions = getCheckboxById(
      "allow-continuous-sessions"
    ).checked;
    this.settings.notifications.smart_pause = getCheckboxById("smart-pause").checked;
    this.settings.notifications.smart_pause_timeout = this.parseNumberOrDefault(
      getInputById("smart-pause-timeout").value,
      30
    );

    const debugModeCheckbox = /** @type {HTMLInputElement | null} */ (
      document.getElementById("debug-mode")
    );
    if (debugModeCheckbox) {
      if (!this.settings.advanced) {
        this.settings.advanced = {};
      }
      this.settings.advanced.debug_mode = debugModeCheckbox.checked;
    }
  }

  async saveSettings() {
    try {
      this.collectSettingsFromUI();

      if (document.getElementById("theme-select")) {
        await this.applyTheme(this.settings.appearance.theme);
      }

      await this.applyTimerTheme(this.settings.appearance.timer_theme);

      await invoke("save_settings", { settings: this.settings });
      await this.registerGlobalShortcuts();

      if (window.pomodoroTimer) {
        await window.pomodoroTimer.applySettings(this.settings);

        // If smart pause is active and countdown is running, restart it with new timeout
        if (
          window.pomodoroTimer.smartPauseEnabled &&
          window.pomodoroTimer.smartPauseCountdownInterval &&
          window.pomodoroTimer.currentMode === "focus" &&
          window.pomodoroTimer.isRunning
        ) {
          window.pomodoroTimer.handleUserActivity();
        }
      }

      NotificationUtils.showNotificationPing("✓ Settings saved successfully!", "success");
    } catch (error) {
      logger.error("Failed to save settings:", error);
      NotificationUtils.showNotificationPing("❌ Failed to save settings", "error");
    }
  }

  async registerGlobalShortcuts() {
    try {
      logger.debug("🔧 Registering global shortcuts:", this.settings.shortcuts);
      await invoke("register_global_shortcuts", { shortcuts: this.settings.shortcuts });
    } catch (error) {
      logger.error("Failed to register global shortcuts:", error);
    }
  }

  resetToDefaults() {
    if (confirm("Are you sure you want to reset all settings to defaults?")) {
      this.settings = this.getDefaultSettings();
      this.populateSettingsUI();
      this.saveSettings();
    }
  }

  resetToDefaultsForce() {
    this.settings = this.getDefaultSettings();
    this.populateSettingsUI();
    // Don't save here since we're doing a complete reset
  }

  /** @param {any} shortcutType */
  clearShortcut(shortcutType) {
    const inputId = `${shortcutType}-shortcut`;
    const input = /** @type {HTMLInputElement | null} */ (document.getElementById(inputId));
    if (input) {
      input.value = "";
      this.scheduleAutoSave();
    }
  }

  setupSliderEventListener() {
    const slider = /** @type {any} */ (document.getElementById("smart-pause-timeout"));
    const valueDisplay = document.getElementById("timeout-value");

    if (slider && valueDisplay && !slider._sliderListenerAttached) {
      slider._sliderListenerAttached = true;
      slider.addEventListener("input", (/** @type {any} */ e) => {
        valueDisplay.textContent = e.target.value;
        this.scheduleAutoSave();
      });
    }
  }

  setupAutoSaveListeners() {
    const timerFields = [
      "focus-duration",
      "break-duration",
      "long-break-duration",
      "total-sessions",
      "weekly-goal-minutes",
      "max-session-time",
    ];

    const appearanceFields = ["theme-select"];

    timerFields.forEach((fieldId) => {
      const field = document.getElementById(fieldId);
      if (field) {
        field.addEventListener("change", () => this.scheduleAutoSave());
        field.addEventListener("input", () => this.scheduleAutoSave());
      }
    });

    appearanceFields.forEach((fieldId) => {
      const field = /** @type {HTMLSelectElement | null} */ (document.getElementById(fieldId));
      if (field) {
        if (fieldId === "theme-select") {
          // Handle theme changes specially to apply theme immediately
          // applyTheme() already saves settings, so no need to scheduleAutoSave
          field.addEventListener("change", async () => {
            await this.applyTheme(field.value);
          });
        } else {
          field.addEventListener("change", () => this.scheduleAutoSave());
        }
      }
    });

    // Handle desktop notifications checkbox separately (requires permission request)
    const desktopNotificationsCheckbox = document.getElementById("desktop-notifications");
    if (desktopNotificationsCheckbox) {
      desktopNotificationsCheckbox.addEventListener("change", async (e) => {
        if (/** @type {any} */ (e.target).checked) {
          try {
            logger.info("🔔 Desktop notifications enabled, requesting permission...");
            const permission = await NotificationUtils.requestNotificationPermission();
            logger.info("🔔 Notification permission result:", permission);

            if (permission !== "granted") {
              // Show warning but don't prevent saving the setting
              const message =
                permission === "unsupported"
                  ? "Desktop notifications are not supported in this browser."
                  : "Notification permission denied. Settings saved, but notifications won't work until permission is granted.";
              NotificationUtils.showNotificationPing(message, "warning");
              // Don't uncheck the box - let the user's choice be saved
            } else {
              NotificationUtils.showNotificationPing("✓ Desktop notifications enabled!", "success");
            }
          } catch (error) {
            logger.warn(
              "Failed to request notification permission, but allowing setting to be saved:",
              error
            );
            // Don't prevent the setting from being saved even if permission request fails
            // This allows the setting to work when Tauri notifications are properly configured
            NotificationUtils.showNotificationPing(
              "Settings saved. Notifications will work when properly configured.",
              "info"
            );
          }
        } else {
          logger.info("🔔 Desktop notifications disabled");
          NotificationUtils.showNotificationPing("Desktop notifications disabled", "info");
        }
        // Always save the setting regardless of permission status
        this.scheduleAutoSave();
      });
    }

    this.setupNotificationStatusDisplay();

    const checkboxFields = ["sound-notifications", "debug-mode"];

    checkboxFields.forEach((fieldId) => {
      const field = document.getElementById(fieldId);
      if (field) {
        // Skip desktop-notifications as it has special handling above
        if (fieldId !== "desktop-notifications") {
          field.addEventListener("change", () => this.scheduleAutoSave());
        }
      }
    });

    // Smart pause timeout slider is already handled in setupSliderEventListener
  }

  scheduleAutoSave() {
    if (this.autoSaveTimeout) {
      clearTimeout(this.autoSaveTimeout);
    }

    this.autoSaveTimeout = setTimeout(() => {
      this.autoSaveSettings();
    }, this.autoSaveDelay);
  }

  async autoSaveSettings() {
    try {
      this.collectSettingsFromUI();

      logger.debug("🔧 AutoSave - Reading checkbox values:");
      logger.debug("auto_start_timer checkbox:", getCheckboxById("auto-start-timer").checked);
      logger.debug(
        "allow_continuous_sessions checkbox:",
        getCheckboxById("allow-continuous-sessions").checked
      );
      logger.debug("smart_pause checkbox:", getCheckboxById("smart-pause").checked);
      logger.debug("🔧 AutoSave - Full settings object being saved:", this.settings);

      await invoke("save_settings", { settings: this.settings });
      await this.registerGlobalShortcuts();

      if (window.pomodoroTimer) {
        await window.pomodoroTimer.applySettings(this.settings);

        // If smart pause is active and countdown is running, restart it with new timeout
        if (
          window.pomodoroTimer.smartPauseEnabled &&
          window.pomodoroTimer.smartPauseCountdownInterval &&
          window.pomodoroTimer.currentMode === "focus" &&
          window.pomodoroTimer.isRunning
        ) {
          window.pomodoroTimer.handleUserActivity();
        }
      }

      this.showAutoSaveFeedback();
    } catch (error) {
      logger.error("Failed to auto-save settings:", error);
    }
  }

  showAutoSaveFeedback() {
    NotificationUtils.showNotificationPing("✓ Settings saved", "success");
  }

  setupSettingsNavigation() {
    const navItems = document.querySelectorAll(".settings-nav-item");
    const categories = document.querySelectorAll(".settings-category");

    navItems.forEach((item) => {
      item.addEventListener("click", () => {
        const targetCategory = /** @type {HTMLElement} */ (item).dataset.category;

        navItems.forEach((nav) => nav.classList.remove("active"));
        categories.forEach((cat) => cat.classList.remove("active"));

        item.classList.add("active");
        const targetElement = document.getElementById(`category-${targetCategory}`);
        if (targetElement) {
          targetElement.classList.add("active");
        }
      });
    });
  }

  async loadAutostartSetting() {
    const checkbox = /** @type {any} */ (document.getElementById("autostart-enabled"));

    if (checkbox && !checkbox._autostartHandlerBound) {
      checkbox._autostartHandlerBound = true;
      checkbox.addEventListener("change", async (/** @type {any} */ e) => {
        await this.toggleAutostart(e.target.checked);
      });
    }

    try {
      const isEnabled = await invoke("is_autostart_enabled");
      this.settings.autostart = isEnabled;
      if (checkbox) {
        checkbox.checked = isEnabled;
      }
    } catch (error) {
      logger.error("Failed to check autostart status:", error);
      if (checkbox) {
        checkbox.checked = false;
      }
    }
  }

  /** @param {any} enabled */
  async toggleAutostart(enabled) {
    try {
      if (enabled) {
        await invoke("enable_autostart");
        logger.info("Autostart enabled");
        NotificationUtils.showNotificationPing(
          "✓ Autostart enabled - Presto will start with your system",
          "success"
        );
      } else {
        await invoke("disable_autostart");
        logger.info("Autostart disabled");
        NotificationUtils.showNotificationPing("✓ Autostart disabled", "success");
      }

      this.settings.autostart = enabled;
      this.scheduleAutoSave();
    } catch (error) {
      logger.error("Failed to toggle autostart:", error);
      NotificationUtils.showNotificationPing(`❌ Failed to toggle autostart: ${error}`, "error");

      const checkbox = /** @type {HTMLInputElement | null} */ (
        document.getElementById("autostart-enabled")
      );
      if (checkbox) {
        checkbox.checked = !enabled;
      }
    }
  }

  loadAnalyticsSetting() {
    const checkbox = /** @type {any} */ (document.getElementById("analytics-enabled"));
    if (!checkbox) {
      return;
    }
    checkbox.checked = this.settings.analytics_enabled;

    if (!checkbox._analyticsHandlerBound) {
      checkbox._analyticsHandlerBound = true;
      checkbox.addEventListener("change", async (/** @type {any} */ e) => {
        await this.toggleAnalytics(e.target.checked);
      });
    }
  }

  /** @param {any} enabled */
  async toggleAnalytics(enabled) {
    try {
      this.settings.analytics_enabled = enabled;

      if (enabled) {
        logger.info("Analytics enabled");
        NotificationUtils.showNotificationPing(
          "✓ Analytics enabled - Help improve Presto!",
          "success"
        );
      } else {
        logger.info("Analytics disabled");
        NotificationUtils.showNotificationPing(
          "✓ Analytics disabled - No data will be collected",
          "success"
        );
      }

      this.scheduleAutoSave();
    } catch (error) {
      logger.error("Failed to toggle analytics:", error);
      NotificationUtils.showNotificationPing(`❌ Failed to toggle analytics: ${error}`, "error");

      const checkbox = /** @type {HTMLInputElement | null} */ (
        document.getElementById("analytics-enabled")
      );
      if (checkbox) {
        checkbox.checked = !enabled;
      }
    }
  }

  loadHideIconOnCloseSetting() {
    const checkbox = /** @type {any} */ (document.getElementById("hide-icon-on-close"));
    if (!checkbox) {
      return;
    }
    checkbox.checked = this.settings.hide_icon_on_close;

    if (!checkbox._hideIconHandlerBound) {
      checkbox._hideIconHandlerBound = true;
      checkbox.addEventListener("change", async (/** @type {any} */ e) => {
        await this.toggleHideIconOnClose(e.target.checked);
      });
    }
  }

  /** @param {any} enabled */
  async toggleHideIconOnClose(enabled) {
    try {
      this.settings.hide_icon_on_close = enabled;

      if (enabled) {
        logger.info("Hide icon on close enabled");
        NotificationUtils.showNotificationPing(
          "✓ Hide icon on close enabled - App will hide from dock when closed",
          "success"
        );
      } else {
        logger.info("Hide icon on close disabled");
        NotificationUtils.showNotificationPing(
          "✓ Hide icon on close disabled - App will remain visible in dock",
          "success"
        );
      }

      this.scheduleAutoSave();
    } catch (error) {
      logger.error("Failed to toggle hide icon on close:", error);
      NotificationUtils.showNotificationPing(
        `❌ Failed to toggle hide icon on close: ${error}`,
        "error"
      );

      const checkbox = /** @type {HTMLInputElement | null} */ (
        document.getElementById("hide-icon-on-close")
      );
      if (checkbox) {
        checkbox.checked = !enabled;
      }
    }
  }

  loadStatusBarDisplaySetting() {
    const select = /** @type {any} */ (document.getElementById("status-bar-display"));
    if (!select) {
      return;
    }
    select.value = this.settings.status_bar_display || "default";

    if (!select._statusBarHandlerBound) {
      select._statusBarHandlerBound = true;
      select.addEventListener("change", async (/** @type {any} */ e) => {
        await this.updateStatusBarDisplay(e.target.value);
      });
    }
  }

  /** @param {any} displayMode */
  async updateStatusBarDisplay(displayMode) {
    try {
      this.settings.status_bar_display = displayMode;

      if (window.pomodoroTimer) {
        await window.pomodoroTimer.updateTrayIcon();
      }

      if (displayMode === "icon-only") {
        logger.info("Status bar display set to icon only");
        NotificationUtils.showNotificationPing("✓ Status bar will show icon only", "success");
      } else {
        logger.info("Status bar display set to default (mm:ss)");
        NotificationUtils.showNotificationPing("✓ Status bar will show timer (mm:ss)", "success");
      }

      this.scheduleAutoSave();
    } catch (error) {
      logger.error("Failed to update status bar display:", error);
      NotificationUtils.showNotificationPing(
        `❌ Failed to update status bar display: ${error}`,
        "error"
      );

      const select = /** @type {HTMLSelectElement | null} */ (
        document.getElementById("status-bar-display")
      );
      if (select) {
        select.value = this.settings.status_bar_display || "default";
      }
    }
  }

  async setupNotificationStatusDisplay() {
    const statusDiv = document.getElementById("notification-status");
    const statusText = document.getElementById("notification-status-text");
    const testBtn = document.getElementById("test-notifications-btn");

    if (!statusDiv || !statusText || !testBtn) {
      logger.warn("Notification status elements not found in DOM");
      return;
    }

    statusDiv.style.display = "block";
    await this.updateNotificationStatus();

    testBtn.addEventListener("click", async () => {
      if (window.pomodoroTimer && typeof window.pomodoroTimer.testNotification === "function") {
        await window.pomodoroTimer.testNotification();
        setTimeout(() => this.updateNotificationStatus(), 1000);
      } else {
        logger.warn("Test notification function not available");
        NotificationUtils.showNotificationPing(
          "Test function not available. Try again after the timer is fully loaded.",
          "warning"
        );
      }
    });

    // Update status when desktop notifications setting changes
    const desktopNotificationsCheckbox = document.getElementById("desktop-notifications");
    if (desktopNotificationsCheckbox) {
      desktopNotificationsCheckbox.addEventListener("change", () => {
        setTimeout(() => this.updateNotificationStatus(), 500);
      });
    }
  }

  async updateNotificationStatus() {
    const statusDiv = document.getElementById("notification-status");
    const statusText = document.getElementById("notification-status-text");

    if (!statusDiv || !statusText) {
      return;
    }

    let status = "";
    let className = "";

    try {
      const isDevMode = window.location.protocol === "tauri:" ? false : true;

      const isEnabledInSettings =
        /** @type {HTMLInputElement | null} */ (document.getElementById("desktop-notifications"))
          ?.checked || false;

      if (!isEnabledInSettings) {
        status = "🔕 Disabled in settings";
        className = "status-disabled";
      } else {
        // Check Tauri notifications first
        if (window.__TAURI__ && window.__TAURI__.notification) {
          try {
            const { isPermissionGranted } = window.__TAURI__.notification;
            const granted = await isPermissionGranted();
            if (granted) {
              if (isDevMode) {
                status = "⚠️ Dev mode - may not work on macOS";
                className = "status-warning";
              } else {
                status = "✅ Tauri notifications ready";
                className = "status-ready";
              }
            } else {
              status = "⚠️ Tauri permission needed";
              className = "status-warning";
            }
          } catch (error) {
            status = `❌ Tauri error: ${toError(error).message}`;
            className = "status-error";
          }
        } else {
          // Check Web Notification API
          if ("Notification" in window) {
            const permission = Notification.permission;
            if (permission === "granted") {
              status = "✅ Web notifications ready";
              className = "status-ready";
            } else if (permission === "denied") {
              status = "❌ Web notifications blocked";
              className = "status-error";
            } else {
              status = "⚠️ Web permission needed";
              className = "status-warning";
            }
          } else {
            status = "❌ Notifications not supported";
            className = "status-error";
          }
        }
      }
    } catch (error) {
      status = "❌ Status check failed";
      className = "status-error";
      logger.error("Failed to check notification status:", error);
    }

    statusText.textContent = status;
    statusDiv.className = `notification-status ${className}`;
  }

  /** @param {any} theme */
  async applyTheme(theme) {
    const html = document.documentElement;

    html.removeAttribute("data-theme");

    let actualTheme = theme;
    if (theme === "auto") {
      actualTheme = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
      logger.debug(`🎨 Auto theme detected system preference: ${actualTheme}`);
    }

    html.setAttribute("data-theme", actualTheme);

    localStorage.setItem("theme-preference", theme); // Store user preference (could be "auto")

    // Update settings object and save immediately to prevent loss on app close
    if (this.settings && this.settings.appearance) {
      this.settings.appearance.theme = theme; // Store user preference (could be "auto")
      try {
        await invoke("save_settings", { settings: this.settings });
        logger.debug(`🎨 Theme preference saved: ${theme}, actual theme applied: ${actualTheme}`);
      } catch (error) {
        logger.error("Failed to save theme setting:", error);
      }
    }

    if (theme === "auto") {
      this.setupSystemThemeListener();
    } else {
      this.removeSystemThemeListener();
    }

    logger.debug(`🎨 Theme preference: ${theme}, actual theme applied: ${actualTheme}`);

    // Update timer theme compatibility when color mode changes
    this.updateTimerThemeCompatibility();
  }

  setupSystemThemeListener() {
    // Remove existing listener if any
    this.removeSystemThemeListener();

    // Create new listener
    this.systemThemeMediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    this.systemThemeListener = (/** @type {any} */ e) => {
      const newSystemTheme = e.matches ? "dark" : "light";
      logger.debug(`🎨 System theme changed: ${newSystemTheme}`);

      // Only apply if current preference is "auto"
      const currentPreference = this.settings?.appearance?.theme || "auto";
      if (currentPreference === "auto") {
        const html = document.documentElement;
        html.setAttribute("data-theme", newSystemTheme);
        logger.debug(`🎨 Auto theme updated to: ${newSystemTheme}`);

        // Update timer theme compatibility when system theme changes
        this.updateTimerThemeCompatibility();
      }
    };

    this.systemThemeMediaQuery.addEventListener("change", this.systemThemeListener);
  }

  removeSystemThemeListener() {
    if (this.systemThemeMediaQuery && this.systemThemeListener) {
      this.systemThemeMediaQuery.removeEventListener("change", this.systemThemeListener);
      this.systemThemeMediaQuery = null;
      this.systemThemeListener = null;
    }
  }

  async initializeTheme() {
    // Check if theme was already initialized early
    const currentTheme = document.documentElement.getAttribute("data-theme");
    const storedTheme = localStorage.getItem("theme-preference");

    // If early theme was set and matches localStorage, check if we need to process it
    if (currentTheme && storedTheme && currentTheme === storedTheme) {
      logger.debug(`🎨 Early initialized theme found: ${currentTheme}`);

      // If the stored theme is "auto", we need to apply the correct auto logic
      // because data-theme should never be "auto" - it should be "light" or "dark"
      if (storedTheme === "auto") {
        logger.debug(`🎨 Converting auto theme to actual theme`);
        await this.applyTheme("auto"); // This will set data-theme to light/dark
        return;
      }

      // For non-auto themes, keep the early initialization
      logger.debug(`🎨 Keeping early initialized theme: ${currentTheme}`);

      // Update settings to match current theme
      if (this.settings && this.settings.appearance) {
        this.settings.appearance.theme = currentTheme;
        try {
          await invoke("save_settings", { settings: this.settings });
          logger.debug(`🎨 Settings updated to match current theme: ${currentTheme}`);
        } catch (error) {
          logger.error("Failed to update theme in settings:", error);
        }
      }
      return;
    }

    // Otherwise apply the theme from settings or default to auto
    const theme = this.settings?.appearance?.theme || "auto";
    await this.applyTheme(theme);
  }

  initializeThemeSelector() {
    const themeSelector = document.getElementById("theme-selector");
    const themeSelect = /** @type {HTMLSelectElement | null} */ (
      document.getElementById("theme-select")
    );

    if (!themeSelector) {
      return;
    }

    const currentTheme = this.settings.appearance?.theme || "auto";

    // Set active theme button
    this.updateThemeSelector(currentTheme);

    // Add event listeners to theme buttons
    const themeButtons = themeSelector.querySelectorAll(".theme-option");
    themeButtons.forEach((button) => {
      button.addEventListener("click", async (_e) => {
        const selectedTheme = button.getAttribute("data-theme");
        if (!selectedTheme) {
          logger.error("Theme option is missing data-theme");
          return;
        }

        // Update visual state
        this.updateThemeSelector(selectedTheme);

        // Update hidden select for compatibility
        if (themeSelect) {
          themeSelect.value = selectedTheme;
        }

        // Apply theme immediately
        this.settings.appearance.theme = selectedTheme;
        await this.applyTheme(selectedTheme);

        // Save settings
        try {
          await this.saveSettings();
          logger.debug(`🎨 Theme changed to: ${selectedTheme}`);
        } catch (error) {
          logger.error("Failed to save theme setting:", error);
        }
      });
    });
  }

  /** @param {any} theme */
  updateThemeSelector(theme) {
    const themeSelector = document.getElementById("theme-selector");
    if (!themeSelector) {
      return;
    }

    // Remove active class from all buttons
    const themeButtons = themeSelector.querySelectorAll(".theme-option");
    themeButtons.forEach((button) => {
      button.classList.remove("active");
    });

    // Add active class to selected button
    const activeButton = themeSelector.querySelector(`[data-theme="${theme}"]`);
    if (activeButton) {
      activeButton.classList.add("active");
    }
  }

  getCurrentColorMode() {
    // data-theme is always "light" or "dark" (never "auto") at this point.
    return document.documentElement.getAttribute("data-theme") === "dark" ? "dark" : "light";
  }

  async initializeTimerTheme() {
    // Check if timer theme was already initialized early
    const currentTimerTheme = document.documentElement.getAttribute("data-timer-theme");
    const storedTimerTheme = localStorage.getItem("timer-theme-preference");

    // If early timer theme was set and matches localStorage, keep it
    if (currentTimerTheme && storedTimerTheme && currentTimerTheme === storedTimerTheme) {
      logger.debug(`🎨 Keeping early initialized timer theme: ${currentTimerTheme}`);

      // Update settings to match current timer theme
      if (this.settings && this.settings.appearance) {
        this.settings.appearance.timer_theme = currentTimerTheme;
        try {
          await invoke("save_settings", { settings: this.settings });
          logger.debug(`🎨 Settings updated to match current timer theme: ${currentTimerTheme}`);
        } catch (error) {
          logger.error("Failed to update timer theme in settings:", error);
        }
      }
      return;
    }

    const timerTheme = this.settings?.appearance?.timer_theme || "espresso";
    await this.applyTimerTheme(timerTheme);
  }

  /** @param {any} themeId */
  async applyTimerTheme(themeId) {
    const html = document.documentElement;

    html.setAttribute("data-timer-theme", themeId);

    localStorage.setItem("timer-theme-preference", themeId);

    if (this.settings && this.settings.appearance) {
      this.settings.appearance.timer_theme = themeId;
    }

    logger.debug(`🎨 Timer theme applied: ${themeId}`);
    logger.debug(
      `🎨 DOM attribute check: data-timer-theme="${html.getAttribute("data-timer-theme")}"`
    );

    const computedStyle = getComputedStyle(html);
    logger.debug(`🎨 CSS Variables check:`, {
      focusColor: computedStyle.getPropertyValue("--focus-color").trim(),
      focusBg: computedStyle.getPropertyValue("--focus-bg").trim(),
      focusTimerColor: computedStyle.getPropertyValue("--focus-timer-color").trim(),
    });

    html.style.display = "none";
    // eslint-disable-next-line no-unused-expressions -- intentional layout trigger
    html.offsetHeight;
    html.style.display = "";
  }

  initializeTimerThemeSelector() {
    const timerThemeGrid = document.getElementById("timer-theme-grid");
    if (!timerThemeGrid || !this.settings || !this.settings.appearance) {
      return; // Exit early if elements or settings not ready
    }

    const currentColorMode = this.getCurrentColorMode();
    const currentTimerTheme = this.settings.appearance?.timer_theme || "espresso";

    timerThemeGrid.innerHTML = "";

    const themes = getAllThemes();

    themes.forEach((theme) => {
      const themeOption = this.createTimerThemeOption(theme, currentTimerTheme, currentColorMode);
      timerThemeGrid.appendChild(themeOption);
    });
  }

  /** @param {any} theme @param {any} currentTimerTheme @param {any} currentColorMode */
  createTimerThemeOption(theme, currentTimerTheme, currentColorMode) {
    const option = document.createElement("div");
    option.className = "timer-theme-option";
    option.setAttribute("data-timer-theme", theme.id);

    const isCompatible = isThemeCompatible(theme.id, currentColorMode);
    const isActive = theme.id === currentTimerTheme;

    if (isActive) {
      option.classList.add("active");
    }
    if (!isCompatible) {
      option.classList.add("disabled");
    }

    const header = document.createElement("div");
    header.className = "timer-theme-header";

    const nameEl = document.createElement("h4");
    nameEl.className = "timer-theme-name";
    nameEl.textContent = theme.name;

    const compatibility = document.createElement("div");
    compatibility.className = "timer-theme-compatibility";

    const allowedModes = ["light", "dark"];
    for (const mode of theme.supports) {
      if (!allowedModes.includes(mode)) {
        continue;
      }
      const badge = document.createElement("span");
      badge.className = `compatibility-badge ${mode}`;
      const icon = document.createElement("i");
      icon.className = `ri-${mode === "light" ? "sun" : "moon"}-line`;
      badge.appendChild(icon);
      compatibility.appendChild(badge);
    }

    header.appendChild(nameEl);
    header.appendChild(compatibility);

    const descEl = document.createElement("p");
    descEl.className = "timer-theme-description";
    descEl.textContent = theme.description;

    const preview = document.createElement("div");
    preview.className = "timer-theme-preview";

    const previewDisplay = document.createElement("div");
    previewDisplay.className = "timer-preview-display";
    previewDisplay.setAttribute("data-preview-theme", theme.id);

    const previewTime = document.createElement("div");
    previewTime.className = "timer-preview-time";
    previewTime.textContent = "25:00";

    const previewStatus = document.createElement("div");
    previewStatus.className = "timer-preview-status";
    previewStatus.textContent = "Focus Session";

    previewDisplay.appendChild(previewTime);
    previewDisplay.appendChild(previewStatus);

    const colorStrip = document.createElement("div");
    colorStrip.className = "color-preview-strip";

    const focusColor = document.createElement("div");
    focusColor.className = "preview-color";
    focusColor.style.backgroundColor = theme.preview.focus;

    const breakColor = document.createElement("div");
    breakColor.className = "preview-color";
    breakColor.style.backgroundColor = theme.preview.break;

    const longBreakColor = document.createElement("div");
    longBreakColor.className = "preview-color";
    longBreakColor.style.backgroundColor = theme.preview.longBreak;

    colorStrip.appendChild(focusColor);
    colorStrip.appendChild(breakColor);
    colorStrip.appendChild(longBreakColor);

    preview.appendChild(previewDisplay);
    preview.appendChild(colorStrip);

    option.appendChild(header);
    option.appendChild(descEl);
    option.appendChild(preview);

    this.applyThemePreviewStyles(option, theme);

    if (isCompatible) {
      option.addEventListener("click", async () => {
        await this.selectTimerTheme(theme.id);
      });
    }

    return option;
  }

  /** @param {any} optionElement @param {any} theme */
  applyThemePreviewStyles(optionElement, theme) {
    const previewDisplay = optionElement.querySelector(".timer-preview-display");
    const previewTime = optionElement.querySelector(".timer-preview-time");
    const previewStatus = optionElement.querySelector(".timer-preview-status");

    if (!previewDisplay || !previewTime || !previewStatus) {
      return;
    }

    const themeId = theme.id;

    previewTime.style.color = theme.preview.focus;
    previewStatus.style.color = theme.preview.focus;

    if (themeId === "pipboy") {
      previewDisplay.style.background = "#000011";
      previewDisplay.style.border = `1px solid ${theme.preview.focus}`;
      previewDisplay.style.fontFamily = '"Share Tech Mono", monospace';
      previewTime.style.textShadow = `0 0 5px ${theme.preview.focus}`;
      previewStatus.style.textShadow = `0 0 3px ${theme.preview.focus}`;
    } else if (themeId === "espresso") {
      previewDisplay.style.background = "#3c2415";
      previewDisplay.style.border = `1px solid ${theme.preview.focus}`;
      previewDisplay.style.color = "#f4f1de";
    } else if (themeId === "pommodore64") {
      previewDisplay.style.background = "#40318d";
      previewDisplay.style.border = `1px solid ${theme.preview.focus}`;
      previewDisplay.style.color = "#7b68ee";
    } else {
      previewDisplay.style.background = "#f8f9fa";
      previewDisplay.style.border = `1px solid ${theme.preview.focus}`;
    }
  }

  /** @param {any} themeId */
  async selectTimerTheme(themeId) {
    this.updateTimerThemeSelector(themeId);
    await this.applyTimerTheme(themeId);

    this.settings.appearance.timer_theme = themeId;

    try {
      await invoke("save_settings", { settings: this.settings });
      logger.debug(`🎨 Timer theme saved: ${themeId}`);

      NotificationUtils.showNotificationPing(
        `✓ Timer theme changed to ${getThemeById(themeId).name}`,
        "success"
      );
    } catch (error) {
      logger.error("Failed to save timer theme setting:", error);
      NotificationUtils.showNotificationPing("❌ Failed to save timer theme", "error");
    }
  }

  /** @param {any} themeId */
  updateTimerThemeSelector(themeId) {
    const timerThemeGrid = document.getElementById("timer-theme-grid");
    if (!timerThemeGrid) {
      return;
    }

    const themeOptions = timerThemeGrid.querySelectorAll(".timer-theme-option");
    themeOptions.forEach((option) => {
      option.classList.remove("active");
    });

    const activeOption = timerThemeGrid.querySelector(`[data-timer-theme="${themeId}"]`);
    if (activeOption) {
      activeOption.classList.add("active");
    }
  }

  updateTimerThemeCompatibility() {
    const timerThemeGrid = document.getElementById("timer-theme-grid");
    if (!timerThemeGrid) {
      return;
    }

    const currentColorMode = this.getCurrentColorMode();
    const currentTimerTheme = this.settings.appearance?.timer_theme || "espresso";

    logger.debug(
      `🎨 Checking theme compatibility: ${currentTimerTheme} with mode ${currentColorMode}`
    );

    const isCompatible = isThemeCompatible(currentTimerTheme, currentColorMode);
    logger.debug(
      `🎨 Theme ${currentTimerTheme} is compatible with ${currentColorMode}: ${isCompatible}`
    );

    if (!isCompatible) {
      logger.debug(
        `🎨 Theme ${currentTimerTheme} not compatible, switching to compatible theme...`
      );
      const compatibleThemes = getCompatibleThemes(currentColorMode);
      if (compatibleThemes.length > 0) {
        const defaultCompatibleTheme =
          compatibleThemes.find((t) => t.isDefault) || compatibleThemes[0];
        logger.debug(`🎨 Switching to compatible theme: ${defaultCompatibleTheme.id}`);
        this.selectTimerTheme(defaultCompatibleTheme.id);
      }
    } else {
      logger.debug(`🎨 Theme ${currentTimerTheme} is compatible, keeping it`);
    }

    this.initializeTimerThemeSelector();
  }
}
