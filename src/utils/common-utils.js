// Notification Utility Functions
export class NotificationUtils {
  // Static properties for notification queue management
  static notificationQueue = [];
  static activeNotifications = new Set();
  static maxSimultaneousNotifications = 3;
  static lastNotificationTimes = new Map(); // Track last notification times to prevent spam

  static showNotificationPing(message, type = null, timerState = null) {
    // Prevent spam notifications - check if the same message was shown recently
    const now = Date.now();
    const lastTime = this.lastNotificationTimes.get(message);
    const cooldownTime = type === "success" && message.includes("Settings saved") ? 1000 : 500;

    if (lastTime && now - lastTime < cooldownTime) {
      return; // Skip if shown too recently
    }

    this.lastNotificationTimes.set(message, now);

    // Clean up old entries from notification times cache (keep only last 10 minutes)
    for (const [msg, time] of this.lastNotificationTimes.entries()) {
      if (now - time > 600000) {
        // 10 minutes
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
      if (type === "success" && message.includes("Settings saved")) {
        this.queueNotification(message, type, timerState);
        return;
      }

      // For high priority notifications, dismiss the oldest one and wait briefly
      const oldestNotification = container.querySelector(".notification-ping");
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
          this.refreshNotification(existing);
        }
        return;
      }
    }

    // Create new notification
    const notification = document.createElement("div");

    // Determina la classe CSS da usare
    let notificationClass = "notification-ping";

    if (timerState) {
      notificationClass += ` ${timerState}`;
    } else if (type) {
      notificationClass += ` ${type}`;
    } else {
      notificationClass += " info";
    }

    notification.className = notificationClass;

    notification.setAttribute("role", "alert");
    notification.setAttribute("aria-live", "polite");

    // Contenuto semplice e pulito
    notification.textContent = message;

