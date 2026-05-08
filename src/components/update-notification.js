/**
 * Update Notification Component
 *
 * Component for showing update notifications in the user interface
 */

// Usa l'updateManager globale invece dell'import per essere sincronizzato con main.js
const getUpdateManager = () => window.updateManager || window.updateManagerInstance;

export class UpdateNotification {
  constructor() {
    this.container = null;
    this.isVisible = false;
    this.animationDuration = 300;
    this.currentVersion = null;
    this._hideTimeoutId = null;
    this._errorHideTimeoutId = null;

    this.createNotificationContainer();
    // Aspetta che l'updateManager sia disponibile prima di bind degli eventi
    this.waitForUpdateManager();
  }

  /**
   * Aspetta che l'updateManager sia disponibile e poi bind gli eventi
   */
  async waitForUpdateManager() {
    // Aspetta che l'updateManager sia disponibile (max 10 secondi)
    let attempts = 0;
    const maxAttempts = 100; // 10 secondi con 100ms di intervallo

    while (attempts < maxAttempts && !getUpdateManager()) {
      await new Promise((resolve) => {
        setTimeout(resolve, 100);
      });
      attempts++;
    }

    if (getUpdateManager()) {
      console.log("✅ [UpdateNotification] UpdateManager found, binding notification events");
      this.bindEvents();

      // RIMOSSO: Il controllo dello stato iniziale può causare problemi
      // L'updateManager dovrebbe emettere gli eventi corretti al momento giusto
    } else {
      console.warn("⚠️ [UpdateNotification] UpdateManager not found after 10 seconds");
      // Non bloccare l'app, continua senza update notifications
    }
  }

  /**
   * Creates the container for update notifications
   */
  createNotificationContainer() {
    this.container = document.createElement("div");
    this.container.className = "update-notification-container";

    // Add desktop class if running in Tauri desktop app
    if (window.__TAURI__ && window.__TAURI__.core) {
      this.container.classList.add("desktop");
    }
    this.container.innerHTML = `
            <div class="update-notification">
                <div class="update-content">
                    <div class="update-icon">
                        <i class="ri-lightbulb-flash-line"></i>
                    </div>
                    <span class="update-message">Update available</span>
                    <span class="update-version"></span>
                    <div class="update-actions">
                        <button class="update-btn update-btn-primary" data-action="download">
                            Update via Homebrew
                        </button>
                        <button class="update-btn update-btn-secondary" data-action="dismiss">
                            Skip release
                        </button>
                    </div>
                </div>
                <div class="update-progress-container" style="display: none;">
                    <div class="update-progress-icon">
                        <div class="spinner"></div>
                    </div>
                    <span class="update-progress-message">Installing update...</span>
                    <div class="update-progress-bar">
                        <div class="update-progress-fill"></div>
                        <span class="update-progress-text">0%</span>
                    </div>
                </div>
                <button class="update-close" data-action="close">
                    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                        <path d="M12 4L4 12M4 4L12 12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                    </svg>
                </button>
            </div>
        `;

    // Inject styles
    this.injectStyles();

    // Add to DOM but hidden
    // this.container.style.display = 'none';
    document.body.appendChild(this.container);

    // Bind button events
    this.bindButtonEvents();
  }

