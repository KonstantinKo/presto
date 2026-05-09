const { invoke } = /** @type {{ invoke: (cmd: string, args?: any) => Promise<any> }} */ (
  window.__TAURI__ ? window.__TAURI__.core : { invoke: null }
);
import { NotificationUtils, KeyboardUtils } from "../utils/common-utils.js";
import { logger } from "../utils/logger.js";

export class PomodoroTimer {
  constructor() {
    // Timer states
    this.isRunning = false;
    this.isPaused = false;
    this.currentMode = "focus"; // 'focus', 'break', 'longBreak'
    this.timeRemaining = 25 * 60; // 25 minutes in seconds
    this.timerInterval = null;

    // Smart pause states
    this.smartPauseEnabled = false;
    this.isAutoPaused = false;
    this.activityTimeout = null;
    this.inactivityThreshold = 30000; // 30 seconds in milliseconds (configurable)
    this.smartPauseCountdownInterval = null;
    this.smartPauseSecondsRemaining = 0;
    /** @type {Array<() => void>} */
    this.activityUnlistenFns = [];

    // Session tracking
    this.completedPomodoros = 0;
    this.currentSession = 1;
    this.totalSessions = 10;
    this.totalFocusTime = 0; // in seconds

    // Session time tracking
    this.sessionStartTime = null; // When the current session was started
    this.lastSessionStartTime = null; // Preserved start time for the last completed session
    this.currentSessionElapsedTime = 0; // Actual elapsed time for current session (in seconds)
    this.lastCompletedSessionTime = 0; // Time of the last completed session for undo functionality
    this.sessionCompletedButNotSaved = false; // Flag to track if session completed but not saved yet
    this.maxSessionTime = 120 * 60 * 1000; // Default 2 hours in milliseconds
    this.maxSessionTimeReached = false; // Flag to track if max session time was reached

    // Timer accuracy tracking (for background throttling fix)
    this.timerStartTime = null; // When the timer was started (Date.now())
    this.timerDuration = null; // Original duration when timer was started
    this.lastUpdateTime = null; // Last time we updated the display

    // Timer durations (in seconds)
    this.durations = {
      focus: 25 * 60, // 25 minutes
      break: 5 * 60, // 5 minutes
      longBreak: 20 * 60, // 20 minutes
    };

    // DOM elements
    this.timerMinutes = document.getElementById("timer-minutes");
    this.timerSeconds = document.getElementById("timer-seconds");
    this.timerStatus = document.getElementById("timer-status");
    this.statusIcon = document.getElementById("status-icon");
    this.statusText = document.getElementById("status-text");
    this.playPauseBtn = document.getElementById("play-pause-btn");
    this.playIcon = document.getElementById("play-icon");
    this.pauseIcon = document.getElementById("pause-icon");
    this.stopBtn = document.getElementById("stop-btn");
    this.stopIcon = document.getElementById("stop-icon");
    this.undoIcon = document.getElementById("undo-icon");
    this.skipBtn = document.getElementById("skip-btn");
    this.skipCoffeeIcon = document.getElementById("skip-coffee-icon");
    this.skipSleepIcon = document.getElementById("skip-sleep-icon");
    this.skipBrainIcon = document.getElementById("skip-brain-icon");
    this.skipDefaultIcon = document.getElementById("skip-default-icon");
    this.progressDots = document.getElementById("progress-dots");
    this.smartIndicator = document.getElementById("smart-indicator");
    this.smartPauseCountdown = document.getElementById("smart-pause-countdown");
    this.autoStartIndicator = document.getElementById("auto-start-indicator");
    this.continuousSessionIndicator = document.getElementById("continuous-session-indicator");
    this.timerPlusBtn = document.getElementById("timer-plus-btn");
    this.timerMinusBtn = document.getElementById("timer-minus-btn");

    // Task management
    /** @type {any[]} */
    this.tasks = [];
    this.currentTask = "";

    // Notification preferences
    this.autoStartTimer = true; // Default to enabled for skip auto-start
    this.allowContinuousSessions = false; // Allow focus sessions to continue beyond timer

    // Debug mode
    this.debugMode = false; // Debug mode with 3-second timers

    // Keyboard shortcuts (will be updated from settings)
    this.customShortcuts = {
      start_stop: "CommandOrControl+Alt+Space",
      reset: "CommandOrControl+Alt+R", // Delete Session (focus) / Undo (break)
      skip: "CommandOrControl+Alt+S", // Save Session
    };

    // Midnight monitoring for daily reset
    this.midnightMonitorInterval = null;
    this.currentDateString = new Date().toDateString();

    this.init();
  }

  async init() {
    // Verify all DOM elements are found
    if (!this.undoIcon) {
      logger.error("Undo icon element not found, re-searching...");
      this.undoIcon = document.getElementById("undo-icon");
    }
    if (!this.stopIcon) {
      logger.error("Stop icon element not found, re-searching...");
      this.stopIcon = document.getElementById("stop-icon");
    }

    // Generate initial progress dots
    this.generateProgressDots();
    this.updateDisplay();
    await this.updateProgressDots();
    this.updateStopUndoButton(); // Initialize stop/undo button state
    this.updateSkipIcon(); // Initialize skip button icon
    this.updateSmartIndicator(); // Initialize smart pause indicator

    // Add tooltip to smart indicator if NavigationManager is available
    if (window.navigationManager && this.smartIndicator) {
      window.navigationManager.addTooltipEvents(this.smartIndicator);
    }
    this.setupEventListeners();
    await this.loadSessionData();
    await this.loadTasks();

    // Initialize task input if it exists
    this.taskInput = document.getElementById("task-input");
    this.taskList = document.getElementById("task-list");

    if (this.taskInput) {
      this.taskInput.addEventListener("keypress", (e) => {
        if (e.key === "Enter") {
          this.addTask();
        }
      });
    }

    // Render loaded tasks
    this.renderTasks();

    // Initialize smart indicator tooltip and click handler
    if (this.smartIndicator) {
      // Add click handler for toggle
      this.smartIndicator.addEventListener("click", () => {
        this.toggleSmartPause();
      });

      // Add tooltip using NavigationManager
      if (window.navigationManager) {
        window.navigationManager.addTooltipEvents(this.smartIndicator);
      }
    }

    // Initialize auto-start indicator
    if (this.autoStartIndicator) {
      this.autoStartIndicator.addEventListener("click", () => {
        this.toggleAutoStart();
      });

      // Add tooltip using NavigationManager
      if (window.navigationManager) {
        window.navigationManager.addTooltipEvents(this.autoStartIndicator);
      }
    }

    // Initialize continuous session indicator
    if (this.continuousSessionIndicator) {
      this.continuousSessionIndicator.addEventListener("click", () => {
        this.toggleContinuousSessions();
      });

      // Add tooltip using NavigationManager
      if (window.navigationManager) {
        window.navigationManager.addTooltipEvents(this.continuousSessionIndicator);
      }
    }

    // Update all setting indicators
    // Note: This will be called again in applySettings() with the correct values
    this.updateSettingIndicators();

    // Initialize sidebar state to match timer
    const sidebar = document.querySelector(".sidebar");
    if (sidebar) {
      sidebar.className = `sidebar ${this.currentMode}`;
    }

    // Initialize tray menu state
    this.updateTrayMenu();

    // Start midnight monitoring for daily reset
    this.startMidnightMonitoring();

    // Setup event listeners for session synchronization
    this.setupSessionEventListeners();
  }

  setupSessionEventListeners() {
    // Listen for session changes from SessionManager to keep dots synchronized
    window.addEventListener("sessionAdded", async (event) => {
      const { date } = /** @type {CustomEvent} */ (event).detail;
      const today = new Date().toDateString();

      // Only update dots if the session was added for today
      if (date === today) {
        await this.updateProgressDots();
      }
    });

    window.addEventListener("sessionDeleted", async (event) => {
      const { date } = /** @type {CustomEvent} */ (event).detail;
      const today = new Date().toDateString();

      // Only update dots if the session was deleted from today
      if (date === today) {
        await this.updateProgressDots();
      }
    });

    window.addEventListener("sessionUpdated", async (event) => {
      const { date } = /** @type {CustomEvent} */ (event).detail;
      const today = new Date().toDateString();

      // Only update dots if the session was updated for today
      if (date === today) {
        await this.updateProgressDots();
      }
    });
  }

  setupEventListeners() {
    /** @type {HTMLElement} */ (this.playPauseBtn).addEventListener("click", () => {
      if (this.isRunning && !this.isPaused && !this.isAutoPaused) {
        this.pauseTimer();
      } else {
        this.startTimer();
      }
    });

    /** @type {HTMLElement} */ (this.skipBtn).addEventListener(
      "click",
      async () => await this.skipSession()
    );

    /** @type {HTMLElement} */ (this.stopBtn).addEventListener("click", () => {
      if (this.currentMode === "focus") {
        // Delete/reset the current session
        this.resetTimer();
      } else {
        // In break/longBreak mode: undo last session
        this.undoLastSession();
      }
    });

    // Timer adjustment buttons
    if (this.timerPlusBtn) {
      this.timerPlusBtn.addEventListener("click", () => {
        this.adjustTimer(5);
      });
    }

    if (this.timerMinusBtn) {
      this.timerMinusBtn.addEventListener("click", () => {
        this.adjustTimer(-5);
      });
    }

    // Keyboard shortcuts
    document.addEventListener("keydown", async (e) => {
      // Only trigger if not typing in an input
      if (/** @type {HTMLElement} */ (e.target)?.tagName !== "INPUT") {
        // Check custom shortcuts first
        if (this.matchesShortcut(e, this.customShortcuts.start_stop)) {
          e.preventDefault();
          if (this.isRunning && !this.isPaused && !this.isAutoPaused) {
            this.pauseTimer();
          } else {
            this.startTimer();
          }
        } else if (this.matchesShortcut(e, this.customShortcuts.reset)) {
          e.preventDefault();
          if (this.currentMode === "focus") {
            this.resetTimer();
          } else {
            this.undoLastSession();
          }
        } else if (this.matchesShortcut(e, this.customShortcuts.skip)) {
          e.preventDefault();
          await this.skipSession();
        }
        // Keep existing hardcoded shortcuts as fallback
        else {
          switch (e.code) {
            case "Space":
              e.preventDefault();
              if (this.isRunning && !this.isPaused && !this.isAutoPaused) {
                this.pauseTimer();
              } else {
                this.startTimer();
              }
              break;
            case "KeyH":
              if (e.ctrlKey || e.metaKey) {
                e.preventDefault();
                this.showHistoryModal();
              }
              break;
            case "Escape":
              this.closeHistoryModal();
              break;
          }
        }
      }
    });

    // Prevent accidental closure during active session
    window.addEventListener("beforeunload", (e) => {
      if (this.isRunning && this.currentMode === "focus") {
        e.preventDefault();
        e.returnValue = "You have an active pomodoro session. Are you sure you want to leave?";
        return e.returnValue;
      }
    });

    // Listen for tray icon events
    this.setupTrayEventListeners();
  }

  async setupTrayEventListeners() {
    const listen = window.__TAURI__?.event?.listen;
    if (!listen) {
      return;
    }

    try {
      await listen("tray-start-session", () => {
        this.startTimer();
      });

      await listen("tray-pause", () => {
        this.pauseTimer();
      });

      await listen("tray-skip", async () => {
        await this.skipSession();
      });

      await listen("tray-cancel", () => {
        if (this.currentMode === "focus") {
          this.resetTimer();
        } else {
          this.undoLastSession();
        }
      });
    } catch (error) {
      logger.warn("Failed to register tray listeners:", error);
    }
  }

