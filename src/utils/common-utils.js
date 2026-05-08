import { logger } from "./logger.js";

const NOTIFICATION_CACHE_TTL_MS = 10 * 60 * 1000; // 10 minutes

/**
 * @param {string} message
 * @param {string | null} type
 * @returns {boolean}
 */
function isSettingsSaved(message, type) {
  return type === "success" && message.includes("Settings saved");
}

export class NotificationUtils {
  static notificationQueue =
    /** @type {Array<{message: string, type: string|null, timerState: string|null}>} */ ([]);
  static activeNotifications = new Set();
  static maxSimultaneousNotifications = 3;
  static lastNotificationTimes = new Map(); // Track last notification times to prevent spam

  /**
   * @param {string} message
   * @param {string | null} [type]
   * @param {string | null} [timerState]
   */
  static showNotificationPing(message, type = null, timerState = null) {
    // Prevent spam notifications - check if the same message was shown recently
    const now = Date.now();
    const lastTime = this.lastNotificationTimes.get(message);
    const cooldownTime = isSettingsSaved(message, type) ? 1000 : 500;

    if (lastTime && now - lastTime < cooldownTime) {
      return; // Skip if shown too recently
    }

    this.lastNotificationTimes.set(message, now);

    // Clean up old entries from notification times cache
    for (const [msg, time] of this.lastNotificationTimes.entries()) {
      if (now - time > NOTIFICATION_CACHE_TTL_MS) {
        this.lastNotificationTimes.delete(msg);
      }
    }

    // Ensure notification container exists
    let container = document.querySelector(".notification-container");
    if (!container) {
      container = document.createElement("div");
      container.className = "notification-container";
      document.body.appendChild(container);
    }

    // Check if we have too many active notifications
    if (this.activeNotifications.size >= this.maxSimultaneousNotifications) {
      // If it's a low priority notification (like auto-save), queue it
      if (isSettingsSaved(message, type)) {
        this.queueNotification(message, type, timerState);
        return;
      }

      // For high priority notifications, dismiss the oldest one and wait briefly
      const oldestNotification = /** @type {HTMLElement | null} */ (
        container.querySelector(".notification-ping")
      );
      if (oldestNotification) {
        this.dismissNotification(oldestNotification);
        // Wait a moment for the dismissal to process before showing new notification
        setTimeout(() => {
          this.showNotificationPing(message, type, timerState);
        }, 100);
        return;
      }
    }

    // Check for duplicate messages and update if found
    const existingNotifications = container.querySelectorAll(".notification-ping");
    for (const existing of existingNotifications) {
      if (existing.textContent === message) {
        // Don't refresh if notification is already dismissing
        if (!existing.classList.contains("dismissing")) {
          // Update existing notification instead of creating a new one
          this.refreshNotification(/** @type {HTMLElement} */ (existing));
        }
        return;
      }
    }

    // Create new notification
    const notification = document.createElement("div");
    const variant = timerState || type || "info";
    notification.className = `notification-ping ${variant}`;

    notification.setAttribute("role", "alert");
    notification.setAttribute("aria-live", "polite");
    notification.textContent = message;

    // Add unique ID for tracking
    const notificationId = `notification-${Date.now()}-${Math.random().toString(36).substring(2, 11)}`;
    notification.setAttribute("data-notification-id", notificationId);
    this.activeNotifications.add(notificationId);

    // Start with entering class for initial hidden state
    notification.classList.add("entering");

    container.appendChild(notification);

    // Force a reflow to ensure the initial styles are applied
    // eslint-disable-next-line no-unused-expressions -- intentional layout trigger
    notification.offsetHeight;

    // Start the animation by removing the entering class
    requestAnimationFrame(() => {
      if (notification.parentNode) {
        notification.classList.remove("entering");
      }
    });

    this.triggerMobileHaptics(type);

    const baseDuration = isSettingsSaved(message, type) ? 2000 : 3000;
    const extraTime = Math.max(0, (message.length - 30) * 50);
    const duration = Math.min(baseDuration + extraTime, 6000);

    const dismissTimer = setTimeout(() => {
      if (notification && notification.parentNode) {
        this.dismissNotification(notification);
      }
    }, duration);

    this.addMobileTouchHandlers(notification, dismissTimer);
  }

  /**
   * @param {string} message
   * @param {string | null} type
   * @param {string | null} timerState
   */
  static queueNotification(message, type, timerState) {
    // Check if this notification is already in the queue
    const isDuplicate = this.notificationQueue.some((item) => item.message === message);
    if (isDuplicate) {
      return; // Don't queue duplicates
    }

    this.notificationQueue.push({ message, type, timerState });

    // Process queue immediately when a notification slot becomes available
    // Don't use setTimeout to avoid animation conflicts
    this.processNotificationQueue();
  }

