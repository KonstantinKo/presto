import { trackEvent } from "@aptabase/tauri";
import { logger } from "./logger.js";

class Analytics {
  static async isEnabled() {
    try {
      const settings = window.settingsManager?.settings;
      return settings ? settings.analytics_enabled !== false : true;
    } catch (error) {
      logger.warn("Could not check analytics settings, defaulting to enabled:", error);
      return true;
    }
  }

  static async track(eventName, properties = {}) {
    if (!(await this.isEnabled())) {
      return;
    }
    try {
      trackEvent(eventName, properties);
    } catch (error) {
      logger.warn("Failed to track analytics event:", error);
    }
  }

  static timer = {
    async started(mode, duration) {
      await Analytics.track("timer_started", { mode, duration_minutes: duration });
    },

    async paused(mode, remainingTime) {
      await Analytics.track("timer_paused", { mode, remaining_seconds: remainingTime });
    },

    async completed(mode, duration) {
      await Analytics.track("timer_completed", { mode, duration_minutes: duration });
    },

    async skipped(mode, remainingTime) {
      await Analytics.track("timer_skipped", { mode, remaining_seconds: remainingTime });
    },

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

    async bulkAction(count, action) {
      await Analytics.track("tasks_bulk_action", { count, action });
    },
  };

  static features = {
    async used(feature, properties = {}) {
      await Analytics.track("feature_used", { feature, ...properties });
    },

    async shortcutUsed(shortcut) {
      await Analytics.track("shortcut_used", { shortcut });
    },

    async smartPauseTriggered(inactiveTime) {
      await Analytics.track("smart_pause_triggered", { inactive_seconds: inactiveTime });
    },

    async viewChanged(view) {
      await Analytics.track("view_changed", { view });
    },
  };

  static sessions = {
    async completed(completedPomodoros, totalFocusTime) {
      await Analytics.track("session_completed", {
        completed_pomodoros: completedPomodoros,
        total_focus_minutes: Math.round(totalFocusTime / 60),
      });
    },

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
    async occurred(error, context) {
      await Analytics.track("error_occurred", { error: String(error), context });
    },
  };
}

export default Analytics;
