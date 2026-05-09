import { trackEvent } from "@aptabase/tauri";
import { logger } from "./logger.js";

class Analytics {
  static isEnabled() {
    const settings = window.settingsManager?.settings;
    return !!settings && settings.analytics_enabled === true;
  }

  /** @param {string} eventName @param {Record<string, string | number>} [properties] */
  static async track(eventName, properties = {}) {
    if (!this.isEnabled()) {
      return;
    }
    try {
      trackEvent(eventName, properties);
    } catch (error) {
      logger.warn("Failed to track analytics event:", error);
    }
  }

  static timer = {
    /** @param {string} mode @param {number} duration */
    async started(mode, duration) {
      await Analytics.track("timer_started", { mode, duration_minutes: duration });
    },
    /** @param {string} mode @param {number} remainingTime */
    async paused(mode, remainingTime) {
      await Analytics.track("timer_paused", { mode, remaining_seconds: remainingTime });
    },
    /** @param {string} mode @param {number} duration */
    async completed(mode, duration) {
      await Analytics.track("timer_completed", { mode, duration_minutes: duration });
    },
    /** @param {string} mode @param {number} remainingTime */
    async skipped(mode, remainingTime) {
      await Analytics.track("timer_skipped", { mode, remaining_seconds: remainingTime });
    },
    /** @param {string} mode */
    async reset(mode) {
      await Analytics.track("timer_reset", { mode });
    },
  };

  static tasks = {
    async created() {
      await Analytics.track("task_created");
    },
    async completed() {
      await Analytics.track("task_completed");
    },
    async deleted() {
      await Analytics.track("task_deleted");
    },
    /** @param {number} count @param {string} action */
    async bulkAction(count, action) {
      await Analytics.track("tasks_bulk_action", { count, action });
    },
  };

  static features = {
    /** @param {string} feature @param {Record<string, string | number>} [properties] */
    async used(feature, properties = {}) {
      await Analytics.track("feature_used", { feature, ...properties });
    },
    /** @param {string} shortcut */
    async shortcutUsed(shortcut) {
      await Analytics.track("shortcut_used", { shortcut });
    },
    /** @param {number} inactiveTime */
    async smartPauseTriggered(inactiveTime) {
      await Analytics.track("smart_pause_triggered", { inactive_seconds: inactiveTime });
    },
    /** @param {string} view */
    async viewChanged(view) {
      await Analytics.track("view_changed", { view });
    },
  };

  static sessions = {
    /** @param {number} completedPomodoros @param {number} totalFocusTime */
    async completed(completedPomodoros, totalFocusTime) {
      await Analytics.track("session_completed", {
        completed_pomodoros: completedPomodoros,
        total_focus_minutes: Math.round(totalFocusTime / 60),
      });
    },
    /** @param {number} goalMinutes @param {number} achievedMinutes */
    async goalProgress(goalMinutes, achievedMinutes) {
      const percentage = Math.round((achievedMinutes / goalMinutes) * 100);
      await Analytics.track("daily_goal_progress", {
        goal_minutes: goalMinutes,
        achieved_minutes: achievedMinutes,
        percentage,
      });
    },
  };

  static errors = {
    /** @param {unknown} error @param {string} context */
    async occurred(error, context) {
      await Analytics.track("error_occurred", { error: String(error), context });
    },
  };
}

export default Analytics;