  static processNotificationQueue() {
    if (
      this.notificationQueue.length > 0 &&
      this.activeNotifications.size < this.maxSimultaneousNotifications
    ) {
      const item = this.notificationQueue.shift();
      if (item) {
        this.showNotificationPing(item.message, item.type, item.timerState);
      }
    }
  }

  /** @param {HTMLElement} notification */
  static refreshNotification(notification) {
    // Add a refresh animation class
    notification.classList.add("refreshing");

    // Remove the class after animation
    setTimeout(() => {
      notification.classList.remove("refreshing");
    }, 300);
  }

  /**
   * @param {HTMLElement} notification
   * @param {ReturnType<typeof setTimeout>} dismissTimer
   */
  static addMobileTouchHandlers(notification, dismissTimer) {
    let startY = 0;
    let currentY = 0;
    let isDragging = false;

    notification.addEventListener(
      "touchstart",
      (/** @type {TouchEvent} */ e) => {
        startY = e.touches[0].clientY;
        isDragging = false;
        notification.style.transition = "none";
      },
      { passive: true }
    );

    notification.addEventListener(
      "touchmove",
      (/** @type {TouchEvent} */ e) => {
        if (!startY) {
          return;
        }

        currentY = e.touches[0].clientY;
        const deltaY = startY - currentY;

        if (Math.abs(deltaY) > 10) {
          isDragging = true;
          if (deltaY > 0) {
            const opacity = Math.max(0.3, 1 - deltaY / 100);
            const translateY = Math.min(deltaY, 50);
            notification.style.transform = `translateY(-${translateY}px)`;
            notification.style.opacity = String(opacity);
          }
        }
      },
      { passive: true }
    );

    notification.addEventListener(
      "touchend",
      (/** @type {TouchEvent} */ _e) => {
        if (isDragging) {
          const deltaY = startY - currentY;
          notification.style.transition = "all 0.3s ease";

          if (deltaY > 50) {
            clearTimeout(dismissTimer);
            this.dismissNotification(notification);
          } else {
            // Restore original position
            notification.style.transform = "translateY(0)";
            notification.style.opacity = "1";
          }
        } else {
          clearTimeout(dismissTimer);
          this.dismissNotification(notification);
        }

        startY = 0;
        isDragging = false;
      },
      { passive: true }
    );

    notification.addEventListener("click", (/** @type {MouseEvent} */ _e) => {
      if (!("ontouchstart" in window)) {
        clearTimeout(dismissTimer);
        this.dismissNotification(notification);
      }
    });
  }

  /** @param {string | null} type */
  static triggerMobileHaptics(type) {
    if (
      "vibrate" in navigator &&
      /Android|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent)
    ) {
      const patterns = /** @type {Record<string, number[]>} */ ({
        success: [100, 50, 100],
        warning: [200],
        error: [100, 100, 100, 100, 100],
      });
      navigator.vibrate(patterns[type ?? ""] ?? [50]);
    }
  }

  /** @param {HTMLElement} notification */
  static dismissNotification(notification) {
    if (!notification || !notification.parentNode) {
      return;
    }

    // Remove from active notifications tracking immediately
    const notificationId = notification.getAttribute("data-notification-id");
    if (notificationId) {
      this.activeNotifications.delete(notificationId);
    }

    // Add dismissing class for animation
    notification.classList.add("dismissing");

    // Wait for animation to complete before removing from DOM
    setTimeout(() => notification.remove(), 300);

    // Process queued notifications shortly after tracking removal
    setTimeout(() => this.processNotificationQueue(), 50);
  }

  static playNotificationSound() {
    try {
      // Create a simple beep sound
      const audioContext = new (window.AudioContext || window.webkitAudioContext)();
      const oscillator = audioContext.createOscillator();
      const gainNode = audioContext.createGain();

      oscillator.connect(gainNode);
      gainNode.connect(audioContext.destination);

      oscillator.frequency.value = 800;
      oscillator.type = "sine";

      gainNode.gain.setValueAtTime(0.3, audioContext.currentTime);
      gainNode.gain.exponentialRampToValueAtTime(0.01, audioContext.currentTime + 0.5);

      oscillator.start(audioContext.currentTime);
      oscillator.stop(audioContext.currentTime + 0.5);
    } catch (error) {
      logger.warn("Failed to play notification sound:", error);
    }
  }

  /** @param {string} title @param {string} message @param {string} [icon] */
  static showWebNotification(title, message, icon) {
    if ("Notification" in window && Notification.permission === "granted") {
      new Notification(title, {
        body: message,
        icon,
        silent: false,
        requireInteraction: false,
      });
    }
  }

  /** @param {string} title @param {string} message @param {string} [icon] */
  static async showDesktopNotification(title, message, icon = "/assets/tauri.svg") {
    try {
      // Check if we're in a Tauri context
      if (window.__TAURI__ && window.__TAURI__.notification) {
        const { isPermissionGranted, requestPermission, sendNotification } =
          window.__TAURI__.notification;

        // Check if permission is granted, request if not
        let permissionGranted = await isPermissionGranted();
        if (!permissionGranted) {
          permissionGranted = (await requestPermission()) === "granted";
        }

        if (permissionGranted) {
          await sendNotification({ title, body: message, icon });
        } else {
          logger.warn("Notification permission denied");
        }
      } else {
        // Fallback to Web Notification API if not in Tauri context
        this.showWebNotification(title, message, icon);
      }
    } catch (error) {
      logger.error("Failed to show desktop notification:", error);
      // Fallback to Web Notification API
      this.showWebNotification(title, message, icon);
    }
  }

  static async getNotificationPermission() {
    try {
      if (window.__TAURI__ && window.__TAURI__.notification) {
        const granted = await window.__TAURI__.notification.isPermissionGranted();
        return granted ? "granted" : "denied";
      }
      if (!("Notification" in window)) {
        return "unsupported";
      }
      return Notification.permission;
    } catch (error) {
      logger.error("Failed to check notification permission:", error);
      return "denied";
    }
  }

  // must be called from user gesture
  static async requestNotificationPermission() {
    try {
      if (window.__TAURI__ && window.__TAURI__.notification) {
        return await window.__TAURI__.notification.requestPermission();
      }
      if (!("Notification" in window)) {
        return "unsupported";
      }
      return await Notification.requestPermission();
    } catch (error) {
      logger.error("Failed to request notification permission:", error);
      return "denied";
    }
  }
}

