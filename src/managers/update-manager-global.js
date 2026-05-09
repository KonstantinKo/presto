import { logger } from "../utils/logger.js";
import { toError } from "../utils/to-error.js";

const GITHUB_REPO = "murdercode/presto";

// Expose UpdateManagerV2 as a global for backwards compatibility
window.UpdateManagerV2 = class UpdateManagerV2 {
  constructor() {
    this.updateAvailable = false;
    this.currentUpdate = null;
    this.isChecking = false;
    this.isDownloading = false;
    this.downloadProgress = 0;
    this.autoCheck = true;
    this.checkInterval = null;
    this.startupTimeout = null;

    /** @type {EventTarget | null} */
    this.eventTarget = new EventTarget();

    this.loadPreferences();
    if (this.autoCheck && !this.checkInterval) {
      this.startAutoCheck();
    }

    logger.debug("UpdateManager v2 initialized (global)");
  }

  isDevelopmentMode() {
    if (localStorage.getItem("presto_force_update_test") === "true") {
      logger.debug("🧪 Update test mode active");
      return false;
    }

    if (!window.__TAURI__) {
      logger.debug("🔍 Not a Tauri environment - development mode");
      return true;
    }

    if (window.location.protocol === "tauri:") {
      logger.debug("🔍 Tauri protocol: - compiled app");
      return false;
    }

    if (
      window.location.hostname === "localhost" ||
      window.location.href.includes("localhost") ||
      window.location.href.includes("127.0.0.1")
    ) {
      logger.debug("🔍 Localhost detected - development mode");
      return true;
    }

    logger.debug("🔍 Production environment detected");
    return false;
  }

  async getTauriUpdaterAPI() {
    if (!window.__TAURI__) {
      throw new Error("Tauri environment not available");
    }

    if (window.__TAURI__.updater) {
      logger.debug("✅ Using global updater API");
      return window.__TAURI__.updater;
    }

    if (window.__TAURI__.core) {
      logger.debug("✅ Using updater API via invoke");
      const tauriCore = window.__TAURI__.core;
      return {
        check: async () => {
          const result = await tauriCore.invoke("plugin:updater|check");
          return result ? { .../** @type {any} */ (result), manualDownloadRequired: true } : result;
        },
      };
    }

    logger.debug("Updater API not available, using manual approach");
    return null;
  }

  async getAppVersion() {
    try {
      if (window.__TAURI__?.app?.getVersion) {
        return await window.__TAURI__.app.getVersion();
      }

      if (window.__TAURI__?.core?.invoke) {
        return await window.__TAURI__.core.invoke("plugin:app|version");
      }

      throw new Error("Version API not available");
    } catch (error) {
      logger.error("❌ Could not retrieve app version:", error);
      throw new Error("Unable to determine current application version");
    }
  }

  async restartApp() {
    try {
      if (window.__TAURI__?.process?.relaunch) {
        await window.__TAURI__.process.relaunch();
        return;
      }

      if (window.__TAURI__?.core?.invoke) {
        await window.__TAURI__.core.invoke("plugin:process|restart");
        return;
      }

      throw new Error("Restart API not available");
    } catch (error) {
      logger.error("❌ Restart error:", error);
      await this.showMessage(
        "The update was installed but automatic restart is not available.\n\nPlease restart the application manually.",
        { title: "Manual Restart", kind: "warning" }
      );
    }
  }

  enableTestMode() {
    localStorage.setItem("presto_force_update_test", "true");
    logger.warn("⚠️ UPDATE TEST MODE ACTIVATED");

    if (!this.isDevelopmentMode() && this.autoCheck && !this.checkInterval) {
      this.startAutoCheck();
    }

    return "Test mode activated! Use checkForUpdates() to test.";
  }

  disableTestMode() {
    localStorage.removeItem("presto_force_update_test");
    logger.debug("Update test mode disabled");

    if (this.isDevelopmentMode()) {
      this.stopAutoCheck();
    }

    return "Test mode disabled!";
  }

  /** @param {string} content @param {any} [options] */
  async showMessage(content, options = {}) {
    const defaultOptions = {
      title: "Presto",
      kind: "info",
    };
    const opts = { ...defaultOptions, ...options };

    try {
      if (window.__TAURI__?.dialog?.message) {
        await window.__TAURI__.dialog.message(content, opts);
        return;
      }

      if (window.__TAURI__?.core?.invoke) {
        await window.__TAURI__.core.invoke("plugin:dialog|message", {
          message: content,
          title: opts.title,
          kind: opts.kind,
        });
        return;
      }

      alert(`${opts.title}\n\n${content}`);
    } catch (error) {
      logger.error("Error showing message:", error);
      alert(`${opts.title}\n\n${content}`);
    }
  }

  /** @param {string} content @param {any} [options] */
  async askConfirmation(content, options = {}) {
    const defaultOptions = {
      title: "Confirm",
      okLabel: "Yes",
      cancelLabel: "No",
    };
    const opts = { ...defaultOptions, ...options };

    try {
      if (window.__TAURI__?.dialog?.ask) {
        return await window.__TAURI__.dialog.ask(content, opts);
      }

      if (window.__TAURI__?.core?.invoke) {
        return await window.__TAURI__.core.invoke("plugin:dialog|ask", {
          ...opts,
          message: content,
        });
      }

      return confirm(content);
    } catch (error) {
      logger.error("Error asking confirmation:", error);
      return confirm(content);
    }
  }

  async showDevelopmentMessage() {
    await this.showMessage(
      "Update check not available in development mode.\n\nUpdates will only work in the compiled application.",
      {
        title: "Development Mode",
        kind: "info",
      }
    );
  }

  startAutoCheck() {
    if (this.autoCheck && !this.checkInterval) {
      this.checkInterval = setInterval(
        () => {
          logger.debug("Automatic periodic update check...");
          this.checkForUpdates(false);
        },
        60 * 60 * 1000
      );

      this.startupTimeout = setTimeout(() => {
        this.startupTimeout = null;
        logger.debug("Initial automatic update check...");
        this.checkForUpdates(false);
      }, 5000);

      logger.debug("Automatic update check started");
    }
  }

  stopAutoCheck() {
    clearTimeout(this.startupTimeout);
    this.startupTimeout = null;
    if (this.checkInterval) {
      clearInterval(this.checkInterval);
      this.checkInterval = null;
      logger.debug("Automatic check stopped");
    }
  }

  /** @param {string} a @param {string} b @returns {number} */
  compareVersions(a, b) {
    const cleanA = a.replace(/^v/, "");
    const cleanB = b.replace(/^v/, "");

    const aParts = cleanA.split(".").map((/** @type {string} */ n) => parseInt(n, 10) || 0);
    const bParts = cleanB.split(".").map((/** @type {string} */ n) => parseInt(n, 10) || 0);

    for (let i = 0; i < Math.max(aParts.length, bParts.length); i++) {
      const aPart = aParts[i] || 0;
      const bPart = bParts[i] || 0;

      if (aPart > bPart) {
        return 1;
      }
      if (aPart < bPart) {
        return -1;
      }
    }

    return 0;
  }

  /** @param {string} url @param {number} [timeoutMs] */
  async fetchWithTimeout(url, timeoutMs = 10000) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    try {
      return await fetch(url, { signal: controller.signal });
    } finally {
      clearTimeout(timer);
    }
  }

  /** @param {boolean} [showDialog] */
  async checkVersionFromGitHub(showDialog = true) {
    try {
      let currentVersion;
      try {
        currentVersion = await this.getAppVersion();
        logger.debug(`Current version: ${currentVersion}`);
      } catch (error) {
        logger.error("❌ Error retrieving current version:", error);
        this.updateAvailable = false;
        this.currentUpdate = null;
        this.emit("checkError", { message: "Unable to determine current version" });
        if (showDialog) {
          await this.showMessage(
            "Unable to check for updates: current version could not be determined",
            { title: "Error", kind: "error" }
          );
        }
        return false;
      }

      const response = await this.fetchWithTimeout(
        `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`
      );
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const githubRelease = await response.json();
      const latestVersion = githubRelease.tag_name.replace(/^v/, "");

      logger.debug(`Latest GitHub version: ${latestVersion}`);

      if (this.compareVersions(latestVersion, currentVersion) <= 0) {
        logger.debug("No updates available");
        this.updateAvailable = false;
        this.currentUpdate = null;
        this.emit("updateNotAvailable");

        if (showDialog) {
          await this.showMessage(
            `No updates available.\n\nCurrent version: ${currentVersion}\nLatest version: ${latestVersion}`,
            { title: "No Updates", kind: "info" }
          );
        }
        return false;
      }

      logger.info(`🎉 Update available: ${latestVersion}`);
      this.updateAvailable = true;
      this.currentUpdate = {
        version: latestVersion,
        body: githubRelease.body || "",
        date: githubRelease.published_at,
      };

      this.emit("updateAvailable", this.currentUpdate);

      if (showDialog) {
        const message =
          `🎉 Update available!\n\n` +
          `Current version: ${currentVersion}\n` +
          `New version: ${latestVersion}\n\n` +
          `Note: In development mode, download manually from GitHub.`;
        await this.showMessage(message, { title: "Update Available", kind: "info" });
      }

      return true;
    } catch (error) {
      logger.error("❌ Error checking GitHub version:", error);
      this.updateAvailable = false;
      this.currentUpdate = null;
      this.emit("checkError", { message: `Network error: ${toError(error).message}` });
      if (showDialog) {
        await this.showMessage(`Error checking for updates:\n${toError(error).message}`, {
          title: "Error",
          kind: "error",
        });
      }
      return false;
    } finally {
      this.isChecking = false;
    }
  }

  /** @param {boolean} [showDialog] */
  async checkForUpdates(showDialog = true) {
    if (this.isChecking) {
      logger.debug("⏳ Check already in progress");
      return false;
    }

    this.isChecking = true;
    this.emit("checkStarted");

    try {
      logger.debug("Checking for updates...");

      const isDevMode = this.isDevelopmentMode();
      const hasTestMode = localStorage.getItem("presto_force_update_test") === "true";

      if (isDevMode && !hasTestMode) {
        logger.debug("Development mode - checking via GitHub API without installation");
        return await this.checkVersionFromGitHub(showDialog);
      }

      if (hasTestMode) {
        logger.debug("Test mode - simulating update");
        return await this.simulateUpdate();
      }

      let currentVersion;
      try {
        currentVersion = await this.getAppVersion();
        logger.debug(`Current version: ${currentVersion}`);
      } catch (versionError) {
        logger.error("❌ Could not retrieve current version:", toError(versionError).message);
        this.updateAvailable = false;
        this.currentUpdate = null;
        this.emit("checkError", { message: "Unable to determine current application version" });
        if (showDialog) {
          await this.showMessage("Unable to determine current application version", {
            title: "Error",
            kind: "error",
          });
        }
        return false;
      }

      const response = await this.fetchWithTimeout(
        `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`
      );
      if (!response.ok) {
        throw new Error(`GitHub API error: ${response.status}`);
      }

      const githubRelease = await response.json();
      const latestVersion = githubRelease.tag_name.replace(/^v/, "");

      logger.debug(`Latest GitHub version: ${latestVersion}`);

      if (this.compareVersions(latestVersion, currentVersion) <= 0) {
        logger.debug("No updates available");
        this.updateAvailable = false;
        this.currentUpdate = null;
        this.emit("updateNotAvailable");
        return false;
      }

      logger.info("🎉 Update available!");

      try {
        const tauriAPI = await this.getTauriUpdaterAPI();
        if (tauriAPI) {
          logger.debug("Using Tauri updater API...");
          const tauriUpdate = await tauriAPI.check();

          if (tauriUpdate && tauriUpdate.available) {
            logger.debug("Update confirmed via Tauri API");
            const canAutoInstall =
              typeof tauriAPI.downloadAndInstall === "function" &&
              !tauriUpdate.manualDownloadRequired;
            this.updateAvailable = true;
            this.currentUpdate = {
              ...tauriUpdate,
              downloadUrl: githubRelease.html_url,
              isAutoDownloadable: canAutoInstall,
              source: canAutoInstall ? "tauri-api" : "github-manual",
            };
            this.emit("updateAvailable", this.currentUpdate);
            return true;
          }
        }
      } catch (error) {
        logger.warn("⚠️ Tauri updater API not available:", toError(error).message);
      }

      logger.debug("Using GitHub info with manual download");
      const manualUpdate = {
        version: latestVersion,
        date: githubRelease.published_at,
        body: githubRelease.body || "No description available",
        downloadUrl: githubRelease.html_url,
        isAutoDownloadable: false,
        source: "github-manual",
      };

      this.updateAvailable = true;
      this.currentUpdate = manualUpdate;
      this.emit("updateAvailable", manualUpdate);
      return true;
    } catch (error) {
      logger.error("❌ Update check error:", error);
      this.updateAvailable = false;
      this.currentUpdate = null;
      this.emit("checkError", { message: toError(error).message || String(error) });

      if (showDialog) {
        await this.showMessage("Error checking for updates.\n\nPlease try again later.", {
          title: "Error",
          kind: "error",
        });
      }
      return false;
    } finally {
      this.isChecking = false;
      this.emit("checkFinished");
    }
  }

  async simulateUpdate() {
    logger.info("🧪 Simulating update for test...");

    const currentVersion = await this.getAppVersion();
    const simulatedNewVersion = this.incrementVersion(currentVersion);

    const update = {
      version: simulatedNewVersion,
      date: new Date().toISOString(),
      body: `🧪 **Simulated Update for Test**\n\nVersion: ${simulatedNewVersion}\n\n**Simulated changes:**\n- Performance improvements\n- Bug fixes\n- New features\n\n*This is a test update. No real downloads will occur.*`,
      downloadUrl: `https://github.com/${GITHUB_REPO}/releases`,
      isAutoDownloadable: true,
      source: "test-simulation",
    };

    this.updateAvailable = true;
    this.currentUpdate = update;
    this.emit("updateAvailable", update);
    return true;
  }

  /** @param {string} version @returns {string} */
  incrementVersion(version) {
    const parts = version
      .replace(/^v/, "")
      .split(".")
      .map((/** @type {string} */ n) => parseInt(n, 10) || 0);
    parts[2] = (parts[2] || 0) + 1;
    return parts.join(".");
  }

  /** @param {string} url */
  async openDownloadUrl(url) {
    try {
      if (window.__TAURI__?.shell?.open) {
        await window.__TAURI__.shell.open(url);
        return;
      }

      if (window.__TAURI__?.core?.invoke) {
        await window.__TAURI__.core.invoke("plugin:shell|open", { url });
        return;
      }

      window.open(url, "_blank");
    } catch (error) {
      logger.error("Error opening URL:", error);
      window.open(url, "_blank");
    }
  }

  async downloadAndInstall() {
    if (!this.updateAvailable || !this.currentUpdate) {
      throw new Error("No updates available");
    }

    if (this.currentUpdate.source === "test-simulation") {
      logger.info("🧪 Simulating download and install...");
      return await this.simulateDownloadAndInstall();
    }

    this.isDownloading = true;
    this.downloadProgress = 0;
    this.emit("downloadStarted");

    try {
      if (this.currentUpdate.isAutoDownloadable && this.currentUpdate.source === "tauri-api") {
        logger.info("📥 Automatic download via Tauri...");

        const tauriAPI = await this.getTauriUpdaterAPI();
        if (tauriAPI && tauriAPI.downloadAndInstall) {
          let downloaded = 0;
          let contentLength = 0;
          await tauriAPI.downloadAndInstall((/** @type {any} */ progress) => {
            if (typeof progress === "number") {
              this.downloadProgress = progress;
              this.emit("downloadProgress", {
                progress,
                chunkLength: progress,
                contentLength: 100,
              });
              return;
            }

            switch (progress?.event) {
              case "Started":
                downloaded = 0;
                contentLength = progress.data?.contentLength ?? 0;
                break;
              case "Progress": {
                const chunkLength = progress.data?.chunkLength ?? 0;
                downloaded += chunkLength;
                const pct = contentLength > 0 ? Math.round((downloaded / contentLength) * 100) : 0;
                logger.debug(`📥 Download progress: ${pct}%`);
                this.downloadProgress = pct;
                if (contentLength > 0) {
                  this.emit("downloadProgress", { progress: pct, chunkLength, contentLength });
                }
                break;
              }
              case "Finished":
                this.downloadProgress = 100;
                break;
            }
          });

          this.downloadProgress = 100;
          this.emit("downloadProgress", {
            progress: 100,
            chunkLength: 100,
            contentLength: 100,
          });

          this.emit("downloadFinished");
          this.emit("installFinished");

          const shouldRestart = await this.askConfirmation(
            "Update downloaded and installed successfully!\n\nWould you like to restart the application now?",
            { title: "Update Complete" }
          );

          if (shouldRestart) {
            await this.restartApp();
          }
        }
      } else {
        logger.info("🌐 Redirecting to manual download...");
        await this.openDownloadUrl(this.currentUpdate.downloadUrl);

        this.emit("manualDownloadRequired", { url: this.currentUpdate.downloadUrl });
      }
    } catch (error) {
      logger.error("❌ Download error:", error);
      this.emit("downloadError", error);
      throw error;
    } finally {
      this.isDownloading = false;
    }
  }

  async simulateDownloadAndInstall() {
    logger.info("🧪 Simulating download...");

    for (let i = 0; i <= 100; i += 10) {
      await new Promise((resolve) => {
        setTimeout(resolve, 100);
      });
      this.downloadProgress = i;
      this.emit("downloadProgress", {
        progress: i,
        chunkLength: i,
        contentLength: 100,
      });
    }

    this.emit("downloadFinished");
    logger.info("🧪 Simulated download complete");

    await new Promise((resolve) => {
      setTimeout(resolve, 500);
    });
    this.emit("installFinished");
    logger.info("🧪 Simulated install complete");

    await this.showMessage(
      "🧪 **Test Complete**\n\nThe update was simulated successfully!\n\nIn a real environment, the application would restart now.",
      { title: "Update Test", kind: "info" }
    );
  }

  async getCurrentVersion() {
    return await this.getAppVersion();
  }

  /** @param {boolean} enabled */
  setAutoCheck(enabled) {
    this.autoCheck = enabled;

    if (enabled) {
      this.startAutoCheck();
    } else {
      this.stopAutoCheck();
    }

    try {
      localStorage.setItem("presto_auto_check_updates", enabled.toString());
    } catch (error) {
      logger.warn("Error saving auto-check preference:", error);
    }
  }

  loadPreferences() {
    try {
      const autoCheck = localStorage.getItem("presto_auto_check_updates");
      if (autoCheck !== null) {
        this.setAutoCheck(autoCheck === "true");
      }
    } catch (error) {
      logger.warn("Error loading preferences:", error);
    }
  }

  /** @param {string} event @param {any} callback */
  on(event, callback) {
    this.eventTarget?.addEventListener(event, callback);
  }

  /** @param {string} event @param {any} callback */
  off(event, callback) {
    this.eventTarget?.removeEventListener(event, callback);
  }

  /** @param {string} event @param {any} [data] */
  emit(event, data = null) {
    this.eventTarget?.dispatchEvent(new CustomEvent(event, { detail: data }));
  }

  destroy() {
    this.stopAutoCheck();
    clearTimeout(this.startupTimeout);
    this.startupTimeout = null;
    this.eventTarget = null;
  }
};

