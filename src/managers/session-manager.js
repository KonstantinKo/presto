import { NotificationUtils } from "../utils/common-utils.js";
import { logger } from "../utils/logger.js";

const { invoke } = /** @type {{ invoke: ((cmd: string, args?: any) => Promise<any>) | null }} */ (
  window.__TAURI__ ? window.__TAURI__.core : { invoke: null }
);

export class SessionManager {
  /** @param {any} navigationManager */
  constructor(navigationManager) {
    this.navManager = navigationManager;
    this.currentEditingSession = null;
    this.selectedDate = null;
    /** @type {any} */
    this.sessions = [];
    this.isUsingTauri = !!invoke;
    this.init();
  }

  async init() {
    await this.loadSessionsFromStorage();
    this.setupEventListeners();
  }

  async loadSessionsFromStorage() {
    try {
      if (this.isUsingTauri) {
        const sessions = await /** @type {NonNullable<typeof invoke>} */ (invoke)(
          "load_manual_sessions"
        );

        this.sessions = {};
        sessions.forEach((/** @type {any} */ session) => {
          if (!this.sessions[session.date]) {
            this.sessions[session.date] = [];
          }
          this.sessions[session.date].push(session);
        });

        logger.info("Loaded", sessions.length, "manual sessions from Tauri backend");
      } else {
        const savedSessions = localStorage.getItem("presto_manual_sessions");
        if (savedSessions) {
          this.sessions = JSON.parse(savedSessions);
          logger.info("Loaded manual sessions from localStorage (fallback)");
        }
      }
    } catch (error) {
      logger.error("Error loading sessions from storage:", error);
      this.sessions = {};
    }
  }

  async saveSessionsToStorage() {
    try {
      if (this.isUsingTauri) {
        const sessionsArray = /** @type {any[]} */ ([]);
        Object.keys(this.sessions).forEach((date) => {
          this.sessions[date].forEach((/** @type {any} */ session) => {
            sessionsArray.push({
              ...session,
              date,
            });
          });
        });

        await /** @type {NonNullable<typeof invoke>} */ (invoke)("save_manual_sessions", {
          sessions: sessionsArray,
        });
        logger.info("Saved", sessionsArray.length, "manual sessions to Tauri backend");
      } else {
        localStorage.setItem("presto_manual_sessions", JSON.stringify(this.sessions));
        logger.info("Saved manual sessions to localStorage (fallback)");
      }
    } catch (error) {
      logger.error("Error saving sessions to storage:", error);
    }
  }