    // Add unique ID for tracking
    const notificationId = `notification-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
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

    const baseDuration = type === "success" && message.includes("Settings saved") ? 2000 : 3000;
    const extraTime = Math.max(0, (message.length - 30) * 50);
    const duration = Math.min(baseDuration + extraTime, 6000);

    const dismissTimer = setTimeout(() => {
      if (notification && notification.parentNode) {
        this.dismissNotification(notification);
      }
    }, duration);

    this.addMobileTouchHandlers(notification, dismissTimer);
  }

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
      const { message, type, timerState } = this.notificationQueue.shift();
      this.showNotificationPing(message, type, timerState);
    }
  }

  static refreshNotification(notification) {
    // Add a refresh animation class
    notification.classList.add("refreshing");

    // Remove the class after animation
    setTimeout(() => {
      notification.classList.remove("refreshing");
    }, 300);
  }

  static addMobileTouchHandlers(notification, dismissTimer) {
    let startY = 0;
    let currentY = 0;
    let isDragging = false;

    // Touch start
    notification.addEventListener(
      "touchstart",
      (e) => {
        startY = e.touches[0].clientY;
        isDragging = false;
        notification.style.transition = "none";
      },
      { passive: true }
    );

    // Touch move (swipe to dismiss)
    notification.addEventListener(
      "touchmove",
      (e) => {
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
            notification.style.opacity = opacity;
          }
        }
      },
      { passive: true }
    );

    // Touch end
    notification.addEventListener(
      "touchend",
      (_e) => {
        if (isDragging) {
          const deltaY = startY - currentY;
          notification.style.transition = "all 0.3s ease";

          if (deltaY > 50) {
            clearTimeout(dismissTimer);
            this.dismissNotification(notification);
          } else {
            // Ripristina posizione
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

    notification.addEventListener("click", (_e) => {
      if (!("ontouchstart" in window)) {
        clearTimeout(dismissTimer);
        this.dismissNotification(notification);
      }
    });
  }

  static triggerMobileHaptics(type) {
    if (
      "vibrate" in navigator &&
      /Android|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent)
    ) {
      let pattern = [100];

      switch (type) {
        case "success":
          pattern = [100, 50, 100];
          break;
        case "warning":
          pattern = [200];
          break;
        case "error":
          pattern = [100, 100, 100, 100, 100];
          break;
        default:
          pattern = [50];
      }

      navigator.vibrate(pattern);
    }
  }

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
    setTimeout(() => {
      if (notification.parentNode) {
        notification.parentNode.removeChild(notification);
      }
    }, 300);

    // Process queued notifications immediately after tracking removal
    // Use a small delay to ensure DOM update
    setTimeout(() => {
      this.processNotificationQueue();
    }, 50);
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
      console.warn("Failed to play notification sound:", error);
    }
  }

  static async showDesktopNotification(title, message, icon = "/assets/tauri.svg") {
    try {
      // Check if we're in a Tauri context
      if (window.__TAURI__ && window.__TAURI__.notification) {
        const { isPermissionGranted, requestPermission, sendNotification } =
          window.__TAURI__.notification;

        // Check if permission is granted
        let permissionGranted = await isPermissionGranted();

        // If not granted, request permission
        if (!permissionGranted) {
          const permission = await requestPermission();
          permissionGranted = permission === "granted";
        }

        // Send notification if permission is granted
        if (permissionGranted) {
          await sendNotification({
            title,
            body: message,
            icon,
          });
        } else {
          console.warn("Notification permission denied");
        }
      } else {
        // Fallback to Web Notification API if not in Tauri context
        if ("Notification" in window && Notification.permission === "granted") {
          new Notification(title, {
            body: message,
            icon,
            silent: false,
            requireInteraction: false,
          });
        }
      }
    } catch (error) {
      console.error("Failed to show desktop notification:", error);
      // Fallback to Web Notification API
      if ("Notification" in window && Notification.permission === "granted") {
        new Notification(title, {
          body: message,
          icon,
          silent: false,
          requireInteraction: false,
        });
      }
    }
  }

  // Helper method to check notification permission status
  static async getNotificationPermission() {
    try {
      if (window.__TAURI__ && window.__TAURI__.notification) {
        const { isPermissionGranted } = window.__TAURI__.notification;
        const granted = await isPermissionGranted();
        return granted ? "granted" : "denied";
      } else {
        // Fallback to Web API
        if (!("Notification" in window)) {
          return "unsupported";
        }
        return Notification.permission;
      }
    } catch (error) {
      console.error("Failed to check notification permission:", error);
      return "denied";
    }
  }

  // Helper method to request notification permission (must be called from user gesture)
  static async requestNotificationPermission() {
    try {
      if (window.__TAURI__ && window.__TAURI__.notification) {
        const { requestPermission } = window.__TAURI__.notification;
        const permission = await requestPermission();
        return permission;
      } else {
        // Fallback to Web API
        if (!("Notification" in window)) {
          return "unsupported";
        }

        const permission = await Notification.requestPermission();
        return permission;
      }
    } catch (error) {
      console.error("Failed to request notification permission:", error);
      return "denied";
    }
  }

  static createNotificationContent(message, type) {
    const icons = {
      success: "✅",
      warning: "⚠️",
      error: "❌",
      info: "ℹ️",
      focus: "🍅",
      break: "☕",
      longBreak: "🌙",
    };

    const icon = icons[type] || icons.info;

    const isMobile = /Android|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(
      navigator.userAgent
    );

    if (isMobile) {
      return `
                <div class="notification-content-mobile">
                    <span class="notification-icon">${icon}</span>
                    <span class="notification-message">${message}</span>
                    <span class="notification-dismiss-hint">↑</span>
                </div>
            `;
    } else {
      return `
                <div class="notification-content-desktop">
                    <span class="notification-icon">${icon}</span>
                    <span class="notification-message">${message}</span>
                </div>
            `;
    }
  }
}

// Time Formatting Utilities
export class TimeUtils {
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

  static formatTimeDetailed(seconds) {
    if (!seconds || seconds < 0) {
      return "0h 0m";
    }

    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);

    return `${hours}h ${minutes}m`;
  }

  static getWeekStart(date) {
    const start = new Date(date);
    const day = start.getDay();
    const diff = start.getDate() - day + (day === 0 ? -6 : 1); // Adjust when day is Sunday
    start.setDate(diff);
    return start;
  }

  static isSameDay(date1, date2) {
    return date1.toDateString() === date2.toDateString();
  }

  static formatDateRange(startDate, endDate) {
    const formatOptions = { day: "numeric", month: "short" };
    const startStr = startDate.toLocaleDateString("en-US", formatOptions);
    const endStr = endDate.toLocaleDateString("en-US", formatOptions);
    const year = endDate.getFullYear();

    return `${startStr} - ${endStr} ${year}`;
  }
}

// Storage Utilities
export class StorageUtils {
  static async saveToTauri(invokeCommand, data, fallbackKey) {
    try {
      const { invoke } = window.__TAURI__.core;
      await invoke(invokeCommand, data);
      return true;
    } catch (error) {
      console.error(`Failed to save to Tauri (${invokeCommand}):`, error);
      // Fallback to localStorage
      localStorage.setItem(fallbackKey, JSON.stringify(data));
      return false;
    }
  }

  static async loadFromTauri(invokeCommand, fallbackKey) {
    try {
      const { invoke } = window.__TAURI__.core;
      return await invoke(invokeCommand);
    } catch (error) {
      console.error(`Failed to load from Tauri (${invokeCommand}):`, error);
      // Fallback to localStorage
      const saved = localStorage.getItem(fallbackKey);
      return saved ? JSON.parse(saved) : null;
    }
  }

  static saveToLocalStorage(key, data) {
    try {
      localStorage.setItem(key, JSON.stringify(data));
      return true;
    } catch (error) {
      console.error(`Failed to save to localStorage (${key}):`, error);
      return false;
    }
  }

  static loadFromLocalStorage(key, defaultValue = null) {
    try {
      const saved = localStorage.getItem(key);
      return saved ? JSON.parse(saved) : defaultValue;
    } catch (error) {
      console.error(`Failed to load from localStorage (${key}):`, error);
      return defaultValue;
    }
  }
}

// DOM Utilities
export class DOMUtils {
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
    const closeBtn = modal.querySelector(".close-btn");
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

  static closeModal(modal) {
    if (!modal) {
      modal = document.querySelector(".modal-overlay");
    }

    if (modal) {
      modal.classList.remove("show");
      setTimeout(() => {
        if (modal.parentNode) {
          modal.parentNode.removeChild(modal);
        }
      }, 300);
    }
  }

  static updateElementText(elementId, text) {
    const element = document.getElementById(elementId);
    if (element) {
      element.textContent = text;
    }
  }

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

// Keyboard Shortcut Utilities
export class KeyboardUtils {
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

    parts.forEach((part) => {
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