// Debug utilities — only in dev mode
const _isDevMode =
  typeof window !== "undefined" &&
  (!window.__TAURI__ ||
    window.location.hostname === "localhost" ||
    window.location.href.includes("127.0.0.1"));

if (_isDevMode) {
  window.updateManagerV2Debug = {
    enableTestMode: () => {
      localStorage.setItem("presto_force_update_test", "true");
      logger.warn("⚠️ UPDATE TEST MODE ACTIVATED");
      return "Test mode activated! Use window.updateManager.checkForUpdates() to test.";
    },
    disableTestMode: () => {
      localStorage.removeItem("presto_force_update_test");
      logger.debug("Update test mode disabled");
      return "Test mode disabled!";
    },
    testUpdate: () => {
      const mgr = window.updateManager || window.updateManagerInstance;
      if (!mgr) {
        logger.error("UpdateManager not initialized");
        return Promise.reject(new Error("UpdateManager not found"));
      }
      return mgr.simulateUpdate();
    },
    checkRealUpdate: () => {
      const mgr = window.updateManager || window.updateManagerInstance;
      if (!mgr) {
        logger.error("UpdateManager not initialized");
        return Promise.reject(new Error("UpdateManager not found"));
      }
      return mgr.checkForUpdates();
    },
    getStatus: () => {
      const mgr = window.updateManager || window.updateManagerInstance;
      if (!mgr) {
        return { error: "UpdateManager not initialized" };
      }
      return {
        updateAvailable: mgr.updateAvailable,
        currentUpdate: mgr.currentUpdate,
        isChecking: mgr.isChecking,
        isDownloading: mgr.isDownloading,
        autoCheck: mgr.autoCheck,
        isDevelopmentMode: mgr.isDevelopmentMode(),
      };
    },
    checkEnvironment: () => {
      const env = {
        hasTauri: !!window.__TAURI__,
        hasUpdater: !!window.__TAURI__?.updater,
        hasCore: !!window.__TAURI__?.core,
        hasApp: !!window.__TAURI__?.app,
        hasDialog: !!window.__TAURI__?.dialog,
        hasShell: !!window.__TAURI__?.shell,
        protocol: window.location.protocol,
        hostname: window.location.hostname,
      };
      logger.info("UpdateManager environment:", env);
      return env;
    },
  };

  logger.debug("UpdateManager V2 debug helpers available at window.updateManagerV2Debug");
}