  setupEventListeners() {
    const addSessionBtn = document.getElementById("add-session-btn");
    if (addSessionBtn) {
      addSessionBtn.addEventListener("click", () => this.openAddSessionModal());
    }

    const modalOverlay = document.getElementById("session-modal-overlay");
    const closeModalBtn = document.getElementById("close-session-modal");
    const cancelBtn = document.getElementById("cancel-session-btn");
    const sessionForm = document.getElementById("session-form");
    const deleteSessionBtn = document.getElementById("delete-session-btn");

    if (modalOverlay) {
      modalOverlay.addEventListener("click", (e) => {
        if (e.target === modalOverlay) {
          this.closeModal();
        }
      });
    }

    if (closeModalBtn) {
      closeModalBtn.addEventListener("click", () => this.closeModal());
    }

    if (cancelBtn) {
      cancelBtn.addEventListener("click", () => this.closeModal());
    }

    if (sessionForm) {
      sessionForm.addEventListener("submit", (e) => {
        e.preventDefault();
        this.saveSession();
      });
    }

    if (deleteSessionBtn) {
      deleteSessionBtn.addEventListener("click", () => this.deleteCurrentSession());
    }

    this.setupTimeCalculation();

    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && this.isModalOpen()) {
        this.closeModal();
      }
    });
  }

  /** @param {Date | null} [date] */
  openAddSessionModal(date = null) {
    this.selectedDate = date || this.navManager.currentDate || new Date();
    this.currentEditingSession = null;

    const modal = /** @type {HTMLElement} */ (document.getElementById("session-modal-overlay"));
    const modalTitle = /** @type {HTMLElement} */ (document.getElementById("session-modal-title"));
    const deleteBtn = /** @type {HTMLElement} */ (document.getElementById("delete-session-btn"));
    const saveBtn = /** @type {HTMLElement} */ (document.getElementById("save-session-btn"));

    modalTitle.textContent = "Add Session";
    deleteBtn.style.display = "none";
    saveBtn.textContent = "Save Session";

    const now = new Date();
    const startMinutes = Math.min(now.getHours() * 60 + now.getMinutes(), 23 * 60 + 58);
    const startTime = this.minutesToTime(startMinutes);
    const endTime = this.calculateEndTime(startTime, 25);
    const actualDuration = Math.max(1, this.timeToMinutes(endTime) - startMinutes);

    /** @type {HTMLFormElement} */ (document.getElementById("session-form")).reset();
    /** @type {HTMLInputElement} */ (document.getElementById("session-duration")).value =
      String(actualDuration);
    /** @type {HTMLInputElement} */ (document.getElementById("session-start-time")).value =
      startTime;
    /** @type {HTMLInputElement} */ (document.getElementById("session-end-time")).value = endTime;

    modal.classList.add("show");
    /** @type {HTMLInputElement} */ (document.getElementById("session-start-time")).focus();
  }

  /** @param {any} session @param {any} date */
  openEditSessionModal(session, date) {
    this.selectedDate = new Date(date);
    this.currentEditingSession = session;

    const modal = /** @type {HTMLElement} */ (document.getElementById("session-modal-overlay"));
    const modalTitle = /** @type {HTMLElement} */ (document.getElementById("session-modal-title"));
    const deleteBtn = /** @type {HTMLElement} */ (document.getElementById("delete-session-btn"));
    const saveBtn = /** @type {HTMLElement} */ (document.getElementById("save-session-btn"));

    modalTitle.textContent = "Edit Session";
    deleteBtn.style.display = "block";
    saveBtn.textContent = "Update Session";

    /** @type {HTMLInputElement} */ (document.getElementById("session-duration")).value = String(
      session.duration
    );
    /** @type {HTMLInputElement} */ (document.getElementById("session-start-time")).value =
      session.start_time;
    /** @type {HTMLInputElement} */ (document.getElementById("session-end-time")).value =
      session.end_time;

    modal.classList.add("show");
    /** @type {HTMLInputElement} */ (document.getElementById("session-start-time")).focus();
  }

  closeModal() {
    const modal = document.getElementById("session-modal-overlay");
    if (modal) {
      modal.classList.remove("show");
    }
    this.currentEditingSession = null;
    this.selectedDate = null;
  }

  isModalOpen() {
    const modal = document.getElementById("session-modal-overlay");
    return modal && modal.classList.contains("show");
  }

  setupTimeCalculation() {
    const startTimeInput = /** @type {HTMLInputElement | null} */ (
      document.getElementById("session-start-time")
    );
    const endTimeInput = /** @type {HTMLInputElement | null} */ (
      document.getElementById("session-end-time")
    );
    const durationInput = /** @type {HTMLInputElement | null} */ (
      document.getElementById("session-duration")
    );

    const calculateDuration = () => {
      if (!startTimeInput || !endTimeInput || !durationInput) {
        return;
      }
      const startTime = startTimeInput.value;
      const endTime = endTimeInput.value;

      if (startTime && endTime) {
        const startMinutes = this.timeToMinutes(startTime);
        const endMinutes = this.timeToMinutes(endTime);
        let duration = endMinutes - startMinutes;

        if (duration < 0) {
          duration += 24 * 60;
        }

        durationInput.value = String(duration);
      }
    };

    const calculateEndTime = () => {
      if (!startTimeInput || !endTimeInput || !durationInput) {
        return;
      }
      const startTime = startTimeInput.value;
      const duration = parseInt(durationInput.value, 10);

      if (startTime && Number.isInteger(duration) && duration >= 0) {
        const endTime = this.calculateEndTime(startTime, duration);
        endTimeInput.value = endTime;
        durationInput.value = String(
          Math.max(0, this.timeToMinutes(endTime) - this.timeToMinutes(startTime))
        );
      }
    };

    if (startTimeInput && endTimeInput && durationInput) {
      startTimeInput.addEventListener("change", calculateDuration);
      endTimeInput.addEventListener("change", calculateDuration);
      durationInput.addEventListener("change", calculateEndTime);
    }
  }

  /** @param {string} timeString @returns {number} */
  timeToMinutes(timeString) {
    const [hours, minutes] = timeString.split(":").map(Number);
    return hours * 60 + minutes;
  }

  /** @param {number} minutes @returns {string} */
  minutesToTime(minutes) {
    const totalMinutes = minutes % (24 * 60);
    const hours = Math.floor(totalMinutes / 60);
    const mins = totalMinutes % 60;
    return `${hours.toString().padStart(2, "0")}:${mins.toString().padStart(2, "0")}`;
  }

  async saveSession() {
    const formData = new FormData(
      /** @type {HTMLFormElement} */ (document.getElementById("session-form"))
    );
    const startTime = /** @type {string | null} */ (formData.get("startTime"));
    const endTime = /** @type {string | null} */ (formData.get("endTime"));
    const duration = parseInt(/** @type {string} */ (formData.get("duration")), 10);

    const sessionData = {
      id: this.currentEditingSession?.id || this.generateSessionId(),
      session_type: "focus",
      duration,
      start_time: startTime,
      end_time: endTime,
      created_at: this.currentEditingSession?.created_at || new Date().toISOString(),
      tags: this.currentEditingSession?.tags || [],
    };

    if (!sessionData.start_time) {
      alert("Please enter a start time");
      return;
    }

    if (!sessionData.end_time) {
      alert("Please enter an end time");
      return;
    }

    if (isNaN(sessionData.duration) || sessionData.duration <= 0) {
      alert("Please enter a valid duration");
      return;
    }

    if (sessionData.end_time <= sessionData.start_time) {
      alert("End time must be after start time");
      return;
    }

    try {
      if (this.currentEditingSession) {
        await this.updateSession(sessionData);
        NotificationUtils.showNotificationPing("Session updated successfully", "success");
      } else {
        await this.addSession(sessionData);
        NotificationUtils.showNotificationPing("Session added successfully", "success");
      }

      const dateForRefresh = this.selectedDate;
      this.closeModal();

      if (this.navManager) {
        await this.navManager.updateSelectedDayDetails(dateForRefresh);
        await this.navManager.updateFocusSummary();
        await this.navManager.updateWeeklySessionsChart();
        await this.navManager.updateDailyChart();
      }
    } catch (error) {
      logger.error("Error saving session:", error);
      NotificationUtils.showNotificationPing("Failed to save session", "error");
    }
  }

  /** @param {any} sessionData */
  async addSession(sessionData) {
    const targetDate = this.selectedDate || new Date();
    const dateString = targetDate.toDateString();

    if (!this.sessions[dateString]) {
      this.sessions[dateString] = [];
    }

    this.sessions[dateString].push(sessionData);

    await this.saveSessionsToStorage();

    window.dispatchEvent(
      new CustomEvent("sessionAdded", {
        detail: { sessionData, date: dateString },
      })
    );
  }

  /** @param {any} sessionData */
  async updateSession(sessionData) {
    const dateString = this.selectedDate.toDateString();

    if (this.sessions[dateString]) {
      const index = this.sessions[dateString].findIndex(
        (/** @type {any} */ s) => s.id === sessionData.id
      );
      if (index !== -1) {
        this.sessions[dateString][index] = sessionData;
      }
    }

    await this.saveSessionsToStorage();

    window.dispatchEvent(
      new CustomEvent("sessionUpdated", {
        detail: { sessionData, date: dateString },
      })
    );
  }

  async deleteCurrentSession() {
    if (!this.currentEditingSession) {
      return;
    }

    try {
      const dateString = this.selectedDate.toDateString();

      if (this.sessions[dateString]) {
        this.sessions[dateString] = this.sessions[dateString].filter(
          (/** @type {any} */ s) => s.id !== this.currentEditingSession.id
        );
      }

      await this.saveSessionsToStorage();

      window.dispatchEvent(
        new CustomEvent("sessionDeleted", {
          detail: { sessionId: this.currentEditingSession.id, date: dateString },
        })
      );

      const dateForRefresh = this.selectedDate;
      this.closeModal();
      NotificationUtils.showNotificationPing("Session deleted successfully", "success");

      if (this.navManager) {
        await this.navManager.updateSelectedDayDetails(dateForRefresh);
        await this.navManager.updateFocusSummary();
        await this.navManager.updateWeeklySessionsChart();
        await this.navManager.updateDailyChart();
      }
    } catch (error) {
      logger.error("Error deleting session:", error);
      NotificationUtils.showNotificationPing("Failed to delete session", "error");
    }
  }

  /** @param {Date} date @returns {any[]} */
  getSessionsForDate(date) {
    const dateString = date.toDateString();
    return this.sessions[dateString] || [];
  }

  /** @returns {string} */
  generateSessionId() {
    return Date.now().toString() + Math.random().toString(36).substring(2, 11);
  }

  /** @param {string} startTime @param {number} durationMinutes @returns {string} */
  calculateEndTime(startTime, durationMinutes) {
    const startMinutes = this.timeToMinutes(startTime);
    const endMinutes = startMinutes + durationMinutes;
    if (endMinutes >= 24 * 60) {
      return "23:59";
    }
    return this.minutesToTime(endMinutes);
  }
}