  async updateTrayMenu() {
    try {
      await invoke("update_tray_menu", {
        isRunning: this.isRunning,
        isPaused: this.isPaused,
        currentMode: this.currentMode,
      });
    } catch (error) {
      logger.error("Failed to update tray menu:", error);
    }
  }

  updateKeyboardShortcuts(/** @type {any} */ shortcuts) {
    const filtered = Object.fromEntries(Object.entries(shortcuts).filter(([, v]) => v != null));
    this.customShortcuts = { ...this.customShortcuts, ...filtered };
    logger.debug("Updated keyboard shortcuts:", this.customShortcuts);
  }

  parseShortcut(/** @type {string} */ shortcutString) {
    return KeyboardUtils.parseShortcut(shortcutString);
  }

  matchesShortcut(/** @type {any} */ event, /** @type {string} */ shortcutString) {
    return KeyboardUtils.matchesShortcut(event, shortcutString);
  }

  async setupSmartPause() {
    if (!this.smartPauseEnabled) {
      return;
    }

    // Setup event listeners for global activity monitoring
    await this.setupGlobalActivityListeners();

    logger.info("Smart pause enabled with global monitoring");
  }

  async setupGlobalActivityListeners() {
    try {
      for (const unlisten of this.activityUnlistenFns) {
        unlisten();
      }
      this.activityUnlistenFns = [];

      // Start global activity monitoring with configurable timeout
      const timeoutSeconds = Math.floor(this.inactivityThreshold / 1000); // convert from milliseconds to seconds
      await invoke("start_activity_monitoring", { timeoutSeconds });

      const listen = window.__TAURI__?.event?.listen;
      if (!listen) {
        throw new Error("Tauri event API unavailable");
      }

      this.activityUnlistenFns.push(
        await listen("user-activity", () => {
          this.handleUserActivity();
        })
      );

      this.activityUnlistenFns.push(
        await listen("user-inactivity", () => {
          this.autoPauseTimer();
        })
      );

      // Start initial timeout for local fallback
      this.handleUserActivity();

      logger.info("Global activity listeners setup complete");
    } catch (error) {
      logger.error("Failed to setup global activity monitoring:", error);
      for (const unlisten of this.activityUnlistenFns) {
        unlisten();
      }
      this.activityUnlistenFns = [];
      // Fallback to local monitoring
      this.setupLocalActivityListeners();
    }
  }

  setupLocalActivityListeners() {
    logger.info("Falling back to local activity monitoring");
    const activityEvents = ["mousemove", "mousedown", "keydown", "scroll", "click"];

    activityEvents.forEach((event) => {
      const handler = () => this.handleUserActivity();
      document.addEventListener(event, handler, { passive: true });
      this.activityUnlistenFns.push(() => document.removeEventListener(event, handler));
    });

    this.handleUserActivity();
  }

  handleUserActivity() {
    if (!this.smartPauseEnabled) {
      return;
    }

    // If currently auto-paused, resume the timer
    if (this.isAutoPaused) {
      logger.debug("🔄 Timer is auto-paused, calling resumeFromAutoPause()");
      this.resumeFromAutoPause();
      return; // Exit early after resume to avoid setting new timeout immediately
    }

    // Clear existing timeout and countdown
    if (this.activityTimeout) {
      clearTimeout(this.activityTimeout);
    }
    this.stopSmartPauseCountdown();

    // Set new timeout for auto-pause after configured inactivity period
    this.activityTimeout = setTimeout(() => {
      this.autoPauseTimer();
    }, this.inactivityThreshold); // Use configurable timeout

    // Start countdown display
    this.startSmartPauseCountdown();
  }

  startSmartPauseCountdown() {
    if (!this.smartPauseEnabled || !this.smartPauseCountdown) {
      return;
    }

    this.smartPauseSecondsRemaining = Math.floor(this.inactivityThreshold / 1000);
    this.updateSmartPauseCountdownDisplay();

    // Start the countdown interval
    this.smartPauseCountdownInterval = setInterval(() => {
      this.smartPauseSecondsRemaining--;
      this.updateSmartPauseCountdownDisplay();

      if (this.smartPauseSecondsRemaining <= 0) {
        this.stopSmartPauseCountdown();
      }
    }, 1000);
  }

  _stopTimerLoop() {
    if (this.timerInterval) {
      clearInterval(this.timerInterval);
    }
    this.timerInterval = null;
    if (this.activityTimeout) {
      clearTimeout(this.activityTimeout);
    }
    this.activityTimeout = null;
    this.stopSmartPauseCountdown();
  }

  stopSmartPauseCountdown() {
    if (this.smartPauseCountdownInterval) {
      clearInterval(this.smartPauseCountdownInterval);
      this.smartPauseCountdownInterval = null;
    }

    if (this.smartPauseCountdown) {
      this.smartPauseCountdown.style.display = "none";
    }

    this.smartPauseSecondsRemaining = 0;
  }

  updateSmartPauseCountdownDisplay() {
    if (!this.smartPauseCountdown) {
      return;
    }

    if (this.smartPauseSecondsRemaining > 0 && this.smartPauseSecondsRemaining <= 10) {
      this.smartPauseCountdown.textContent = String(this.smartPauseSecondsRemaining);
      this.smartPauseCountdown.style.display = "flex";
    } else {
      this.smartPauseCountdown.style.display = "none";
    }
  }

  autoPauseTimer() {
    logger.debug(
      "🚨 autoPauseTimer called - isRunning:",
      this.isRunning,
      "isPaused:",
      this.isPaused,
      "isAutoPaused:",
      this.isAutoPaused,
      "currentMode:",
      this.currentMode
    );

    if (
      !this.smartPauseEnabled ||
      !this.isRunning ||
      this.isPaused ||
      this.isAutoPaused ||
      this.currentMode !== "focus"
    ) {
      logger.debug("❌ autoPauseTimer early return due to conditions");
      return;
    }

    logger.info("💤 Auto-pausing timer due to inactivity");
    this.isAutoPaused = true;
    this.isPaused = true;
    logger.debug("🔧 States set: isAutoPaused =", this.isAutoPaused, "isPaused =", this.isPaused);

    this._stopTimerLoop();

    // Show auto-pause notification
    NotificationUtils.showNotificationPing(
      "Timer auto-paused due to inactivity 💤",
      "warning",
      this.currentMode
    );

    // Update UI to show auto-pause state
    this.updateDisplay();
    this.updateTrayIcon();
  }

  resumeFromAutoPause() {
    if (!this.isAutoPaused) {
      return;
    }

    logger.info("🔄 Resuming from auto-pause...");

    // Clear any existing timer interval first
    if (this.timerInterval) {
      clearInterval(this.timerInterval);
      this.timerInterval = null;
    }

    // Set states to resume - be very explicit about the state
    this.isAutoPaused = false;
    this.isPaused = false;
    this.isRunning = true;
    this._justResumedFromAutoPause = true; // Flag to ensure status update happens

    logger.debug(
      "✅ States set: isAutoPaused =",
      this.isAutoPaused,
      "isPaused =",
      this.isPaused,
      "isRunning =",
      this.isRunning
    );

    // Initialize timer accuracy tracking for resume
    this.timerStartTime = Date.now();
    this.timerDuration = this.timeRemaining;

    // Restart the timer interval with accuracy updates
    this.timerInterval = setInterval(() => {
      this.updateTimerWithAccuracy();
    }, 100);

    // Force synchronous UI update first
    this.updateDisplay();
    this.updateTrayIcon();

    // Clear the resume flag after UI update
    this._justResumedFromAutoPause = false;

    // Show resume notification after UI update
    NotificationUtils.showNotificationPing(
      "Timer resumed - you're back! 🎯",
      "info",
      this.currentMode
    );

    // Restart smart pause monitoring by setting new timeout
    if (this.smartPauseEnabled && this.currentMode === "focus") {
      // Clear any existing timeout first
      if (this.activityTimeout) {
        clearTimeout(this.activityTimeout);
      }
      this.stopSmartPauseCountdown();

      // Set new timeout for auto-pause
      this.activityTimeout = setTimeout(() => {
        this.autoPauseTimer();
      }, this.inactivityThreshold);

      // Start countdown display
      this.startSmartPauseCountdown();
    }

    // Update display to remove (Auto-paused) status
    this.updateDisplay();

    logger.debug("🎯 Resume complete, timer should be running normally");
  }

  async enableSmartPause(/** @type {boolean} */ enabled) {
    this.smartPauseEnabled = enabled;

    if (enabled) {
      await this.setupSmartPause();
      // Start monitoring if timer is already running and in focus mode
      if (this.isRunning && !this.isPaused && this.currentMode === "focus") {
        this.handleUserActivity();
      }
    } else {
      // Stop global monitoring
      try {
        await invoke("stop_activity_monitoring");
      } catch (error) {
        logger.error("Failed to stop activity monitoring:", error);
      }

      for (const unlisten of this.activityUnlistenFns) {
        unlisten();
      }
      this.activityUnlistenFns = [];

      // Clear local timeout, countdown, and resume if auto-paused
      if (this.activityTimeout) {
        clearTimeout(this.activityTimeout);
      }
      this.activityTimeout = null;
      this.stopSmartPauseCountdown();

      if (this.isAutoPaused) {
        this.resumeFromAutoPause();
      }
    }

    logger.info("Smart pause", enabled ? "enabled" : "disabled");

    // Update the smart indicator
    this.updateSmartIndicator();
  }

  startTimer() {
    if (!this.isRunning) {
      const wasResuming = this.isPaused; // Check if we're resuming from pause

      this.isRunning = true;
      this.isPaused = false;

      // Set flag to ensure status update when resuming from manual pause
      if (wasResuming) {
        this._justResumedFromPause = true;
      }

      // Track session start time if not already set
      if (!this.sessionStartTime) {
        this.sessionStartTime = Date.now();
        logger.info("🟢 NEW SESSION STARTED - sessionStartTime set to:", {
          timestamp: this.sessionStartTime,
          dateISO: new Date(this.sessionStartTime).toISOString(),
          dateLocal: new Date(this.sessionStartTime).toString(),
        });
        this.currentSessionElapsedTime = 0;
        this.sessionCompletedButNotSaved = false; // Reset flag for new session
      } else {
        logger.debug("⚠️ Session already started - not updating sessionStartTime:", {
          existingTimestamp: this.sessionStartTime,
          existingDateISO: new Date(this.sessionStartTime).toISOString(),
          existingDateLocal: new Date(this.sessionStartTime).toString(),
        });
      }

      // Initialize timer accuracy tracking
      this.timerStartTime = Date.now();
      this.timerDuration = this.timeRemaining;
      this.lastUpdateTime = this.timerStartTime;

      this.timerInterval = setInterval(() => {
        this.updateTimerWithAccuracy();
      }, 100); // Update more frequently (10 times per second) for smoother display

      this.updateDisplay();

      // Clear the resume flag after UI update
      if (wasResuming) {
        this._justResumedFromPause = false;
      }

      if (this.enableSoundNotifications) {
        NotificationUtils.playNotificationSound();
      }
      NotificationUtils.showNotificationPing("Timer started! 🍅", "info", this.currentMode);

      // Start smart pause monitoring if enabled
      if (this.smartPauseEnabled && this.currentMode === "focus") {
        this.handleUserActivity();
      }

      // Update tray menu
      this.updateTrayMenu();

      // Notify tag manager about timer start or resume
      if (window.tagManager) {
        if (wasResuming) {
          window.tagManager.onTimerResume();
        } else {
          window.tagManager.onTimerStart();
        }
      }
    }
  }