  /**
   * Injects CSS styles for the notification
   */
  injectStyles() {
    if (document.getElementById("update-notification-styles")) {
      return;
    }

    const styles = document.createElement("style");
    styles.id = "update-notification-styles";
    styles.textContent = `
            .update-notification-container {
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            z-index: 10000;
            transform: translateY(-100%);
            transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
            }
            
            .update-notification-container.desktop {
            left: 80px;
            }

            .update-notification-container.visible {
            transform: translateY(0);
            }

            .update-notification {
            background: var(--accent-color, #007AFF);
            color: white;
            padding: 8px 16px;
            display: flex;
            align-items: center;
            gap: 12px;
            font-size: 14px;
            line-height: 1.4;
            box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
            }

            .update-content {
            display: flex;
            align-items: center;
            gap: 12px;
            flex: 1;
            min-width: 0;
            }

            .update-icon {
            color: white;
            flex-shrink: 0;
            display: flex;
            align-items: center;
            }

            .update-message {
            font-weight: 500;
            white-space: nowrap;
            }

            .update-version {
            font-size: 13px;
            opacity: 0.9;
            font-family: 'SF Mono', 'Monaco', 'Cascadia Code', monospace;
            margin-left: 8px;
            }

            .update-actions {
            display: flex;
            gap: 8px;
            margin-left: auto;
            flex-shrink: 0;
            }

            .update-btn {
            padding: 4px 12px;
            border-radius: 4px;
            font-size: 13px;
            font-weight: 500;
            border: none;
            cursor: pointer;
            transition: all 0.2s ease;
            white-space: nowrap;
            }

            .update-btn-primary {
            background: rgba(255, 255, 255, 0.2);
            color: white;
            border: 1px solid rgba(255, 255, 255, 0.3);
            }

            .update-btn-primary:hover {
            background: rgba(255, 255, 255, 0.3);
            border-color: rgba(255, 255, 255, 0.4);
            }

            .update-btn-secondary {
            background: transparent;
            color: white;
            border: 1px solid rgba(255, 255, 255, 0.3);
            }

            .update-btn-secondary:hover {
            background: rgba(255, 255, 255, 0.1);
            border-color: rgba(255, 255, 255, 0.4);
            }

            .update-close {
            background: transparent;
            border: none;
            padding: 4px;
            cursor: pointer;
            color: white;
            opacity: 0.8;
            transition: opacity 0.2s ease;
            border-radius: 3px;
            margin-left: 8px;
            flex-shrink: 0;
            }

            .update-close:hover {
            opacity: 1;
            background: rgba(255, 255, 255, 0.1);
            }

            .update-progress-container {
            display: flex;
            align-items: center;
            gap: 12px;
            flex: 1;
            min-width: 0;
            }

            .update-progress-icon {
            flex-shrink: 0;
            display: flex;
            align-items: center;
            }

            .spinner {
            width: 16px;
            height: 16px;
            border: 2px solid rgba(255, 255, 255, 0.3);
            border-top: 2px solid white;
            border-radius: 50%;
            animation: spin 1s linear infinite;
            }

            @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
            }

            .update-progress-message {
            font-weight: 500;
            white-space: nowrap;
            }

            .update-progress-bar {
            background: rgba(255, 255, 255, 0.2);
            border-radius: 3px;
            height: 4px;
            position: relative;
            overflow: hidden;
            flex: 1;
            min-width: 100px;
            margin: 0 12px;
            }

            .update-progress-fill {
            background: white;
            height: 100%;
            width: 0%;
            transition: width 0.3s ease;
            border-radius: 3px;
            }

            .update-progress-text {
            font-size: 12px;
            font-weight: 600;
            color: white;
            opacity: 0.9;
            white-space: nowrap;
            flex-shrink: 0;
            }

            /* Responsive */
            @media (max-width: 768px) {
            .update-notification-container {
                left: 0 !important;
                top: 30px;
            }

            .update-notification {
                padding: 4px 8px;
                font-size: 12px;
                gap: 6px;
            }

            .update-content {
                gap: 6px;
            }

            .update-message {
                font-size: 12px;
            }

            .update-actions {
                gap: 4px;
            }

            .update-btn {
                padding: 2px 6px;
                font-size: 11px;
                border-radius: 3px;
            }

            .update-version {
                display: none;
            }

            .update-progress-bar {
                min-width: 60px;
                margin: 0 6px;
            }

            .update-close {
                padding: 2px;
                margin-left: 4px;
            }
            }

            @media (max-width: 480px) {
            .update-actions {
                gap: 6px;
            }

            .update-btn {
                padding: 3px 6px;
            }

            .update-close {
                margin-left: 4px;
            }
            }
        `;

    document.head.appendChild(styles);
  }

  /**
   * Binds events to buttons
   */
  bindButtonEvents() {
    const buttons = this.container.querySelectorAll("[data-action]");
    console.log("🔔 [UpdateNotification] Found", buttons.length, "buttons with data-action");
    buttons.forEach((button) => {
      console.log("🔔 [UpdateNotification] Binding evento per pulsante:", button.dataset.action);
      button.addEventListener("click", (e) => {
        // Trova il pulsante con data-action, anche se si clicca su un elemento figlio (come l'icona SVG)
        let target = e.target;
        while (target && !target.dataset.action) {
          target = target.parentElement;
        }

        const action = target ? target.dataset.action : null;
        console.log("🔔 [UpdateNotification] Target found:", target, "Action:", action);
        if (action) {
          this.handleAction(action);
        } else {
          console.warn("🔔 [UpdateNotification] No action found for this click");
        }
      });
    });
  }

  /**
   * Handles button actions
   */
  handleAction(action) {
    console.log("🔔 [UpdateNotification] Azione pulsante:", action);
    switch (action) {
      case "download":
        this.startDownload();
        break;
      case "dismiss":
        this.skipVersion();
        break;
      case "close":
        this.hide();
        break;
    }
  }

