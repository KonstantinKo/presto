// TODO(stack-swap): this fixture mocks the Tauri JS bridge by setting window.__TAURI__.
// After the Leptos/WASM swap, the bridge shape will change (or be replaced by a
// Trunk-served WASM binary that talks to Tauri via a different IPC). Re-implement
// this fixture against whatever the new bridge boundary is. The spec files do not
// depend on this file's internals; they depend only on user-visible UI and on the
// high-level seedX/setX configuration helpers exposed via window.__E2E_TEST_HARNESS__.

/**
 * Browser-context init script installed via addInitScript before any page navigation.
 * It runs before all page scripts and installs:
 * - window.supabase mock (so auth-manager doesn't timeout waiting for the CDN)
 * - window.__TAURI__ mock with in-memory command handlers
 * - window.__E2E_TEST_HARNESS__ for test harness access
 *
 * Configuration is read lazily at invoke-time from window.__E2E_CONFIG__, which lets
 * override scripts added AFTER this one (but before goto) take effect.
 */
const TAURI_MOCK_INIT_SCRIPT = `
(function () {
  // Ensure config namespace exists; override scripts may add to it after this script runs
  if (!window.__E2E_CONFIG__) {
    window.__E2E_CONFIG__ = {};
  }

  // --- Supabase mock ---
  // waitForSupabase() in src/utils/supabase.js polls window.supabase; provide it
  // immediately so auth-manager resolves without the 5-second CDN timeout.
  var _authCallbacks = [];
  window.supabase = {
    createClient: function (url, key, opts) {
      return {
        auth: {
          getSession: async function () {
            return { data: { session: null }, error: null };
          },
          onAuthStateChange: function (callback) {
            _authCallbacks.push(callback);
            return {
              data: {
                subscription: {
                  unsubscribe: function () {
                    _authCallbacks = _authCallbacks.filter(function (c) {
                      return c !== callback;
                    });
                  },
                },
              },
            };
          },
          signInWithPassword: async function (creds) {
            var user = {
              id: "mock-user-id",
              email: creds.email || "test@example.com",
              user_metadata: { full_name: "Test User", name: "Test User" },
            };
            var session = {
              user: user,
              access_token: "mock-access-token",
              refresh_token: "mock-refresh-token",
            };
            setTimeout(function () {
              _authCallbacks.forEach(function (cb) {
                try { cb("SIGNED_IN", session); } catch (e) {}
              });
            }, 80);
            return { data: { session: session, user: user }, error: null };
          },
          signUp: async function () {
            return { data: null, error: { message: "Registration not mocked in E2E tests" } };
          },
          signInWithOAuth: async function () {
            return { data: null, error: { message: "OAuth not mocked in E2E tests" } };
          },
          signOut: async function () {
            setTimeout(function () {
              _authCallbacks.forEach(function (cb) {
                try { cb("SIGNED_OUT", null); } catch (e) {}
              });
            }, 80);
            return { error: null };
          },
          getUser: async function () {
            return { data: { user: null }, error: null };
          },
          setSession: async function () {
            return { data: { session: null }, error: null };
          },
        },
      };
    },
  };

  // Set guest mode and mark auth as seen so auth-manager skips the sign-in overlay
  localStorage.setItem("presto-guest-mode", "true");
  localStorage.setItem("presto-auth-seen", "true");

  // --- In-memory Tauri command state ---
  var _state = {
    tags: null, // lazy init; see _getTags()
    manualSessions: [],
    settings: {},
    sessionData: {
      completedPomodoros: 0,
      currentSession: 1,
      totalFocusTime: 0,
      currentMode: "focus",
    },
    history: [],
    autostartEnabled: false,
  };

  function _getTags() {
    if (_state.tags === null) {
      // Read lazily so override scripts that set window.__E2E_CONFIG__.initialTags
      // (which run after this script) are respected
      var cfg = window.__E2E_CONFIG__ || {};
      _state.tags = cfg.initialTags
        ? cfg.initialTags.slice()
        : [
            {
              id: "default-focus",
              name: "Focus",
              icon: "ri-brain-line",
              color: "#4CAF50",
              created_at: new Date().toISOString(),
            },
          ];
    }
    return _state.tags;
  }

  // --- Tauri event bus ---
  var _listeners = {};

  // --- window.__TAURI__ mock ---
  window.__TAURI__ = {
    core: {
      invoke: async function (cmd, args) {
        switch (cmd) {
          case "load_tags":
            return _getTags().slice();

          case "save_tag": {
            var tag = args && args.tag;
            if (!tag) { return; }
            var tags = _getTags();
            var idx = tags.findIndex(function (t) { return t.id === tag.id; });
            if (idx >= 0) { tags[idx] = tag; } else { tags.push(tag); }
            return;
          }

          case "delete_tag": {
            var tagId = args && args.tag_id;
            _state.tags = _getTags().filter(function (t) { return t.id !== tagId; });
            return;
          }

          case "add_session_tag":
            return;

          case "load_tasks":
            return [];

          case "save_tasks":
            return;

          case "load_settings":
            return Object.assign({}, _state.settings);

          case "save_settings": {
            if (args && args.settings) {
              _state.settings = Object.assign({}, args.settings);
            }
            return;
          }

          case "register_global_shortcuts":
            return;

          case "load_manual_sessions":
            return _state.manualSessions.slice();

          case "save_manual_sessions": {
            if (args && args.sessions) {
              _state.manualSessions = args.sessions.slice();
            } else if (Array.isArray(args)) {
              _state.manualSessions = args.slice();
            }
            return;
          }

          case "delete_manual_session": {
            var sessId = args && args.id;
            _state.manualSessions = _state.manualSessions.filter(function (s) {
              return s.id !== sessId;
            });
            return;
          }

          case "is_autostart_enabled":
            return _state.autostartEnabled;

          case "enable_autostart":
            _state.autostartEnabled = true;
            return;

          case "disable_autostart":
            _state.autostartEnabled = false;
            return;

          case "load_session_data":
            return Object.assign({}, _state.sessionData);

          case "save_session_data": {
            if (args) { _state.sessionData = Object.assign({}, args); }
            return;
          }

          case "save_daily_stats":
            // Rust handler signature: (session: PomodoroSession, app) -> Result<(), String>
            return;

          case "get_stats_history":
            // Rust handler signature: (app) -> Result<Vec<PomodoroSession>, String>
            return _state.history.slice();

          case "reset_all_data": {
            // Rust handler signature: (app) -> Result<(), String>
            _state.tags = [
              {
                id: "default-focus",
                name: "Focus",
                icon: "ri-brain-line",
                color: "#4CAF50",
                created_at: new Date().toISOString(),
              },
            ];
            _state.manualSessions = [];
            _state.settings = {};
            _state.history = [];
            return;
          }

          case "start_activity_monitoring":
            // Rust handler signature: (app, timeout_seconds: u64) -> Result<(), String>
            return;

          case "stop_activity_monitoring":
            // Rust handler signature: () -> Result<(), String>
            return;

          case "update_activity_timeout":
            // Rust handler signature: (timeout_seconds: u64) -> Result<(), String>
            return;

          case "update_tray_icon":
            // Rust handler signature: (app, timer_text, is_running, session_mode,
            //                         current_session, total_sessions, mode_icon)
            //                         -> Result<(), String>
            return;

          case "update_tray_menu":
            // Rust handler signature: (app, is_running, is_paused, current_mode)
            //                         -> Result<(), String>
            return;

          case "write_excel_file":
            return null;

          case "plugin:updater|check": {
            // Read config lazily so override scripts that run after this init script
            // can configure the updater response.
            // Call sequence when ucfg.updaterSecondCallUpdate is set:
            //   call #1 (startup check) → returns null
            //   call #2 (button click)  → returns null (ucfg.updaterCallCount === 2, not > 2)
            //   call #3+               → returns Object.assign({ available: true }, ucfg.updaterSecondCallUpdate)
            var ucfg = window.__E2E_CONFIG__ || {};
            if (ucfg.updaterCallCount === undefined) { ucfg.updaterCallCount = 0; }
            ucfg.updaterCallCount++;
            if (ucfg.updaterSecondCallUpdate && ucfg.updaterCallCount > 2) {
              return Object.assign({ available: true }, ucfg.updaterSecondCallUpdate);
            }
            return null;
          }

          case "plugin:app|version":
            return "0.4.4";

          case "plugin:dialog|message":
            return;

          case "plugin:dialog|ask":
            return false;

          case "plugin:shell|open":
            return;

          case "plugin:process|restart":
            return;

          case "start_oauth_server":
            return 9999;

          case "plugin:oauth|cancel":
            return;

          case "plugin:opener|open_url":
            return;

          default:
            console.warn("[tauriMock] Unmocked Tauri command:", cmd, args);
            return Promise.reject(new Error("Unmocked command: " + cmd));
        }
      },
    },

    dialog: {
      save: async function () { return null; },
      open: async function () { return null; },
      message: async function () {},
      ask: async function () {
        var cfg = window.__E2E_CONFIG__ || {};
        return cfg.dialogAskResult !== undefined ? cfg.dialogAskResult : false;
      },
    },

    event: {
      listen: async function (event, handler) {
        if (!_listeners[event]) { _listeners[event] = []; }
        _listeners[event].push(handler);
        return function () {
          if (_listeners[event]) {
            _listeners[event] = _listeners[event].filter(function (h) {
              return h !== handler;
            });
          }
        };
      },
      emit: async function (event, payload) {
        if (_listeners[event]) {
          _listeners[event].forEach(function (h) {
            try { h({ event: event, payload: payload }); } catch (e) {}
          });
        }
      },
    },

    notification: {
      isPermissionGranted: async function () {
        var perm = (window.__E2E_CONFIG__ || {}).notificationPermission;
        return perm === "granted";
      },
      requestPermission: async function () {
        return (window.__E2E_CONFIG__ || {}).notificationPermission || "denied";
      },
      sendNotification: function (opts) {
        if (!window.__E2E_CONFIG__) { window.__E2E_CONFIG__ = {}; }
        window.__E2E_CONFIG__.lastNotification = opts;
      },
    },
  };

  // Expose harness for fixture-level introspection
  window.__E2E_TEST_HARNESS__ = {
    state: _state,
    config: window.__E2E_CONFIG__,
  };
})();
`;