  updateTimerWithAccuracy() {
    const now = Date.now();
    const elapsedSinceStart = Math.floor(
      (now - /** @type {number} */ (this.timerStartTime)) / 1000
    );
    const newTimeRemaining = /** @type {number} */ (this.timerDuration) - elapsedSinceStart;

    // Only update if the time has actually changed (to avoid unnecessary updates)
    if (newTimeRemaining !== this.timeRemaining) {
      const oldTimeRemaining = this.timeRemaining;
      this.timeRemaining = newTimeRemaining;

      // Track elapsed time for focus sessions based on actual time passed
      if (this.currentMode === "focus") {
        const timeDiff = oldTimeRemaining - this.timeRemaining;
        if (timeDiff > 0) {
          this.currentSessionElapsedTime += timeDiff;
        }
      }

      // Check if max session time has been reached
      this.checkMaxSessionTime();

      this.updateDisplay();

      // Warning when less than 2 minutes remaining
      if (
        this.timeRemaining <= 120 &&
        this.timeRemaining > 0 &&
        oldTimeRemaining > 120 &&
        this.currentMode === "focus"
      ) {
        this.addWarningClass();
        NotificationUtils.showNotificationPing(
          "2 minutes remaining! 🔥",
          "warning",
          this.currentMode
        );
      }

      // Final warning at 30 seconds
      if (this.timeRemaining <= 30 && this.timeRemaining > 0 && oldTimeRemaining > 30) {
        NotificationUtils.showNotificationPing("30 seconds left! ⏰", "warning", this.currentMode);
      }

      // Check if timer should complete
      if (this.timeRemaining <= 0 && oldTimeRemaining > 0) {
        if (this.allowContinuousSessions) {
          // For continuous sessions, show completion notification but keep timer running
          this.showSessionCompletedNotification();
          // Reset timer tracking for overtime
          this.timerStartTime = now;
          this.timerDuration = 0; // Start from 0 for overtime
        } else {
          // Traditional behavior - complete and stop the session
          this.completeSession();
        }
      }
    }
  }

  checkMaxSessionTime() {
    // Only check during focus sessions and if a session is active
    if (this.currentMode !== "focus" || !this.sessionStartTime || this.maxSessionTimeReached) {
      return;
    }

    const now = Date.now();
    const sessionElapsed = now - this.sessionStartTime;

    // Check if session has exceeded max time
    if (sessionElapsed >= this.maxSessionTime) {
      this.maxSessionTimeReached = true;
      this.pauseTimer();

      // Show notification
      const maxTimeInMinutes = Math.floor(this.maxSessionTime / (60 * 1000));
      NotificationUtils.showNotificationPing(
        `Session automatically paused after ${maxTimeInMinutes} minutes. Take a break! 🛑`,
        "warning"
      );

      // Show desktop notification if enabled
      if (this.enableDesktopNotifications) {
        NotificationUtils.showDesktopNotification(
          "Session Time Limit Reached",
          `Your session has been automatically paused after ${maxTimeInMinutes} minutes. Consider taking a break!`
        );
      }
    }
  }

  pauseTimer() {
    if (this.isRunning) {
      this.isRunning = false;
      this.isPaused = true;
      this.isAutoPaused = false; // Manual pause overrides auto-pause
      if (this.timerInterval) {
        clearInterval(this.timerInterval);
      }
      this.timerInterval = null;

      // Update timer with current accurate time before pausing
      if (this.timerStartTime) {
        const now = Date.now();
        const elapsedSinceStart = Math.floor((now - this.timerStartTime) / 1000);
        this.timeRemaining = /** @type {number} */ (this.timerDuration) - elapsedSinceStart;
      }

      if (this.activityTimeout) {
        clearTimeout(this.activityTimeout);
      }
      this.activityTimeout = null;
      this.stopSmartPauseCountdown();

      this.updateDisplay();
      NotificationUtils.showNotificationPing("Timer paused ⏸️");

      // Update tray menu
      this.updateTrayMenu();

      // Notify tag manager about timer pause
      if (window.tagManager) {
        window.tagManager.onTimerPause();
      }
    }
  }

  resetTimer() {
    this.isRunning = false;
    this.isPaused = false;
    this.isAutoPaused = false;
    this._stopTimerLoop();

    // Reset session tracking
    this.sessionStartTime = null;
    this.currentSessionElapsedTime = 0;
    this.sessionCompletedButNotSaved = false; // Reset flag
    this.maxSessionTimeReached = false; // Reset max session time flag

    // Reset timer accuracy tracking
    this.timerStartTime = null;
    this.timerDuration = null;
    this.lastUpdateTime = null;

    this.timeRemaining = /** @type {Record<string, number>} */ (this.durations)[this.currentMode];
    this.updateDisplay();
    NotificationUtils.showNotificationPing("Session deleted ❌", "warning");

    // Update tray menu
    this.updateTrayMenu();

    // Notify tag manager about timer stop
    if (window.tagManager) {
      window.tagManager.onTimerStop();
    }
  }

  adjustTimer(/** @type {number} */ minutes) {
    // Convert minutes to seconds
    const adjustment = minutes * 60;

    // Add the adjustment to the current time remaining
    this.timeRemaining += adjustment;

    // Ensure we don't go below 0 seconds
    if (this.timeRemaining < 0) {
      this.timeRemaining = 0;
    }

    // If timer is running, we need to update the accuracy tracking
    if (this.isRunning && this.timerStartTime) {
      // Calculate how much time should remain based on the adjustment
      const now = Date.now();
      const elapsedSinceStart = Math.floor((now - this.timerStartTime) / 1000);

      // Update the timer duration to account for the adjustment
      this.timerDuration = elapsedSinceStart + this.timeRemaining;
    }

    // Update the display immediately
    this.updateDisplay();

    // Show notification
    const action = minutes > 0 ? "added" : "subtracted";
    const absMinutes = Math.abs(minutes);
    NotificationUtils.showNotificationPing(
      `${absMinutes} minute${absMinutes !== 1 ? "s" : ""} ${action} ${minutes > 0 ? "to" : "from"} timer ⏰`
    );
  }

  startMidnightMonitoring() {
    // Clear any existing monitoring
    this.stopMidnightMonitoring();

    // Check for date change every minute
    this.midnightMonitorInterval = setInterval(() => {
      this.checkForMidnightReset();
    }, 60000); // Check every minute

    logger.debug("🌙 Midnight monitoring started");
  }

  stopMidnightMonitoring() {
    if (this.midnightMonitorInterval) {
      clearInterval(this.midnightMonitorInterval);
      this.midnightMonitorInterval = null;
      logger.debug("🌙 Midnight monitoring stopped");
    }
  }

  checkForMidnightReset() {
    const newDateString = new Date().toDateString();

    if (newDateString !== this.currentDateString) {
      logger.info("🌙 Date change detected:", this.currentDateString, "→", newDateString);
      this.currentDateString = newDateString;
      this.performMidnightReset();
    }
  }

  async performMidnightReset() {
    logger.info("🌅 Performing midnight reset...");

    // Store previous session count for notification
    const previousPomodoros = this.completedPomodoros;

    // Use enhanced loadSessionData with force reset to ensure clean state
    await this.loadSessionData(true);

    // Update display to show the reset state
    this.updateDisplay();

    // Save the reset state
    await this.saveSessionData();

    // Show user notification about the reset
    if (previousPomodoros > 0) {
      NotificationUtils.showNotificationPing(
        `New day! Yesterday's ${previousPomodoros} sessions have been saved. Fresh start! 🌅`,
        "info"
      );
    } else {
      NotificationUtils.showNotificationPing(
        "Good morning! Ready for a productive new day? 🌅",
        "info"
      );
    }

    // Update all visual elements
    this.updateTrayIcon();

    // Refresh navigation charts if available
    if (window.navigationManager) {
      try {
        await window.navigationManager.updateDailyChart();
        await window.navigationManager.updateFocusSummary();
        await window.navigationManager.updateWeeklySessionsChart();
      } catch (error) {
        logger.error("Failed to update navigation charts after midnight reset:", error);
      }
    }

    logger.info("✅ Midnight reset completed successfully");
  }

  async skipSession() {
    this.isRunning = false;
    this.isPaused = false;
    this.isAutoPaused = false;
    this._stopTimerLoop();

    const skippedMode = this.currentMode;

    // Track if we need to save session data
    let shouldSaveSession = false;

    // OVERTIME: If in overtime and continuous sessions, save session and skip double-counting
    if (this.timeRemaining < 0 && this.allowContinuousSessions) {
      shouldSaveSession = true;

      // During overtime, if session was completed but not saved, now save it with overtime included
      if (this.sessionCompletedButNotSaved && this.currentMode === "focus") {
        // Calculate total time including overtime
        const now = Date.now();
        const totalElapsedTime = Math.floor(
          (now - /** @type {number} */ (this.sessionStartTime)) / 1000
        );
        this.lastCompletedSessionTime = totalElapsedTime;

        // Only save if session lasted at least 1 minute
        if (this.lastCompletedSessionTime > 60) {
          await this.saveCompletedFocusSession();
        }
      }

      this.sessionCompletedButNotSaved = false;

      // Move to next mode as usual
      if (this.currentMode === "focus") {
        if (this.completedPomodoros % 4 === 0) {
          this.currentMode = "longBreak";
        } else {
          this.currentMode = "break";
        }

        // Reset display state tracking when switching to break modes
        /** @type {any} */ (this)._lastLoggedState = null;
        this._lastAutoPausedLogged = false;
        this._lastPausedLogged = false;
      } else {
        this.currentMode = "focus";

        // Reset display state tracking when switching to focus mode
        /** @type {any} */ (this)._lastLoggedState = null;
        this._lastAutoPausedLogged = false;
        this._lastPausedLogged = false;

        // Restore TagManager display when returning to focus mode
        if (window.tagManager) {
          window.tagManager.updateStatusDisplay();
        }

        if (this.completedPomodoros < this.totalSessions) {
          this.currentSession = this.completedPomodoros + 1;
        }
      }
      this.timeRemaining = /** @type {Record<string, number>} */ (this.durations)[this.currentMode];
      this.updateDisplay();
      if (shouldSaveSession) {
        this.saveSessionData();
      }

      // Reset session start time for next session (after saving)
      logger.debug("🔄 Resetting sessionStartTime after overtime skip:", {
        beforeReset: this.sessionStartTime,
        beforeResetISO: this.sessionStartTime
          ? new Date(this.sessionStartTime).toISOString()
          : null,
      });
      this.sessionStartTime = null;

      const messages = {
        focus: "Focus session skipped. Time for a break! 😌",
        break: "Break skipped. Ready to focus? 🍅",
        longBreak: "Long break skipped. Time to get back to work! 🚀",
      };
      NotificationUtils.showNotificationPing(
        /** @type {Record<string, string>} */ (messages)[skippedMode] || "Session skipped 📤",
        "info",
        this.currentMode
      );
      if (this.autoStartTimer) {
        setTimeout(() => {
          this.startTimer();
        }, 1500);
      }
      this.updateTrayMenu();
      return; // Prevent double-counting
    }

    // NORMAL SKIP LOGIC
    if (this.currentMode === "focus") {
      if (!this.sessionCompletedButNotSaved) {
        this.completedPomodoros++;
        await this.updateProgressDots();
        const actualElapsedTime =
          this.currentSessionElapsedTime || this.durations.focus - this.timeRemaining;
        this.totalFocusTime += actualElapsedTime;
        this.lastCompletedSessionTime = actualElapsedTime;

        // Preserve session start time for saving
        logger.debug("Preserving session start time:", {
          before: this.lastSessionStartTime,
          sessionStartTime: this.sessionStartTime,
          preservedValue: this.sessionStartTime,
        });
        this.lastSessionStartTime = this.sessionStartTime;

        // Save skipped focus session to SessionManager as individual session
        // Only save if session lasted at least 1 minute
        if (this.lastCompletedSessionTime > 60) {
          await this.saveCompletedFocusSession();
        }
      }
      shouldSaveSession = true;
      this.sessionCompletedButNotSaved = false;
      if (this.completedPomodoros % 4 === 0) {
        this.currentMode = "longBreak";
      } else {
        this.currentMode = "break";
      }

      // Reset display state tracking when switching to break modes
      /** @type {any} */ (this)._lastLoggedState = null;
      this._lastAutoPausedLogged = false;
      this._lastPausedLogged = false;
    } else {
      this.currentMode = "focus";

      // Reset display state tracking when switching to focus mode
      /** @type {any} */ (this)._lastLoggedState = null;
      this._lastAutoPausedLogged = false;
      this._lastPausedLogged = false;

      // Restore TagManager display when returning to focus mode
      if (window.tagManager) {
        window.tagManager.updateStatusDisplay();
      }

      if (this.completedPomodoros < this.totalSessions) {
        this.currentSession = this.completedPomodoros + 1;
      }
    }
    this.timeRemaining = /** @type {Record<string, number>} */ (this.durations)[this.currentMode];
    this.updateDisplay();
    if (shouldSaveSession) {
      this.saveSessionData();
    }

    // Reset session start time for next session (after saving)
    logger.debug("🔄 Resetting sessionStartTime after normal skip:", {
      beforeReset: this.sessionStartTime,
      beforeResetISO: this.sessionStartTime ? new Date(this.sessionStartTime).toISOString() : null,
    });
    this.sessionStartTime = null;

    const messages = {
      focus: "Focus session skipped. Time for a break! 😌",
      break: "Break skipped. Ready to focus? 🍅",
      longBreak: "Long break skipped. Time to get back to work! 🚀",
    };
    NotificationUtils.showNotificationPing(
      /** @type {Record<string, string>} */ (messages)[skippedMode] || "Session skipped 📤",
      "info",
      this.currentMode
    );
    if (this.autoStartTimer) {
      setTimeout(() => {
        this.startTimer();
      }, 1500);
    }
    this.updateTrayMenu();
  }