  /**
   * Saves skipped version to localStorage
   */
  skipVersion() {
    if (this.currentVersion) {
      try {
        const skippedVersions = this.getSkippedVersions();
        if (!skippedVersions.includes(this.currentVersion)) {
          skippedVersions.push(this.currentVersion);
          localStorage.setItem("presto-skipped-versions", JSON.stringify(skippedVersions));
          console.log(`Skipped version ${this.currentVersion}`);
        }
      } catch (err) {
        console.error("Could not save skipped version:", err);
      }
    }
    this.hide();
  }

  /**
   * Gets list of skipped versions from localStorage
   */
  getSkippedVersions() {
    try {
      const stored = localStorage.getItem("presto-skipped-versions");
      return stored ? JSON.parse(stored) : [];
    } catch (err) {
      console.error("Could not load skipped versions:", err);
      return [];
    }
  }

  /**
   * Checks if a version has been skipped
   */
  isVersionSkipped(version) {
    const skippedVersions = this.getSkippedVersions();
    return skippedVersions.includes(version);
  }

  /**
   * Shows brew install command to user
   */
  async startDownload() {
    // Show brew install command instead of Tauri updater
    const brewCommand = "brew install murdercode/presto/presto --cask";

    // Copy command to clipboard if available
    if (navigator.clipboard && navigator.clipboard.writeText) {
      try {
        await navigator.clipboard.writeText(brewCommand);
        console.log("Brew command copied to clipboard");
      } catch (err) {
        console.log("Could not copy to clipboard:", err);
      }
    }

    // Show alert with the command
    const message = `To update Presto, run this command in your terminal:\n\n${brewCommand}\n\n${navigator.clipboard ? "The command has been copied to your clipboard." : "Please copy this command manually."}`;

    if (window.__TAURI__ && window.__TAURI__.dialog) {
      // Use Tauri dialog if available
      await window.__TAURI__.dialog.message(message, {
        title: "Update Presto via Homebrew",
        type: "info",
      });
    } else {
      // Fallback to browser alert
      alert(message);
    }

    // Hide the notification after showing the command
    this.hide();
  }

  /**
   * Shows the progress container, hiding the default content area
   */
  showProgressContainer() {
    const content = this.container.querySelector(".update-content");
    const progressContainer = this.container.querySelector(".update-progress-container");
    if (content) {
      content.style.display = "none";
    }
    if (progressContainer) {
      progressContainer.style.display = "flex";
    }
    if (!this.isVisible) {
      this.show();
    }
  }

  /**
   * Updates download progress
   */
  updateProgress(progress) {
    const progressFill = this.container.querySelector(".update-progress-fill");
    const progressText = this.container.querySelector(".update-progress-text");

    if (progressFill && progressText) {
      progressFill.style.width = `${progress}%`;
      progressText.textContent = `${progress}%`;
    }
  }

  /**
   * Binds update manager events
   */
  bindEvents() {
    const updateManager = getUpdateManager();

    if (!updateManager) {
      console.error(
        "❌ [UpdateNotification] UpdateManager not available to bind notification events"
      );
      return;
    }

    console.log("🔔 [UpdateNotification] Binding update notification events...");
    console.log("🔍 [UpdateNotification] UpdateManager state:", {
      updateAvailable: updateManager.updateAvailable,
      currentUpdate: updateManager.currentUpdate,
      isDevelopmentMode: updateManager.isDevelopmentMode
        ? updateManager.isDevelopmentMode()
        : "N/A",
      testMode: localStorage.getItem("presto_force_update_test"),
    });

    updateManager.on("updateAvailable", (event) => {
      console.log("🔔 [UpdateNotification] Evento updateAvailable ricevuto:", event.detail);
      this.showUpdateAvailable(event.detail);
    });

    // Ascolta anche quando NON ci sono aggiornamenti per nascondere la notifica
    updateManager.on("updateNotAvailable", () => {
      console.log("👍 [UpdateNotification] No updates available - nascondo notifica");
      this.hide();
    });

    // Nasconde la notifica anche quando il controllo fallisce
    updateManager.on("checkError", () => {
      console.log("❌ [UpdateNotification] Update check error - nascondo notifica");
      this.hide();
    });

    updateManager.on("downloadProgress", (event) => {
      const { progress } = event.detail;
      this.showProgressContainer();
      this.updateProgress(progress);
    });

    updateManager.on("downloadFinished", () => {
      this.showProgressContainer();
      this.showInstalling();
    });

    updateManager.on("downloadError", (event) => {
      this.showProgressContainer();
      this.showError(event.detail);
    });
  }

