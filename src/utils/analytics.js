// Analytics utility for tracking events with Aptabase
import { trackEvent } from "@aptabase/tauri";

/**
 * Analytics utility class for tracking user events
 */
class Analytics {
  /**
   * Check if analytics are enabled in user settings
   * @returns {Promise<boolean>} Whether analytics are enabled
   */
  static async isEnabled() {
    try {
      // Import settings manager to check analytics preference
      // Note: This is a dynamic import to avoid circular dependencies
      const settingsManager = window.settingsManager;
      if (settingsManager && settingsManager.settings) {
        return settingsManager.settings.analytics_enabled !== false;
      }
      // Default to enabled if we can't check settings
      return true;
    } catch (error) {
      console.warn("Could not check analytics settings, defaulting to enabled:", error);
      return true;
    }
  }

  /**
   * Track an event only if analytics are enabled
   * @param {string} eventName - The event name
   * @param {object} properties - Event properties
   */
  static async track(eventName, properties = {}) {
    if (await this.isEnabled()) {
      try {
        trackEvent(eventName, properties);
      } catch (error) {
        console.warn("Failed to track analytics event:", error);
      }
    }
  }

  /**
   * Track timer-related events
   */
  static timer = {
    /**
     * Track when a timer is started
     * @param {string} mode - The timer mode (focus, break, longBreak)
     * @param {number} duration - Duration in minutes
     */
    async started(mode, duration) {
      await Analytics.track("timer_started", {
        mode,
        duration_minutes: duration,
      });
    },

    /**
     * Track when a timer is paused
     * @param {string} mode - The timer mode
     * @param {number} remainingTime - Remaining time in seconds
     */
    async paused(mode, remainingTime) {
      await Analytics.track("timer_paused", {
        mode,
        remaining_seconds: remainingTime,
      });
    },

    /**
     * Track when a timer is completed
     * @param {string} mode - The timer mode
     * @param {number} duration - Duration in minutes
     */
    async completed(mode, duration) {
      await Analytics.track("timer_completed", {
        mode,
        duration_minutes: duration,
      });
    },

    /**
     * Track when a timer is skipped
     * @param {string} mode - The timer mode
     * @param {number} remainingTime - Remaining time in seconds
     */
    async skipped(mode, remainingTime) {
      await Analytics.track("timer_skipped", {
        mode,
        remaining_seconds: remainingTime,
      });
    },

    /**
     * Track when a timer is reset
     * @param {string} mode - The timer mode
     */
    async reset(mode) {
      await Analytics.track("timer_reset", { mode });
    },
  };

  /**
   * Track task-related events
   */
  static tasks = {
    /**
     * Track when a task is created
     */
    async created() {
      await Analytics.track("task_created");
    },

    /**
     * Track when a task is completed
     */
    async completed() {
      await Analytics.track("task_completed");
    },

    /**
     * Track when a task is deleted
     */
    async deleted() {
      await Analytics.track("task_deleted");
    },

    /**
     * Track when tasks are bulk imported/exported
     * @param {number} count - Number of tasks
     * @param {string} action - 'import' or 'export'
     */
    async bulkAction(count, action) {
      await Analytics.track("tasks_bulk_action", {
        count,
        action,
      });
    },
  };

  /**
   * Track feature usage
   */
  static features = {
    /**
     * Track when a specific feature is used
     * @param {string} feature - Feature name
     * @param {Object} properties - Additional properties
     */
    async used(feature, properties = {}) {
      await Analytics.track("feature_used", {
        feature,
        ...properties,
      });
    },

    /**
     * Track shortcuts usage
     * @param {string} shortcut - Shortcut name (start-stop, reset, skip)
     */
    async shortcutUsed(shortcut) {
      await Analytics.track("shortcut_used", { shortcut });
    },

    /**
     * Track when smart pause is triggered
     * @param {number} inactiveTime - Time inactive in seconds
     */
    async smartPauseTriggered(inactiveTime) {
      await Analytics.track("smart_pause_triggered", {
        inactive_seconds: inactiveTime,
      });
    },

    /**
     * Track view changes
     * @param {string} view - View name (timer, tasks, statistics, settings)
     */
    async viewChanged(view) {
      await Analytics.track("view_changed", { view });
    },
  };

  /**
   * Track session-related events
   */
  static sessions = {
    /**
     * Track session completion
     * @param {number} completedPomodoros - Number of completed pomodoros
     * @param {number} totalFocusTime - Total focus time in minutes
     */
    async completed(completedPomodoros, totalFocusTime) {
      await Analytics.track("session_completed", {
        completed_pomodoros: completedPomodoros,
        total_focus_minutes: Math.round(totalFocusTime / 60),
      });
    },

    /**
     * Track daily goal achievement
     * @param {number} goalMinutes - Goal in minutes
     * @param {number} achievedMinutes - Achieved minutes
     */
    async goalProgress(goalMinutes, achievedMinutes) {
      const percentage = Math.round((achievedMinutes / goalMinutes) * 100);
      await Analytics.track("daily_goal_progress", {
        goal_minutes: goalMinutes,
        achieved_minutes: achievedMinutes,
        percentage,
      });
    },
  };

  /**
   * Track errors and issues
   */
  /**
   * Track errors and issues
   */
  static errors = {
    /**
     * Track when an error occurs
     * @param {string} error - Error message or type
     * @param {string} context - Where the error occurred
     */
    async occurred(error, context) {
      await Analytics.track("error_occurred", {
        error: String(error),
        context,
      });
    },
  };
}

export default Analytics;