  async completeSession() {
    this.isRunning = false;
    this.isPaused = false;
    this._stopTimerLoop();

    // Track completion state
    let shouldChangeMode = true;

    if (this.currentMode === "focus") {
      this.completedPomodoros++;
      await this.updateProgressDots();

      // Calculate actual elapsed time for focus sessions
      const actualElapsedTime =
        this.currentSessionElapsedTime || this.durations.focus - this.timeRemaining;
      this.totalFocusTime += actualElapsedTime;

      // Store the actual elapsed time for undo functionality
      this.lastCompletedSessionTime = actualElapsedTime;

      // Preserve session start time for saving
      logger.debug("Preserving session start time (timer completion):", {
        before: this.lastSessionStartTime,
        sessionStartTime: this.sessionStartTime,
        preservedValue: this.sessionStartTime,
      });
      this.lastSessionStartTime = this.sessionStartTime;

      // Mark current task as completed if exists
      if (this.currentTask.trim()) {
        await this.markTaskCompleted(this.currentTask.trim());
        if (this.taskInput) {
          /** @type {HTMLInputElement} */ (this.taskInput).value = "";
        }
        this.currentTask = "";
      }

      // Check if continuous sessions are enabled
      if (this.allowContinuousSessions) {
        // Don't change mode - stay in focus for continuous sessions
        shouldChangeMode = false;
      } else {
        // Determine next mode for traditional behavior
        if (this.completedPomodoros % 4 === 0) {
          this.currentMode = "longBreak";
        } else {
          this.currentMode = "break";
        }

        // Reset display state tracking when switching to break modes
        /** @type {any} */ (this)._lastLoggedState = null;
        this._lastAutoPausedLogged = false;
        this._lastPausedLogged = false;
      }
    } else {
      // Break completed
      if (this.allowContinuousSessions) {
        // Don't change mode - stay in current break type for continuous sessions
        shouldChangeMode = false;
      } else {
        // Traditional behavior - go back to focus
        this.currentMode = "focus";

        // Reset display state tracking when switching to focus mode
        /** @type {any} */ (this)._lastLoggedState = null;
        this._lastAutoPausedLogged = false;
        this._lastPausedLogged = false;

        // Restore TagManager display when returning to focus mode
        if (window.tagManager) {
          window.tagManager.updateStatusDisplay();
        }
        if (this.completedPomodoros < this.totalSessions) {
          this.currentSession = this.completedPomodoros + 1;
        }
      }
    }

    // Save completed focus session to SessionManager as individual session BEFORE resetting sessionStartTime
    if (this.lastCompletedSessionTime > 0 && this.completedPomodoros > 0) {
      await this.saveCompletedFocusSession();
    }

    // Reset session tracking for next session
    logger.debug("Resetting sessionStartTime for next session (from completeSession)");
    this.sessionStartTime = null;
    this.currentSessionElapsedTime = 0;
    this.sessionCompletedButNotSaved = false; // Reset flag
    this.maxSessionTimeReached = false; // Reset max session time flag

    // Reset timer accuracy tracking
    this.timerStartTime = null;
    this.timerDuration = null;
    this.lastUpdateTime = null;

    this.timeRemaining = /** @type {Record<string, number>} */ (this.durations)[this.currentMode];
    this.updateDisplay();

    // Only save aggregated session data, individual sessions are handled by saveCompletedFocusSession
    await this.saveSessionData();
    this.showNotification();
    if (this.enableSoundNotifications) {
      NotificationUtils.playNotificationSound();
    }

    // Show completion message
    let completionMessage;
    if (this.allowContinuousSessions) {
      // Messages for continuous sessions
      const continuousMessages = {
        focus: "Pomodoro completed! Continue working or take a break? 🍅",
        break: "Break time completed! Continue resting or ready to focus? ☕",
        longBreak: "Long break completed! Continue resting or ready to work? 🌙",
      };
      completionMessage = /** @type {Record<string, string>} */ (continuousMessages)[
        this.currentMode
      ];
    } else {
      // Messages for traditional mode
      const traditionalMessages = {
        focus:
          this.currentMode === "longBreak"
            ? "Great work! Take a long break 🎉"
            : "Pomodoro completed! Take a short break 😌",
        break: "Break over! Ready to focus? 🍅",
        longBreak: "Long break over! Time to get back to work 🚀",
      };
      completionMessage =
        /** @type {Record<string, string>} */ (traditionalMessages)[this.currentMode] ||
        traditionalMessages.focus;
    }

    NotificationUtils.showNotificationPing(completionMessage, "success", this.currentMode);

    // Stop the timer after session completion
    this.isRunning = false;
    this.isPaused = false;
    this.isAutoPaused = false;

    // Clear intervals and timeouts
    if (this.timerInterval) {
      clearInterval(this.timerInterval);
      this.timerInterval = null;
    }
    if (this.activityTimeout) {
      clearTimeout(this.activityTimeout);
      this.activityTimeout = null;
    }

    // Update display and buttons to reflect stopped state
    this.updateDisplay();
    this.updateTrayIcon();

    // Auto-start new session if enabled and mode changed (traditional mode only)
    if (this.autoStartTimer && !this.allowContinuousSessions && shouldChangeMode) {
      logger.info("Auto-starting new session after completion in 1.5 seconds...");
      // Add a small delay to let the user see the completion message
      setTimeout(() => {
        this.startTimer();
      }, 1500); // 1.5 second delay
    }

    // Update tray menu
    this.updateTrayMenu();

    // Notify tag manager about timer completion
    if (window.tagManager) {
      window.tagManager.onTimerComplete();
    }
  }

  async showSessionCompletedNotification() {
    // Update completed sessions count for focus sessions
    if (this.currentMode === "focus") {
      this.completedPomodoros++;
      await this.updateProgressDots();

      // Calculate actual elapsed time for focus sessions
      const actualElapsedTime = this.currentSessionElapsedTime || this.durations.focus;
      this.totalFocusTime += actualElapsedTime;

      // Store the actual elapsed time for undo functionality
      this.lastCompletedSessionTime = actualElapsedTime;

      // Preserve session start time for saving
      logger.debug("Preserving session start time (overtime):", {
        before: this.lastSessionStartTime,
        sessionStartTime: this.sessionStartTime,
        preservedValue: this.sessionStartTime,
      });
      this.lastSessionStartTime = this.sessionStartTime;

      // Mark current task as completed if exists
      if (this.currentTask.trim()) {
        await this.markTaskCompleted(this.currentTask.trim());
        if (this.taskInput) {
          /** @type {HTMLInputElement} */ (this.taskInput).value = "";
        }
        this.currentTask = "";
      }

      // Mark session as completed but not saved yet (will be saved when user skips)
      this.sessionCompletedButNotSaved = true;

      // Don't save session immediately during overtime - wait for skip to include full duration
    }

    // For continuous sessions, don't save the session data here
    // It will be saved when the user skips the session
    if (!this.allowContinuousSessions) {
      // Save session data only for traditional mode
      await this.saveSessionData();
      await this.updateWeeklyStats();
    }

    // Show notification
    this.showNotification();
    if (this.enableSoundNotifications) {
      NotificationUtils.playNotificationSound();
    }

    // Show completion message for continuous sessions
    const continuousMessages = {
      focus: "Pomodoro completed! You can continue working or take a break 🍅⏰",
      break: "Break time completed! You can continue resting or start focusing ☕⏰",
      longBreak: "Long break completed! You can continue resting or start working 🌙⏰",
    };

    const completionMessage = /** @type {Record<string, string>} */ (continuousMessages)[
      this.currentMode
    ];
    NotificationUtils.showNotificationPing(completionMessage, "success", this.currentMode);

    // Update tray icon to show completion but keep running
    this.updateTrayIcon();
  }