  /**
   * Shows update available notification
   */
  showUpdateAvailable(updateInfo) {
    console.log("🔔 [UpdateNotification] Show update notification requested:", updateInfo);

    if (!updateInfo || !updateInfo.version) {
      console.log("❌ [UpdateNotification] Invalid update info - not showing notification");
      return;
    }

    // Verifica esplicita che l'aggiornamento sia davvero disponibile
    if (updateInfo.available === false) {
      console.log(
        "❌ [UpdateNotification] Update explicitly unavailable - not showing notification"
      );
      return;
    }

    // Verifica se siamo in modalità sviluppo senza test mode
    // RIMOSSO: Ora permettiamo la notifica anche in modalità sviluppo per GitHub releases
    // const updateManager = getUpdateManager();
    // if (updateManager && updateManager.isDevelopmentMode && updateManager.isDevelopmentMode()) {
    //     const hasTestMode = localStorage.getItem('presto_force_update_test') === 'true';
    //     if (!hasTestMode) {
    //         console.log('🔍 [UpdateNotification] Development mode without test mode - not showing notification');
    //         return;
    //     }
    // }

    // Don't show if this version has been skipped
    if (this.isVersionSkipped(updateInfo.version)) {
      console.log(
        `⏭️ [UpdateNotification] Version ${updateInfo.version} was skipped - not showing notification`
      );
      return;
    }

    console.log(`✅ [UpdateNotification] Showing notification for update ${updateInfo.version}`);

    this.currentVersion = updateInfo.version;

    const versionElement = this.container.querySelector(".update-version");
    if (versionElement) {
      versionElement.textContent = `Version ${updateInfo.version}`;
    }

    this.show();
  }

  /**
   * Shows installation status
   */
  showInstalling() {
    const message = this.container.querySelector(".update-progress-message");

    if (message) {
      message.textContent = "Installing update...";
    }

    this.updateProgress(100);
  }

  /**
   * Shows an error
   */
  showError() {
    const message = this.container.querySelector(".update-progress-message");

    if (message) {
      message.textContent = "Update error";
    }

    // Hide after 5 seconds
    clearTimeout(this._errorHideTimeoutId);
    this._errorHideTimeoutId = setTimeout(() => {
      this._errorHideTimeoutId = null;
      this.hide();
    }, 5000);
  }

  /**
   * Shows the notification
   */
  show() {
    clearTimeout(this._hideTimeoutId);
    this._hideTimeoutId = null;
    clearTimeout(this._errorHideTimeoutId);
    this._errorHideTimeoutId = null;

    if (this.isVisible) {
      console.log("🔔 [UpdateNotification] Notification already visible - skip");
      return;
    }

    console.log("🔔 [UpdateNotification] Showing update notification");

    this.container.style.display = "block";

    // Force reflow before adding class
    // eslint-disable-next-line no-unused-expressions -- intentional layout trigger
    this.container.offsetHeight;

    requestAnimationFrame(() => {
      this.container.classList.add("visible");
    });

    this.isVisible = true;
  }

  /**
   * Hides the notification
   */
  hide() {
    if (!this.isVisible) {
      console.log("🔔 [UpdateNotification] Notification already hidden - skip");
      return;
    }

    console.log("🔔 [UpdateNotification] Hiding update notification");

    this.container.classList.remove("visible");

    this._hideTimeoutId = setTimeout(() => {
      this._hideTimeoutId = null;
      this.container.style.display = "none";
      this.resetToInitialState();
    }, this.animationDuration);

    this.isVisible = false;
  }

  /**
   * Resets notification to initial state
   */
  resetToInitialState() {
    const content = this.container.querySelector(".update-content");
    const progressContainer = this.container.querySelector(".update-progress-container");

    content.style.display = "flex";
    progressContainer.style.display = "none";

    this.updateProgress(0);
  }

  /**
   * Destroys the component
   */
  destroy() {
    clearTimeout(this._hideTimeoutId);
    this._hideTimeoutId = null;
    clearTimeout(this._errorHideTimeoutId);
    this._errorHideTimeoutId = null;
    if (this.container && this.container.parentNode) {
      this.container.parentNode.removeChild(this.container);
    }
    this.container = null;
    this.isVisible = false;
  }
}

// Export the class, not an instance - let main.js handle initialization
// export const updateNotification = new UpdateNotification();