/**
 * Installs the Tauri bridge mock and Supabase stub via addInitScript before navigation.
 * Returns a harness object whose methods may add further override scripts before goto.
 *
 * @param {import('@playwright/test').Page} page
 * @returns {Promise<object>} harness with pre-navigation configuration helpers
 */
export async function applyTauriMock(page) {
  await page.addInitScript({ content: TAURI_MOCK_INIT_SCRIPT });

  return {
    /**
     * Seeds `localStorage.presto_force_update_test` so UpdateManagerV2 calls simulateUpdate()
     * on its startup check, causing an update-available event ~5 s after page boot.
     */
    async setUpdateAvailable() {
      await page.addInitScript({
        content: `
if (!window.__E2E_CONFIG__) window.__E2E_CONFIG__ = {};
window.__E2E_CONFIG__.enableUpdateTestMode = true;
localStorage.setItem('presto_force_update_test', 'true');
`,
      });
    },

    /**
     * Configures the updater mock for the settings-updates spec.
     * Call sequence: call #1 → null, call #2 → null, call #3+ → provided update.
     * (The update is returned on the 3rd+ invocation because the handler checks
     * ucfg.updaterCallCount > 2, i.e. strictly greater than 2.)
     *
     * Also enables `presto_force_update_test` so `UpdateManagerV2.checkForUpdates()`
     * routes through `simulateUpdate()` rather than `checkVersionFromGitHub()`. The
     * latter requires a non-blocked network fetch to api.github.com, which the
     * `_blockExternal` fixture forbids. `simulateUpdate()` reads the same
     * `__E2E_CONFIG__.updaterCallCount`/`updaterSecondCallUpdate` keys consumed by
     * the `plugin:updater|check` mock handler to produce a deterministic sequence.
     * @param {{ version: string }} secondCallUpdate
     */
    async configureUpdaterCalls(secondCallUpdate) {
      await page.addInitScript({
        content: `
if (!window.__E2E_CONFIG__) window.__E2E_CONFIG__ = {};
window.__E2E_CONFIG__.updaterCallCount = 0;
window.__E2E_CONFIG__.updaterSecondCallUpdate = ${JSON.stringify(secondCallUpdate)};
localStorage.setItem('presto_force_update_test', 'true');
`,
      });
    },

    /**
     * Pre-configures the notification permission state for the notifications spec.
     * @param {'granted'|'denied'|'default'} permission
     */
    async setNotificationPermission(permission) {
      await page.addInitScript({
        content: `
if (!window.__E2E_CONFIG__) window.__E2E_CONFIG__ = {};
window.__E2E_CONFIG__.notificationPermission = ${JSON.stringify(permission)};
`,
      });
    },

    /**
     * Seeds the initial tag list returned by load_tags.
     * @param {Array<{id: string, name: string, icon: string, color: string, created_at: string}>} tags
     */
    async seedTags(tags) {
      await page.addInitScript({
        content: `
if (!window.__E2E_CONFIG__) window.__E2E_CONFIG__ = {};
window.__E2E_CONFIG__.initialTags = ${JSON.stringify(tags)};
`,
      });
    },

    /**
     * Enables the #team-nav button before navigation (it is disabled in HTML today).
     * This is a documented exception to rule 1.2: we are testing the team feature's
     * behavior, not its current product gating.
     */
    async enableTeamButton() {
      await page.addInitScript({
        content: `
document.addEventListener('DOMContentLoaded', function () {
  var el = document.getElementById('team-nav');
  if (el) {
    el.disabled = false;
    el.style.opacity = '';
    el.style.cursor = '';
    el.title = 'Team';
  }
}, { once: true });
`,
      });
    },

    /**
     * Freezes wall-clock time to a fixed ISO instant before navigation.
     * Overrides Date constructor and Date.now() so calendar headers and
     * team-manager demo timers render deterministically across runs.
     * Opt-in only — existing specs continue to use real time.
     * @param {string} isoString  e.g. '2026-05-09T12:00:00Z'
     */
    // TODO(stack-swap): reaches into global Date; re-evaluate if the new stack
    // uses a different clock abstraction.
    async freezeTime(isoString) {
      await page.addInitScript({
        content: `
(function () {
  var _frozen = ${JSON.stringify(new Date(isoString).getTime())};
  var _OrigDate = Date;
  function FrozenDate() {
    if (arguments.length === 0) {
      return new _OrigDate(_frozen);
    }
    return new _OrigDate(...arguments);
  }
  FrozenDate.now = function () { return _frozen; };
  FrozenDate.parse = _OrigDate.parse.bind(_OrigDate);
  FrozenDate.UTC = _OrigDate.UTC.bind(_OrigDate);
  FrozenDate.prototype = _OrigDate.prototype;
  globalThis.Date = FrozenDate;
})();
`,
      });
    },
  };
}