export class TimeUtils {
  /** @param {number} seconds @returns {string} */
  static formatTime(seconds) {
    if (!seconds || seconds < 0) {
      return "0m";
    }

    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const remainingSeconds = seconds % 60;

    if (hours > 0) {
      return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`;
    } else if (minutes > 0) {
      return `${minutes}m`;
    } else {
      return `${remainingSeconds}s`;
    }
  }

  /** @param {number} seconds @returns {string} */
  static formatTimeDetailed(seconds) {
    if (!seconds || seconds < 0) {
      return "0h 0m";
    }

    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);

    return `${hours}h ${minutes}m`;
  }

  /** @param {Date} date @returns {Date} */
  static getWeekStart(date) {
    const start = new Date(date);
    const day = start.getDay();
    const diff = start.getDate() - day + (day === 0 ? -6 : 1); // Adjust when day is Sunday
    start.setDate(diff);
    return start;
  }

  /** @param {Date} date1 @param {Date} date2 @returns {boolean} */
  static isSameDay(date1, date2) {
    return date1.toDateString() === date2.toDateString();
  }

  /** @param {Date} startDate @param {Date} endDate @returns {string} */
  static formatDateRange(startDate, endDate) {
    const formatOptions = /** @type {Intl.DateTimeFormatOptions} */ ({
      day: "numeric",
      month: "short",
    });
    const startStr = startDate.toLocaleDateString("en-US", formatOptions);
    const endStr = endDate.toLocaleDateString("en-US", formatOptions);
    const year = endDate.getFullYear();

    return `${startStr} - ${endStr} ${year}`;
  }
}

export class StorageUtils {
  /**
   * @param {string} invokeCommand
   * @param {any} data
   * @param {string} fallbackKey
   * @returns {Promise<boolean>}
   */
  static async saveToTauri(invokeCommand, data, fallbackKey) {
    try {
      const { invoke } = /** @type {{ invoke: (cmd: string, args?: any) => Promise<any> }} */ (
        window.__TAURI__?.core ?? {}
      );
      await invoke(invokeCommand, data);
      return true;
    } catch (error) {
      logger.error(`Failed to save to Tauri (${invokeCommand}):`, error);
      // Fallback to localStorage
      localStorage.setItem(fallbackKey, JSON.stringify(data));
      return false;
    }
  }

  /**
   * @param {string} invokeCommand
   * @param {string} fallbackKey
   * @returns {Promise<any>}
   */
  static async loadFromTauri(invokeCommand, fallbackKey) {
    try {
      const { invoke } = /** @type {{ invoke: (cmd: string, args?: any) => Promise<any> }} */ (
        window.__TAURI__?.core ?? {}
      );
      return await invoke(invokeCommand);
    } catch (error) {
      logger.error(`Failed to load from Tauri (${invokeCommand}):`, error);
      // Fallback to localStorage
      const saved = localStorage.getItem(fallbackKey);
      return saved ? JSON.parse(saved) : null;
    }
  }

  /** @param {string} key @param {any} data @returns {boolean} */
  static saveToLocalStorage(key, data) {
    try {
      localStorage.setItem(key, JSON.stringify(data));
      return true;
    } catch (error) {
      logger.error(`Failed to save to localStorage (${key}):`, error);
      return false;
    }
  }

  /** @param {string} key @param {any} [defaultValue] @returns {any} */
  static loadFromLocalStorage(key, defaultValue = null) {
    try {
      const saved = localStorage.getItem(key);
      return saved ? JSON.parse(saved) : defaultValue;
    } catch (error) {
      logger.error(`Failed to load from localStorage (${key}):`, error);
      return defaultValue;
    }
  }
}

export class DOMUtils {
  /**
   * @param {string} title
   * @param {string} content
   * @param {string} [className]
   * @returns {HTMLElement}
   */
  static createModal(title, content, className = "") {
    // Remove existing modal if any
    const existing = document.querySelector(".modal-overlay");
    if (existing) {
      existing.remove();
    }

    const overlay = document.createElement("div");
    overlay.className = `modal-overlay ${className}`;

    const modal = document.createElement("div");
    modal.className = "modal-content";

    modal.innerHTML = `
      <div class="modal-header">
        <h3>${title}</h3>
        <button class="close-btn">&times;</button>
      </div>
      <div class="modal-body">
        ${content}
      </div>
    `;

    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    // Add event listeners
    const closeBtn = /** @type {HTMLElement} */ (modal.querySelector(".close-btn"));
    closeBtn.addEventListener("click", () => this.closeModal(overlay));

    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) {
        this.closeModal(overlay);
      }
    });

    // Show modal with animation
    setTimeout(() => {
      overlay.classList.add("show");
    }, 10);

    return overlay;
  }

  /** @param {HTMLElement | null} [modal] */
  static closeModal(modal) {
    const target = modal || document.querySelector(".modal-overlay");
    if (!target) {
      return;
    }
    target.classList.remove("show");
    setTimeout(() => target.remove(), 300);
  }

  /** @param {string} elementId @param {string} text */
  static updateElementText(elementId, text) {
    const element = document.getElementById(elementId);
    if (element) {
      element.textContent = text;
    }
  }

  /**
   * @param {HTMLElement | null} element
   * @param {string} className
   * @param {boolean | null} [condition]
   */
  static toggleClass(element, className, condition = null) {
    if (!element) {
      return;
    }

    if (condition === null) {
      element.classList.toggle(className);
    } else if (condition) {
      element.classList.add(className);
    } else {
      element.classList.remove(className);
    }
  }
}

export class KeyboardUtils {
  /** @param {string | null} shortcutString */
  static parseShortcut(shortcutString) {
    if (!shortcutString) {
      return null;
    }

    const parts = shortcutString.split("+");
    const result = {
      meta: false,
      ctrl: false,
      alt: false,
      shift: false,
      key: "",
    };

    parts.forEach((/** @type {string} */ part) => {
      const partLower = part.toLowerCase();
      switch (partLower) {
        case "commandorcontrol":
        case "cmd":
        case "command":
          result.meta = true;
          result.ctrl = true; // For cross-platform compatibility
          break;
        case "alt":
          result.alt = true;
          break;
        case "shift":
          result.shift = true;
          break;
        case "space":
          result.key = " ";
          break;
        default:
          result.key = partLower;
      }
    });

    return result;
  }

  /** @param {KeyboardEvent} event @param {string} shortcutString @returns {boolean} */
  static matchesShortcut(event, shortcutString) {
    const shortcut = this.parseShortcut(shortcutString);
    if (!shortcut) {
      return false;
    }

    const eventKey = event.key.toLowerCase();

    // Handle special key matches
    let keyMatches = false;
    if (shortcut.key === " ") {
      keyMatches = event.code === "Space" || eventKey === " ";
    } else if (shortcut.key === "s") {
      keyMatches = eventKey === "s" || event.code === "KeyS";
    } else if (shortcut.key === "r") {
      keyMatches = eventKey === "r" || event.code === "KeyR";
    } else {
      keyMatches = shortcut.key === eventKey;
    }

    const modifiersMatch =
      (!shortcut.meta || event.metaKey || event.ctrlKey) &&
      (!shortcut.ctrl || event.ctrlKey || event.metaKey) &&
      (!shortcut.alt || event.altKey) &&
      (!shortcut.shift || event.shiftKey);

    return keyMatches && modifiersMatch;
  }
}