  updateDisplay() {
    let displayMinutes,
      displaySeconds,
      isOvertime = false;

    if (this.timeRemaining < 0 && this.allowContinuousSessions) {
      // Show overtime in continuous sessions
      isOvertime = true;
      const overtimeSeconds = Math.abs(this.timeRemaining);
      displayMinutes = Math.floor(overtimeSeconds / 60);
      displaySeconds = overtimeSeconds % 60;
    } else {
      // Normal display or traditional mode
      const absTime = Math.abs(this.timeRemaining);
      displayMinutes = Math.floor(absTime / 60);
      displaySeconds = absTime % 60;
    }

    // Update the split display
    if (this.timerMinutes) {
      this.timerMinutes.textContent = displayMinutes.toString().padStart(2, "0");
    }
    if (this.timerSeconds) {
      this.timerSeconds.textContent = displaySeconds.toString().padStart(2, "0");
    }

    // Update status - respect TagManager when tags are selected
    const statusTexts = {
      focus: "Focus",
      break: "Break",
      longBreak: "Long Break",
    };

    let statusText = /** @type {Record<string, string>} */ (statusTexts)[this.currentMode];
    let shouldUpdateStatus = true;

    // Check if TagManager is active and has selected tags for focus mode
    if (
      this.currentMode === "focus" &&
      window.tagManager &&
      window.tagManager.getCurrentTags().length > 0
    ) {
      // Don't override tag display during normal focus sessions
      // BUT always allow status updates when just resumed from pause to clear pause labels
      if (
        !isOvertime &&
        !this.isAutoPaused &&
        !(this.isPaused && !this.isRunning) &&
        !this._justResumedFromAutoPause &&
        !this._justResumedFromPause
      ) {
        shouldUpdateStatus = false;
      } else {
        // For special states, append to tag name instead of overriding
        const currentTags = window.tagManager.getCurrentTags();
        if (currentTags.length === 1) {
          statusText = currentTags[0].name;
        } else if (currentTags.length > 1) {
          statusText = `${currentTags.length} Tags`;
        }
      }
    }

    // Add state indicators
    if (shouldUpdateStatus) {
      // Only log state changes, not every update to reduce spam
      const currentState = `${isOvertime}-${this.isAutoPaused}-${this.isPaused}-${this.isRunning}`;
      const _self = /** @type {any} */ (this);
      if (
        !_self._lastLoggedState ||
        _self._lastLoggedState !== currentState ||
        this._justResumedFromAutoPause ||
        this._justResumedFromPause
      ) {
        logger.debug(
          "📊 updateDisplay - isOvertime:",
          isOvertime,
          "isAutoPaused:",
          this.isAutoPaused,
          "isPaused:",
          this.isPaused,
          "isRunning:",
          this.isRunning,
          "justResumedFromAutoPause:",
          this._justResumedFromAutoPause,
          "justResumedFromPause:",
          this._justResumedFromPause
        );
        _self._lastLoggedState = currentState;
      }

      if (isOvertime) {
        statusText += " (Overtime)";
        if (!_self._lastLoggedState || !_self._lastLoggedState.startsWith("true-")) {
          logger.debug("⏰ Showing Overtime status");
        }
      } else if (this.isAutoPaused) {
        statusText += " (Auto-paused)";
        if (!this._lastAutoPausedLogged) {
          logger.debug("💤 Showing Auto-paused status");
          this._lastAutoPausedLogged = true;
        }
      } else if (this.isPaused && !this.isRunning) {
        statusText += " (Paused)";
        if (!this._lastPausedLogged) {
          logger.debug("⏸️ Showing Paused status");
          this._lastPausedLogged = true;
        }
      } else {
        // Reset paused flags when not in those states
        this._lastAutoPausedLogged = false;
        this._lastPausedLogged = false;
        if (
          _self._lastLoggedState &&
          (_self._lastLoggedState.includes("true") ||
            this._justResumedFromAutoPause ||
            this._justResumedFromPause)
        ) {
          logger.debug("▶️ No status suffix (timer running normally)");
        }
      }

      // Update status text only when necessary
      const statusTextElement = document.getElementById("status-text");
      if (statusTextElement) {
        statusTextElement.textContent = statusText;
      } else {
        // Fallback to setting the entire timer status if span doesn't exist
        if (this.timerStatus) {
          this.timerStatus.textContent = statusText;
        }
      }
    }

    // Update status icon based on current mode (only for break modes or special states)
    if (
      this.currentMode !== "focus" ||
      isOvertime ||
      this.isAutoPaused ||
      (this.isPaused && !this.isRunning)
    ) {
      this.updateStatusIcon();
    }

    // Update play/pause button
    if (this.isRunning && !this.isPaused && !this.isAutoPaused) {
      if (this.playIcon) {
        this.playIcon.style.display = "none";
      }
      if (this.pauseIcon) {
        this.pauseIcon.style.display = "block";
      }
    } else {
      if (this.playIcon) {
        this.playIcon.style.display = "block";
      }
      if (this.pauseIcon) {
        this.pauseIcon.style.display = "none";
      }
    }

    // Update main container class for background styling
    const mainContainer = document.querySelector(".container");
    let containerClass = `container ${this.currentMode}`;
    if (isOvertime) {
      containerClass += " overtime";
    }
    if (mainContainer) {
      mainContainer.className = containerClass;
    }

    // Update body class to match current timer state for background (only in timer view)
    const body = document.body;
    const navigationManager = window.navigationManager;
    // Always apply timer classes if we're in timer view OR if navigation manager isn't initialized yet (default view is timer)
    if (body && (!navigationManager || navigationManager.currentView === "timer")) {
      let bodyClass = `${this.currentMode}`;
      if (isOvertime) {
        bodyClass += " overtime";
      }
      body.className = bodyClass;
    }

    // Update sidebar class to match current timer state
    const sidebar = document.querySelector(".sidebar");
    if (sidebar) {
      let sidebarClass = `sidebar ${this.currentMode}`;
      if (isOvertime) {
        sidebarClass += " overtime";
      }
      sidebar.className = sidebarClass;
    }

    // Update timer container class for styling
    const timerContainer = document.querySelector(".timer-container");
    if (timerContainer) {
      timerContainer.className = `timer-container ${this.currentMode}`;
    }

    // Update controls class to match current timer state
    const controls = document.querySelector(".controls");
    if (controls) {
      controls.className = `controls ${this.currentMode}`;
    }

    // Add running class when timer is active
    if (this.isRunning) {
      if (timerContainer) {
        timerContainer.classList.add("running");
      }
    }

    // Add auto-paused class when in auto-pause state
    if (this.isAutoPaused) {
      if (timerContainer) {
        timerContainer.classList.add("auto-paused");
      }
    }

    // Add warning class when time is running low
    if (this.timeRemaining <= 120 && this.timeRemaining > 0 && this.isRunning) {
      if (timerContainer) {
        timerContainer.classList.add("warning");
      }
    }

    // Update page title
    const statusIcon =
      this.currentMode === "focus" ? "🍅" : this.currentMode === "break" ? "😌" : "🎉";
    const overtimePrefix = isOvertime ? "+" : "";
    document.title = `${statusIcon} ${overtimePrefix}${displayMinutes.toString().padStart(2, "0")}:${displaySeconds.toString().padStart(2, "0")} - Presto`;

    // Update stop/undo button icon based on current mode
    this.updateStopUndoButton();

    // Update skip button icon based on current mode and next state
    this.updateSkipIcon();

    // Update progress dots
    this.updateProgressDots();

    // Update tray icon with timer information
    this.updateTrayIcon();

    // Hide smart pause countdown if not in focus mode
    if (this.currentMode !== "focus") {
      this.stopSmartPauseCountdown();
    }
  }

  updateStatusIcon() {
    if (!this.statusIcon) {
      logger.error("Status icon element not found!");
      return;
    }

    // Define icon classes for different modes
    const iconClasses = {
      focus: "ri-brain-line",
      break: "ri-cup-fill",
      longBreak: "ri-moon-line",
    };

    // Apply the appropriate Remix Icon class based on current mode
    const newClass =
      /** @type {Record<string, string>} */ (iconClasses)[this.currentMode] || "ri-brain-line";
    this.statusIcon.className = newClass;

    // Clear any fallback text content since we're using icons
    this.statusIcon.textContent = "";
  }

  updateSettingIndicators() {
    logger.debug("🔄 Updating setting indicators...");
    logger.debug("autoStartTimer:", this.autoStartTimer);
    logger.debug("allowContinuousSessions:", this.allowContinuousSessions);
    logger.debug("smartPauseEnabled:", this.smartPauseEnabled);

    // Update smart pause indicator
    if (this.smartIndicator) {
      // Always show the indicator
      this.smartIndicator.style.display = "block";

      if (this.smartPauseEnabled) {
        // Use filled bulb icon when active
        this.smartIndicator.className = "ri-lightbulb-fill active";
      } else {
        // Use line bulb icon when inactive
        this.smartIndicator.className = "ri-lightbulb-line";
      }
      logger.debug("✅ Smart indicator updated:", this.smartIndicator.className);
    } else {
      logger.debug("❌ Smart indicator element not found");
    }

    // Update auto-start indicator
    if (this.autoStartIndicator) {
      if (this.autoStartTimer) {
        // Use filled play icon when active
        this.autoStartIndicator.className = "ri-play-circle-fill active";
      } else {
        // Use line play icon when inactive
        this.autoStartIndicator.className = "ri-play-circle-line";
      }
      logger.debug("✅ Auto-start indicator updated:", this.autoStartIndicator.className);
    } else {
      logger.debug("❌ Auto-start indicator element not found");
    }

    // Update continuous session indicator
    if (this.continuousSessionIndicator) {
      if (this.allowContinuousSessions) {
        // Use filled repeat icon when active
        this.continuousSessionIndicator.className = "ri-repeat-fill active";
      } else {
        // Use line repeat icon when inactive
        this.continuousSessionIndicator.className = "ri-repeat-line";
      }
      logger.debug(
        "✅ Continuous sessions indicator updated:",
        this.continuousSessionIndicator.className
      );
    } else {
      logger.debug("❌ Continuous sessions indicator element not found");
    }
  }

  // Legacy method for backward compatibility
  updateSmartIndicator() {
    this.updateSettingIndicators();
  }

  async toggleSmartPause() {
    const newState = !this.smartPauseEnabled;
    await this.enableSmartPause(newState);

    // Save the setting by updating checkbox and triggering auto-save
    if (window.settingsManager) {
      const smartPauseCheckbox = document.getElementById("smart-pause");
      if (smartPauseCheckbox) {
        /** @type {HTMLInputElement} */ (smartPauseCheckbox).checked = newState;
        window.settingsManager.scheduleAutoSave();
      }
    }

    // Show notification
    const message = newState
      ? "Smart Pause enabled! Timer will auto-pause during inactivity 🧠"
      : "Smart Pause disabled 💡";
    NotificationUtils.showNotificationPing(message, "info");
  }

  async enableAutoStart(/** @type {boolean} */ enabled) {
    this.autoStartTimer = enabled;
    logger.debug("📍 enableAutoStart called with:", enabled);

    // Update the indicator to reflect the new state
    this.updateSettingIndicators();
  }

  async toggleAutoStart() {
    const newState = !this.autoStartTimer;
    await this.enableAutoStart(newState);

    // Save the setting by updating checkbox and triggering auto-save
    if (window.settingsManager) {
      const autoStartCheckbox = document.getElementById("auto-start-timer");
      if (autoStartCheckbox) {
        /** @type {HTMLInputElement} */ (autoStartCheckbox).checked = newState;
        window.settingsManager.scheduleAutoSave();
      }
    }

    // Show notification
    const message = newState
      ? "Auto-start enabled! Sessions will start automatically ⚡"
      : "Auto-start disabled 🛑";
    NotificationUtils.showNotificationPing(message, "info");
  }

  async enableContinuousSessions(/** @type {boolean} */ enabled) {
    this.allowContinuousSessions = enabled;
    logger.debug("📍 enableContinuousSessions called with:", enabled);

    // Update the indicator to reflect the new state
    this.updateSettingIndicators();
  }

  async toggleContinuousSessions() {
    const newState = !this.allowContinuousSessions;
    await this.enableContinuousSessions(newState);

    // Save the setting by updating checkbox and triggering auto-save
    if (window.settingsManager) {
      const continuousCheckbox = document.getElementById("allow-continuous-sessions");
      if (continuousCheckbox) {
        /** @type {HTMLInputElement} */ (continuousCheckbox).checked = newState;
        logger.debug("🔄 Toggle - Set checkbox to:", newState);
        logger.debug(
          "🔄 Toggle - Checkbox actual value:",
          /** @type {HTMLInputElement} */ (continuousCheckbox).checked
        );
        window.settingsManager.scheduleAutoSave();
      } else {
        logger.debug("❌ Toggle - Checkbox element not found!");
      }
    }

    // Show notification
    const message = newState
      ? "Continuous Sessions enabled! Sessions will continue beyond timer ♾️"
      : "Continuous Sessions disabled ⏹️";
    NotificationUtils.showNotificationPing(message, "info");
  }

