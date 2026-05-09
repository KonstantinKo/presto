/** @type {Readonly<Record<string, string>>} */
const STORAGE_KEYS = Object.freeze({
  SESSION: "pomodoro-session",
  TASKS: "pomodoro-tasks",
  SETTINGS: "pomodoro-settings",
  HISTORY: "pomodoro-history",
  STATS: "pomodoro-stats",
  TAGS: "presto-tags",
  SKIPPED_VERSIONS: "presto-skipped-versions",
  FORCE_UPDATE_TEST: "presto_force_update_test",
  AUTO_CHECK_UPDATES: "presto_auto_check_updates",
  AUTH_SEEN: "presto-auth-seen",
  GUEST_MODE: "presto-guest-mode",
  MANUAL_SESSIONS: "presto_manual_sessions",
  THEME_PREFERENCE: "theme-preference",
  TIMER_THEME_PREFERENCE: "timer-theme-preference",
});

export default STORAGE_KEYS;
export { STORAGE_KEYS };