  updateStopUndoButton() {
    // Check if DOM elements exist
    if (!this.stopIcon || !this.undoIcon) {
      logger.error("Stop or undo icon elements not found!");
      logger.debug("stopIcon:", this.stopIcon);
      logger.debug("undoIcon:", this.undoIcon);
      return;
    }

    if (this.currentMode === "focus") {
      // Show X icon during focus sessions for deletion
      this.stopIcon.style.display = "block";
      this.undoIcon.style.display = "none";
    } else {
      // Show undo icon during break/longBreak sessions (only if there's a session to undo)
      if (this.completedPomodoros > 0) {
        this.stopIcon.style.display = "none";
        this.undoIcon.style.display = "block";
      } else {
        // No sessions to undo, show X icon
        this.stopIcon.style.display = "block";
        this.undoIcon.style.display = "none";
      }
    }
  }

  updateSkipIcon() {
    // Hide all skip icons first
    if (this.skipCoffeeIcon) {
      this.skipCoffeeIcon.style.display = "none";
    }
    if (this.skipSleepIcon) {
      this.skipSleepIcon.style.display = "none";
    }
    if (this.skipBrainIcon) {
      this.skipBrainIcon.style.display = "none";
    }
    if (this.skipDefaultIcon) {
      this.skipDefaultIcon.style.display = "none";
    }

    if (this.currentMode === "focus") {
      // During focus: skip will go to break/longBreak
      // Show icon representing the next break type
      if ((this.completedPomodoros + 1) % 4 === 0) {
        // Next will be long break - show sleep icon
        if (this.skipSleepIcon) {
          this.skipSleepIcon.style.display = "block";
        }
      } else {
        // Next will be short break - show coffee icon
        if (this.skipCoffeeIcon) {
          this.skipCoffeeIcon.style.display = "block";
        }
      }
    } else {
      // During break/longBreak: skip will go to focus
      // Show brain icon
      if (this.skipBrainIcon) {
        this.skipBrainIcon.style.display = "block";
      }
    }
  }

  async undoLastSession() {
    if (this.completedPomodoros === 0) {
      NotificationUtils.showNotificationPing("No sessions to undo! 🤷‍♂️", "warning");
      return;
    }

    // Decrease completed pomodoros count
    this.completedPomodoros--;

    // Subtract the actual focus time from total (use stored time or fallback to duration)
    const timeToSubtract = this.lastCompletedSessionTime || this.durations.focus;
    this.totalFocusTime = Math.max(0, this.totalFocusTime - timeToSubtract);

    // Return to focus mode
    this.currentMode = "focus";
    this.currentSession = this.completedPomodoros + 1;

    // Restore TagManager display when returning to focus mode
    if (window.tagManager) {
      window.tagManager.updateStatusDisplay();
    }

    // Reset timer to focus duration
    this.timeRemaining = this.durations.focus;
    this.isRunning = false;
    this.isPaused = false;
    this.isAutoPaused = false;
    this._stopTimerLoop();

    // Reset session tracking
    this.sessionStartTime = null;
    this.currentSessionElapsedTime = 0;
    this.lastCompletedSessionTime = 0;
    this.sessionCompletedButNotSaved = false;
    this.maxSessionTimeReached = false;

    // Reset timer accuracy tracking
    this.timerStartTime = null;
    this.timerDuration = null;
    this.lastUpdateTime = null;

    // Update all displays
    this.updateDisplay();
    await this.updateProgressDots();
    await this.saveSessionData();
    this.updateTrayIcon();

    // Show undo notification
    NotificationUtils.showNotificationPing(
      "Last session undone! Back to focus mode 🔄",
      "info",
      this.currentMode
    );

    // Update tray menu
    this.updateTrayMenu();
  }

  generateProgressDots() {
    if (!this.progressDots) {
      return;
    }

    // Clear existing dots
    this.progressDots.innerHTML = "";

    // Generate new dots based on totalSessions
    for (let i = 0; i < this.totalSessions; i++) {
      const dot = document.createElement("div");
      dot.className = "dot";
      this.progressDots.appendChild(dot);
    }
  }

  async updateProgressDots() {
    if (!this.progressDots) {
      return;
    }
    const dots = this.progressDots.querySelectorAll(".dot");

    // Remove any existing overflow indicators
    const existingOverflows = this.progressDots.querySelectorAll(".overflow-indicator");
    existingOverflows.forEach((overflow) => overflow.remove());

    // Get actual completed sessions count from SessionManager
    const actualCompletedSessions = await this.getCompletedSessionsToday();

    // Update each dot based on actual completed sessions and current session
    dots.forEach((dot, index) => {
      // Remove all classes first
      dot.classList.remove("completed", "current");

      if (index < actualCompletedSessions) {
        dot.classList.add("completed");
      } else if (index === actualCompletedSessions && this.currentMode === "focus") {
        dot.classList.add("current");
      }
    });

    if (actualCompletedSessions > this.totalSessions) {
      const overflowCount = actualCompletedSessions - this.totalSessions;
      const overflowIndicator = document.createElement("div");
      overflowIndicator.className = "overflow-indicator";
      overflowIndicator.textContent = `+${overflowCount}`;
      this.progressDots.appendChild(overflowIndicator);
    }
  }

  async addTask() {
    if (!this.taskInput) {
      return;
    } // Skip if task input element doesn't exist

    const taskText = /** @type {HTMLInputElement} */ (this.taskInput).value.trim();
    if (taskText) {
      const task = {
        id: Date.now(),
        text: taskText,
        completed: false,
        created_at: new Date().toISOString(),
      };

      this.tasks.unshift(task);
      await this.saveTasks();
      this.renderTasks();
      /** @type {HTMLInputElement} */ (this.taskInput).value = "";
    }
  }

  async markTaskCompleted(/** @type {string} */ taskText) {
    const task = this.tasks.find((t) => t.text === taskText && !t.completed);
    if (task) {
      task.completed = true;
      task.completed_at = new Date().toISOString();
      await this.saveTasks();
      this.renderTasks();
    }
  }

  async deleteTask(/** @type {any} */ taskId) {
    this.tasks = this.tasks.filter((t) => t.id !== taskId);
    await this.saveTasks();
    this.renderTasks();
  }

  renderTasks() {
    if (!this.taskList) {
      return;
    }

    this.taskList.innerHTML = "";

    const recentTasks = this.tasks.slice(0, 5);

    recentTasks.forEach((task) => {
      const taskEl = document.createElement("div");
      taskEl.className = `task-item ${task.completed ? "completed" : ""}`;

      const span = document.createElement("span");
      span.textContent = task.text;

      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "task-delete";
      btn.textContent = "×";
      btn.setAttribute("aria-label", `Delete task: ${task.text}`);
      btn.addEventListener("click", () => this.deleteTask(task.id));

      taskEl.append(span, btn);
      /** @type {HTMLElement} */ (this.taskList).appendChild(taskEl);
    });
  }

  async updateWeeklyStats() {
    try {
      const history = await invoke("get_stats_history");
      this.renderWeeklyStats(history);
    } catch (error) {
      logger.error("Failed to load history:", error);
      this.renderWeeklyStats([]);
    }
  }

  renderWeeklyStats(/** @type {any[]} */ history) {
    const weeklyStatsContainer = document.getElementById("weekly-stats");
    if (!weeklyStatsContainer) {
      return;
    }

    const today = new Date();
    const weekDays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

    weeklyStatsContainer.innerHTML = "";

    // Generate last 7 days
    for (let i = 6; i >= 0; i--) {
      const date = new Date(today);
      date.setDate(today.getDate() - i);

      const dayElement = document.createElement("div");
      dayElement.className = "day-stat";

      if (i === 0) {
        dayElement.classList.add("today");
      }

      // Find data for this date
      const dayData = history.find((/** @type {any} */ h) => h.date === date.toDateString());
      const completed = dayData ? dayData.completed_pomodoros : 0;
      const focusTime = dayData ? dayData.total_focus_time : 0;

      if (completed > 0) {
        dayElement.classList.add("has-data");
      }

      const hours = Math.floor(focusTime / 3600);
      const minutes = Math.floor((focusTime % 3600) / 60);

      dayElement.innerHTML = `
        <div class="day-label">${weekDays[date.getDay()]}</div>
        <div class="day-count">${completed}</div>
        <div class="day-time">${hours}h ${minutes}m</div>
      `;

      weeklyStatsContainer.appendChild(dayElement);
    }
  }

  async showHistoryModal() {
    try {
      const history = await invoke("get_stats_history");
      this.createHistoryModal(history);
    } catch (error) {
      logger.error("Failed to load history:", error);
      NotificationUtils.showNotificationPing("Failed to load history 😞");
    }
  }

  createHistoryModal(/** @type {any[]} */ history) {
    // Remove existing modal
    const existing = document.querySelector(".history-modal");
    if (existing) {
      existing.remove();
    }

    const modal = document.createElement("div");
    modal.className = "history-modal";

    modal.innerHTML = `
      <div class="history-content">
        <div class="history-header">
          <h3>Your Progress History</h3>
          <button class="close-btn">&times;</button>
        </div>
        <div class="history-list">
          ${
            history.length === 0
              ? "<p>No history data yet. Start your first pomodoro session!</p>"
              : history
                  .slice()
                  .reverse()
                  .map((/** @type {any} */ day) => {
                    const date = new Date(day.date);
                    const hours = Math.floor(day.total_focus_time / 3600);
                    const minutes = Math.floor((day.total_focus_time % 3600) / 60);

                    return `
                <div class="history-item">
                  <div class="history-date">${date.toLocaleDateString("it-IT", {
                    weekday: "long",
                    year: "numeric",
                    month: "long",
                    day: "numeric",
                  })}</div>
                  <div class="history-stats">
                    <span>🍅 ${day.completed_pomodoros} pomodoros</span>
                    <span>⏰ ${hours}h ${minutes}m focus</span>
                  </div>
                </div>
              `;
                  })
                  .join("")
          }
        </div>
      </div>
    `;

    document.body.appendChild(modal);

    // Add event listeners
    const closeBtn = modal.querySelector(".close-btn");
    if (closeBtn) {
      closeBtn.addEventListener("click", () => this.closeHistoryModal());
    }

    modal.addEventListener("click", (e) => {
      if (e.target === modal) {
        this.closeHistoryModal();
      }
    });

    // Show modal with animation
    setTimeout(() => {
      modal.classList.add("show");
    }, 10);
  }

  closeHistoryModal() {
    const modal = document.querySelector(".history-modal");
    if (modal) {
      modal.classList.remove("show");
      setTimeout(() => {
        if (modal.parentNode) {
          modal.parentNode.removeChild(modal);
        }
      }, 300);
    }
  }

  async saveTasks() {
    try {
      await invoke("save_tasks", { tasks: this.tasks });
    } catch (error) {
      logger.error("Failed to save tasks:", error);
      // Fallback to localStorage
      localStorage.setItem("pomodoro-tasks", JSON.stringify(this.tasks));
    }
  }

  async loadTasks() {
    try {
      this.tasks = await invoke("load_tasks");
      logger.info("Loaded tasks from Tauri:", this.tasks.length);
    } catch (error) {
      logger.error("Failed to load tasks from Tauri, using localStorage:", error);
      const saved = localStorage.getItem("pomodoro-tasks");
      this.tasks = saved ? JSON.parse(saved) : [];
    }
  }

  async saveSessionData() {
    const data = {
      completed_pomodoros: this.completedPomodoros,
      total_focus_time: this.totalFocusTime,
      current_session: this.currentSession,
      date: new Date().toDateString(),
    };

    try {
      await invoke("save_session_data", { session: data });
      // Also save daily stats for history
      await invoke("save_daily_stats", { session: data });
    } catch (error) {
      logger.error("Failed to save session data:", error);
      // Fallback to localStorage
      localStorage.setItem("pomodoro-session", JSON.stringify(data));
    }

    // Update navigation charts if navigation manager is available
    if (window.navigationManager) {
      try {
        await window.navigationManager.updateDailyChart();
        await window.navigationManager.updateFocusSummary();
        await window.navigationManager.updateWeeklySessionsChart();
      } catch (error) {
        logger.error("Failed to update navigation charts:", error);
      }
    }
  }

  async getCompletedSessionsToday() {
    if (!window.sessionManager) {
      return this.completedPomodoros; // Fallback to internal counter
    }

    try {
      const today = new Date(); // Pass Date object instead of string
      const todaySessions = await window.sessionManager.getSessionsForDate(today);
      return todaySessions ? todaySessions.length : 0;
    } catch (error) {
      logger.error("Failed to get completed sessions from SessionManager:", error);
      return this.completedPomodoros; // Fallback to internal counter
    }
  }

  async saveCompletedFocusSession() {
    if (!window.sessionManager) {
      logger.debug("SessionManager not available, skipping individual session save");
      return;
    }

    const now = new Date();
    const durationMinutes = Math.round(this.lastCompletedSessionTime / 60);

    // Use preserved session start time if available, otherwise fall back to calculating backwards
    let startHour, startMinute;
    const actualSessionStartTime = this.lastSessionStartTime;

    logger.debug("Session saving debug:", {
      lastSessionStartTime: this.lastSessionStartTime,
      sessionStartTime: this.sessionStartTime,
      actualSessionStartTime,
      durationMinutes,
      nowISO: now.toISOString(),
      nowLocal: now.toString(),
    });

    if (actualSessionStartTime) {
      const sessionStart = new Date(actualSessionStartTime);
      startHour = sessionStart.getHours();
      startMinute = sessionStart.getMinutes();
      logger.debug("Using preserved session start time:", {
        timestampUTC: sessionStart.toISOString(),
        timestampLocal: sessionStart.toString(),
        extractedHour: startHour,
        extractedMinute: startMinute,
      });
    } else {
      // Fallback to calculating backwards from duration
      const endHour = now.getHours();
      const endMinute = now.getMinutes();
      const startTotalMinutes = endHour * 60 + endMinute - durationMinutes;
      startHour = Math.max(0, Math.floor(startTotalMinutes / 60));
      startMinute = Math.max(0, startTotalMinutes % 60);
      logger.debug(
        "Using fallback calculation for session start time (no preserved time available)"
      );
    }

    const endHour = now.getHours();
    const endMinute = now.getMinutes();

    logger.debug("Final time values:", {
      startHour,
      startMinute,
      endHour,
      endMinute,
      startTimeString: `${startHour.toString().padStart(2, "0")}:${startMinute.toString().padStart(2, "0")}`,
      endTimeString: `${endHour.toString().padStart(2, "0")}:${endMinute.toString().padStart(2, "0")}`,
    });

    // Get current tags from TagManager
    const currentTags = window.tagManager ? window.tagManager.getCurrentTags() : [];

    const sessionData = {
      id: crypto.randomUUID(),
      session_type: "focus",
      duration: durationMinutes,
      start_time: `${startHour.toString().padStart(2, "0")}:${startMinute.toString().padStart(2, "0")}`,
      end_time: `${endHour.toString().padStart(2, "0")}:${endMinute.toString().padStart(2, "0")}`,
      notes: this.currentTask && this.currentTask.trim() ? this.currentTask.trim() : null,
      tags: currentTags, // Include current tags from TagManager
      created_at: now.toISOString(),
    };

    try {
      await window.sessionManager.addSession(sessionData);
      logger.info("Timer session saved to SessionManager:", {
        id: sessionData.id,
        duration: sessionData.duration,
        start_time: sessionData.start_time,
        end_time: sessionData.end_time,
        created_at: sessionData.created_at,
      });

      // Clear the preserved session start time after successful save
      this.lastSessionStartTime = null;
      logger.debug("Cleared lastSessionStartTime after successful save");
    } catch (error) {
      logger.error("Failed to save timer session to SessionManager:", error);
    }
  }

  async loadSessionData(forceReset = false) {
    const today = new Date().toDateString();

    // Update current date string for midnight monitoring
    this.currentDateString = today;

    try {
      const data = await invoke("load_session_data");
      if (data && data.date === today && !forceReset) {
        // Load today's session data
        this.completedPomodoros = data.completed_pomodoros || 0;
        this.totalFocusTime = data.total_focus_time || 0;
        this.currentSession = data.current_session || 1;
        await this.updateProgressDots();
        logger.info("📊 Loaded existing session data for today");
      } else {
        // Reset to default values for new day, no data, or forced reset
        this.completedPomodoros = 0;
        this.totalFocusTime = 0;
        this.currentSession = 1;
        await this.updateProgressDots();
        logger.info("🌅 Reset session data for new day or forced reset");
      }
    } catch (error) {
      logger.error("Failed to load session data from Tauri, using localStorage:", error);
      const saved = localStorage.getItem("pomodoro-session");

      if (saved) {
        const data = JSON.parse(saved);
        if (data.date === today && !forceReset) {
          // Load today's session data from localStorage
          this.completedPomodoros = data.completedPomodoros || 0;
          this.totalFocusTime = data.totalFocusTime || 0;
          this.currentSession = data.currentSession || 1;
          await this.updateProgressDots();
          logger.info("📊 Loaded existing session data from localStorage");
        } else {
          // Reset to default values for new day, no data, or forced reset
          this.completedPomodoros = 0;
          this.totalFocusTime = 0;
          this.currentSession = 1;
          await this.updateProgressDots();
          logger.info("🌅 Reset session data from localStorage for new day or forced reset");
        }
      } else {
        // No saved data at all, reset to defaults
        this.completedPomodoros = 0;
        this.totalFocusTime = 0;
        this.currentSession = 1;
        await this.updateProgressDots();
        logger.info("🌅 No saved data found, using defaults");
      }
    }
  }

  async showNotification() {
    // Only show desktop notifications if the setting is enabled
    if (!this.enableDesktopNotifications) {
      logger.debug("🔔 Desktop notifications are disabled in settings");
      return;
    }

    const messages = {
      focus: "Break time! Take a rest 😌",
      break: "Break over! Time to focus 🍅",
      longBreak: "Long break over! Ready for more focus? 🚀",
    };

    const notificationTitle = "Presto - Pomodoro Timer";
    const notificationBody = /** @type {Record<string, string>} */ (messages)[this.currentMode];

    logger.debug(`🔔 Attempting to show desktop notification: "${notificationBody}"`);

    try {
      // Check if we're in a Tauri context and use Tauri notifications
      if (window.__TAURI__ && window.__TAURI__.notification) {
        logger.debug("🔔 Using Tauri notification system");
        const { isPermissionGranted, requestPermission, sendNotification } =
          window.__TAURI__.notification;

        // Check if permission is granted
        let permissionGranted = await isPermissionGranted();
        logger.debug(`🔔 Tauri notification permission status: ${permissionGranted}`);

        // If not granted, request permission
        if (!permissionGranted) {
          logger.debug("🔔 Requesting Tauri notification permission...");
          const permission = await requestPermission();
          permissionGranted = permission === "granted";
          logger.debug(
            `🔔 Permission request result: ${permission} (granted: ${permissionGranted})`
          );

          if (!permissionGranted) {
            logger.warn("❌ Tauri notification permission was denied");
            NotificationUtils.showNotificationPing(
              "Desktop notifications are disabled. Enable them in system settings to get timer alerts! 🔔",
              "warning",
              this.currentMode
            );
            return;
          }
        }

        // Send notification if permission is granted
        if (permissionGranted) {
          logger.debug("🔔 Sending Tauri notification...");
          await sendNotification({
            title: notificationTitle,
            body: notificationBody,
            icon: "/assets/tauri.svg",
          });
          logger.debug("✅ Tauri notification sent successfully");
        } else {
          logger.warn("❌ Tauri notification permission not available");
          this.fallbackToWebNotifications(notificationTitle, notificationBody);
        }
      } else {
        logger.debug("🔔 Tauri not available, falling back to Web Notification API");
        this.fallbackToWebNotifications(notificationTitle, notificationBody);
      }
    } catch (error) {
      logger.error("❌ Failed to show Tauri notification:", error);
      logger.debug("🔄 Attempting fallback to Web Notification API...");
      this.fallbackToWebNotifications(notificationTitle, notificationBody);
    }
  }

  async fallbackToWebNotifications(/** @type {string} */ title, /** @type {string} */ body) {
    try {
      if ("Notification" in window) {
        logger.debug(`🔔 Web Notification API available, permission: ${Notification.permission}`);

        if (Notification.permission === "granted") {
          logger.debug("🔔 Sending Web notification...");
          NotificationUtils.showDesktopNotification(title, body);
          logger.debug("✅ Web notification sent successfully");
        } else if (Notification.permission === "default") {
          logger.debug("🔔 Requesting Web notification permission...");
          const permission = await Notification.requestPermission();
          logger.debug(`🔔 Web permission request result: ${permission}`);

          if (permission === "granted") {
            NotificationUtils.showDesktopNotification(title, body);
            logger.debug("✅ Web notification sent after permission granted");
          } else {
            logger.warn("❌ Web notification permission was denied");
            NotificationUtils.showNotificationPing(
              "Desktop notifications are disabled. Enable them in your browser to get timer alerts! 🔔",
              "warning",
              this.currentMode
            );
          }
        } else {
          logger.warn("❌ Web notification permission was previously denied");
          NotificationUtils.showNotificationPing(
            "Desktop notifications are disabled. Enable them in your browser settings to get timer alerts! 🔔",
            "warning",
            this.currentMode
          );
        }
      } else {
        logger.warn("❌ Web Notification API not supported");
        this.fallbackToInAppNotification(body);
      }
    } catch (error) {
      logger.error("❌ Failed to show Web notification:", error);
      this.fallbackToInAppNotification(body);
    }
  }

  fallbackToInAppNotification(/** @type {string} */ message) {
    logger.debug("🔔 Using in-app notification as final fallback");
    NotificationUtils.showNotificationPing(message, "info", this.currentMode);
  }

  // Usage: Open browser console and type: window.pomodoroTimer.testNotification()
  async testNotification() {
    logger.debug("🧪 Testing notification system...");
    logger.debug(
      "📝 Instructions: This will test the notification system and show debug info in the console"
    );
    logger.debug(`🔧 Current settings: desktop notifications = ${this.enableDesktopNotifications}`);

    // Detect if we're in development mode
    const isDevMode = window.location.protocol === "tauri:" ? false : true;
    const bundleId = "com.presto.app";

    logger.debug(
      `🔧 Environment: ${isDevMode ? "Development (tauri dev)" : "Production (built app)"}`
    );
    logger.debug(`🔧 Bundle ID: ${bundleId}`);

    if (isDevMode) {
      logger.debug("⚠️  IMPORTANT: You're running in development mode (tauri dev)");
      logger.debug("⚠️  On macOS, Tauri notifications often don't work in dev mode due to:");
      logger.debug(
        "   1. Tauri uses Terminal.app for dev mode, which may not have notification permissions"
      );
      logger.debug("   2. Bundle identifier is handled differently in dev vs production");
      logger.debug("   3. macOS requires proper app bundle registration for notifications");
      logger.debug("");
      logger.debug("🔧 To test notifications properly:");
      logger.debug("   1. Run: npm run tauri build");
      logger.debug("   2. Install the built app from src-tauri/target/release/bundle/");
      logger.debug("   3. Test notifications in the installed production app");
      logger.debug("");
      logger.debug("🔧 For dev mode, check Terminal.app permissions:");
      logger.debug("   - System Preferences > Notifications & Focus > Terminal");
      logger.debug('   - Make sure "Allow Notifications" is enabled');
      logger.debug("");
    }

    // Show in-app notification first
    NotificationUtils.showNotificationPing(
      "Testing notification system... 🧪",
      "info",
      this.currentMode
    );

    // Test desktop notification
    /** @type {any} */
    const originalSetting = this.enableDesktopNotifications;
    /** @type {any} */ (this).enableDesktopNotifications = true; // Temporarily enable for testing

    try {
      await this.showNotification();
      logger.debug(
        "✅ Test notification API call completed - check console logs above for detailed debug info"
      );
      logger.debug("🔍 Look for messages starting with 🔔 for notification flow details");

      if (isDevMode) {
        logger.debug("");
        logger.debug(
          '⚠️  If you see "✅ Tauri notification sent successfully" but no notification appeared:'
        );
        logger.debug("   - This is NORMAL in development mode on macOS");
        logger.debug("   - Test with a production build to verify notifications work");
        logger.debug("");
        logger.debug("🔄 Trying Web Notification API as fallback...");
        await this.testWebNotificationFallback();
      }
    } catch (error) {
      logger.error("❌ Test notification failed:", error);
      logger.debug("💡 Troubleshooting steps:");
      if (isDevMode) {
        logger.debug("   1. This is likely due to dev mode limitations on macOS");
        logger.debug("   2. Check Terminal.app notification permissions in System Preferences");
        logger.debug("   3. Test with a production build: npm run tauri build");
      } else {
        logger.debug(
          "   1. Check if notifications are enabled in System Preferences > Notifications"
        );
        logger.debug('   2. Look for "presto" or "com.presto.app" in the notifications list');
        logger.debug('   3. Ensure "Allow Notifications" is enabled for the app');
      }
    } finally {
      // Restore original setting
      /** @type {any} */ (this).enableDesktopNotifications = originalSetting;
    }
  }

  async testWebNotificationFallback() {
    try {
      if ("Notification" in window) {
        logger.debug("🌐 Web Notification API available");
        logger.debug(`🌐 Current permission: ${Notification.permission}`);

        if (Notification.permission === "default") {
          logger.debug("🌐 Requesting Web notification permission...");
          const permission = await Notification.requestPermission();
          logger.debug(`🌐 Permission result: ${permission}`);
        }

        if (Notification.permission === "granted") {
          logger.debug("🌐 Sending Web notification...");
          const notification = new Notification("Presto - Test Web Notification", {
            body: "This is a fallback Web notification test",
            icon: "/assets/tauri.svg",
          });

          notification.onshow = () => logger.debug("✅ Web notification displayed");
          notification.onerror = (error) => logger.error("❌ Web notification error:", error);

          // Auto-close after 5 seconds
          setTimeout(() => {
            notification.close();
            logger.debug("🌐 Web notification closed automatically");
          }, 5000);
        } else {
          logger.debug("❌ Web notification permission denied");
        }
      } else {
        logger.debug("❌ Web Notification API not available");
      }
    } catch (error) {
      logger.error("❌ Web notification test failed:", error);
    }
  }

  addWarningClass() {
    const container = document.querySelector(".timer-container");
    if (container && !container.classList.contains("warning")) {
      container.classList.add("warning");
    }
  }

  async updateTrayIcon() {
    try {
      // Check status bar display setting
      const settingsManager = window.settingsManager;
      const statusBarDisplay = settingsManager
        ? settingsManager.settings.status_bar_display
        : "default";

      let displayText = "";
      let modeIcon;

      // Define icons for different modes
      const modeIcons = {
        focus: "🧠",
        break: "☕",
        longBreak: "🌙",
      };

      // Show pause icon if timer is paused or auto-paused
      if (this.isPaused || this.isAutoPaused) {
        modeIcon = "⏸️";
      } else if (this.timeRemaining < 0 && this.allowContinuousSessions) {
        // Show overtime indicator in tray
        modeIcon = "⏰";
      } else {
        modeIcon = /** @type {Record<string, string>} */ (modeIcons)[this.currentMode] || "🧠";
      }

      // Set display text based on status bar display mode
      if (statusBarDisplay === "icon-only") {
        // Show only the mode icon in displayText, and pass empty string for modeIcon to avoid duplication
        displayText = modeIcon;
        modeIcon = ""; // Clear modeIcon to avoid showing it twice
      } else {
        // Default mode: show timer in mm:ss format
        let displayMinutes,
          displaySeconds,
          isOvertime = false;

        if (this.timeRemaining < 0 && this.allowContinuousSessions) {
          // Show overtime in continuous sessions
          isOvertime = true;
          const overtimeSeconds = Math.abs(this.timeRemaining);
          displayMinutes = Math.floor(overtimeSeconds / 60);
          displaySeconds = overtimeSeconds % 60;
        } else {
          // Normal display or traditional mode
          const absTime = Math.abs(this.timeRemaining);
          displayMinutes = Math.floor(absTime / 60);
          displaySeconds = absTime % 60;
        }

        const timerText = `${displayMinutes.toString().padStart(2, "0")}:${displaySeconds.toString().padStart(2, "0")}`;
        const overtimePrefix = isOvertime ? "+" : "";
        const fullTimerText = `${overtimePrefix}${timerText}`;

        // Show timer with session counter
        const sessionCounter = `(${this.completedPomodoros}/${this.totalSessions})`;
        const realText = `${fullTimerText} ${sessionCounter}`;

        // Add invisible characters (zero-width spaces) to pad to maximum possible length
        // Maximum length would be: "+99:99 (99/99)" = 14 characters
        const maxLength = 14;
        const padding = "\u200B".repeat(maxLength - realText.length); // zero-width space
        displayText = realText + padding;
      }

      await invoke("update_tray_icon", {
        timerText: displayText,
        isRunning: this.isRunning,
        sessionMode: this.currentMode,
        currentSession: this.currentSession,
        totalSessions: this.totalSessions,
        modeIcon,
      });
    } catch (error) {
      logger.warn("Failed to update tray icon:", error);
    }
  }

  /**
   * @typedef {{ focus_duration: number, break_duration: number, long_break_duration: number, total_sessions: number, max_session_time: number, weekly_goal_minutes?: number }} AppSettingsTimer
   * @typedef {{ desktop_notifications: boolean, sound_notifications: boolean, auto_start_timer: boolean, allow_continuous_sessions: boolean, smart_pause: boolean, smart_pause_timeout: number }} AppSettingsNotifications
   * @typedef {{ start_stop?: string | null, reset?: string | null, skip?: string | null }} AppSettingsShortcuts
   * @typedef {{ theme?: string, timer_theme?: string }} AppSettingsAppearance
   * @typedef {{ debug_mode?: boolean }} AppSettingsAdvanced
   * @typedef {{ timer: AppSettingsTimer, notifications: AppSettingsNotifications, shortcuts: AppSettingsShortcuts, appearance: AppSettingsAppearance, advanced?: AppSettingsAdvanced }} AppSettings
   */

  /** @param {AppSettings} settings */
  async applySettings(settings) {
    // Update timer durations
    this.durations.focus = settings.timer.focus_duration * 60;
    this.durations.break = settings.timer.break_duration * 60;
    this.durations.longBreak = settings.timer.long_break_duration * 60;

    // Apply debug mode if enabled (overrides all timer durations)
    if (settings.advanced?.debug_mode) {
      logger.info("Debug mode enabled - all timers set to 3 seconds");
      this.durations.focus = 3;
      this.durations.break = 3;
      this.durations.longBreak = 3;
    }

    this.totalSessions = settings.timer.total_sessions;

    // Update max session time (convert from minutes to milliseconds)
    this.maxSessionTime = (settings.timer.max_session_time || 120) * 60 * 1000;

    // If timer is not running, update current time remaining
    if (!this.isRunning) {
      this.timeRemaining = /** @type {Record<string, number>} */ (this.durations)[this.currentMode];
      this.updateDisplay();
    }

    // Regenerate progress dots when total sessions change
    this.generateProgressDots();
    await this.updateProgressDots();

    // Update notification preferences
    this.enableDesktopNotifications = settings.notifications.desktop_notifications;
    this.enableSoundNotifications = settings.notifications.sound_notifications;

    // Enable auto-start using the dedicated method
    await this.enableAutoStart(settings.notifications.auto_start_timer);

    // Enable continuous sessions using the dedicated method
    await this.enableContinuousSessions(settings.notifications.allow_continuous_sessions || false);

    // Update debug mode state
    this.debugMode = settings.advanced?.debug_mode || false;

    logger.info("🔧 Applying settings in PomodoroTimer:");
    logger.info("Auto-start timer setting:", this.autoStartTimer);
    logger.info("Allow continuous sessions setting:", this.allowContinuousSessions);
    logger.info("Debug mode setting:", this.debugMode);

    // Update smart pause setting and timeout
    this.inactivityThreshold = (settings.notifications.smart_pause_timeout || 30) * 1000; // convert to milliseconds

    // Update keyboard shortcuts
    if (settings.shortcuts) {
      this.updateKeyboardShortcuts(settings.shortcuts);
    }

    // Update backend timeout if smart pause is enabled and monitoring is active
    if (this.smartPauseEnabled) {
      try {
        const timeoutSeconds = Math.floor(this.inactivityThreshold / 1000);
        await invoke("update_activity_timeout", { timeoutSeconds });
      } catch (error) {
        logger.debug("Activity monitoring not active or failed to update timeout:", error);
      }
    }

    // Enable smart pause using the dedicated method
    await this.enableSmartPause(settings.notifications.smart_pause);

    // Update all setting indicators to reflect current state
    logger.debug("🔄 Calling updateSettingIndicators from applySettings...");
    this.updateSettingIndicators();
  }

  resetToInitialState() {
    // Stop any running timer
    this.isRunning = false;
    this.isPaused = false;
    this.isAutoPaused = false;
    if (this.timerInterval) {
      clearInterval(this.timerInterval);
    }

    // Clear smart pause timeout and disable
    if (this.activityTimeout) {
      clearTimeout(this.activityTimeout);
      this.activityTimeout = null;
    }
    this.stopSmartPauseCountdown();
    this.smartPauseEnabled = false;
    this.inactivityThreshold = 30000; // Reset to default 30 seconds

    // Clear midnight monitoring
    this.stopMidnightMonitoring();

    // Reset all counters and state
    this.completedPomodoros = 0;
    this.generateProgressDots();
    this.updateProgressDots();
    this.currentSession = 1;
    this.totalFocusTime = 0;
    this.currentMode = "focus";

    // Reset session tracking
    this.sessionStartTime = null;
    this.currentSessionElapsedTime = 0;
    this.lastCompletedSessionTime = 0;
    this.maxSessionTimeReached = false; // Reset max session time flag

    // Reset timer accuracy tracking
    this.timerStartTime = null;
    this.timerDuration = null;
    this.lastUpdateTime = null;

    // Reset durations to defaults
    this.durations = {
      focus: 25 * 60,
      break: 5 * 60,
      longBreak: 20 * 60,
    };
    this.totalSessions = 10;

    // Reset timer display
    this.timeRemaining = /** @type {Record<string, number>} */ (this.durations)[this.currentMode];

    // Clear task input
    this.currentTask = "";
    if (this.taskInput) {
      /** @type {HTMLInputElement} */ (this.taskInput).value = "";
    }

    // Reset notification preferences to defaults
    this.autoStartTimer = true;
    this.allowContinuousSessions = false;

    // Update all displays
    this.updateDisplay();
    this.renderTasks();
    this.updateWeeklyStats();
    this.updateTrayIcon();
    this.updateSettingIndicators();

    // Restart midnight monitoring
    this.startMidnightMonitoring();

    logger.info("Timer reset to initial state");
  }
}
