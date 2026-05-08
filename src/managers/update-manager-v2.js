/**
 * Update Manager per Tauri v2
 *
 * Gestisce il controllo e l'installazione degli aggiornamenti dell'applicazione
 * seguendo rigorosamente la guida ufficiale di Tauri v2.
 * https://v2.tauri.app/plugin/updater/
 */

import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";

export class UpdateManagerV2 {
  constructor() {
    this.updateAvailable = false;
    this.currentUpdate = null;
    this.isChecking = false;
    this.isDownloading = false;
    this.downloadProgress = 0;
    this.autoCheck = true;
    this.checkInterval = null;

    // Eventi personalizzati
    this.eventTarget = new EventTarget();

    // Inizializza il controllo automatico solo se non siamo in dev mode
    if (!this.isDevelopmentMode()) {
      this.startAutoCheck();
    }

    console.log("✅ UpdateManager v2 initialized for macOS");
  }

  /**
   * Verifica se siamo in modalità sviluppo
   */
  isDevelopmentMode() {
    // Permetti override per test degli aggiornamenti
    if (localStorage.getItem("presto_force_update_test") === "true") {
      console.log("🧪 Update test mode active - bypassing dev check");
      return false;
    }

    // Verifica se siamo in un ambiente Tauri
    if (!window.__TAURI__) {
      console.log("🔍 Not a Tauri environment - development mode");
      return true;
    }

    // Verifica se stiamo running da tauri dev (protocollo tauri: indica app compilata)
    if (window.location.protocol === "tauri:") {
      console.log("🔍 Tauri protocol: - compiled app");
      return false;
    }

    // Se stiamo usando localhost, siamo probabilmente in modalità dev
    if (
      window.location.hostname === "localhost" ||
      window.location.href.includes("localhost") ||
      window.location.href.includes("127.0.0.1")
    ) {
      console.log("🔍 Localhost detected - development mode");
      return true;
    }

    // Default: se arriviamo qui probabilmente siamo in un'app compilata
    console.log("🔍 Ambiente sconosciuto - assumo app compilata");
    return false;
  }

  /**
   * Attiva la modalità test per gli aggiornamenti (solo per sviluppo)
   */
  enableTestMode() {
    localStorage.setItem("presto_force_update_test", "true");
    console.warn("⚠️ UPDATE TEST MODE ACTIVATED - For development only!");
    console.log("🔄 Reload the page or restart the app to activate test mode");

    if (!this.isDevelopmentMode() && this.autoCheck && !this.checkInterval) {
      this.startAutoCheck();
    }

    return "Modalità test attivata! Usa checkForUpdates() per testare.";
  }

  /**
   * Disattiva la modalità test per gli aggiornamenti
   */
  disableTestMode() {
    localStorage.removeItem("presto_force_update_test");
    console.log("✅ Update test mode disabled");

    if (this.isDevelopmentMode()) {
      this.stopAutoCheck();
    }

    return "Test mode disabled";
  }

  /**
   * Mostra un messaggio all'utente
   */
  async showMessage(content, options = {}) {
    try {
      const { message } = await import("@tauri-apps/plugin-dialog");
      return await message(content, options);
    } catch (error) {
      console.error("Error showing message:", error);
      alert(content);
    }
  }

  /**
   * Chiede conferma all'utente
   */
  async askUser(content, options = {}) {
    try {
      const { ask } = await import("@tauri-apps/plugin-dialog");
      return await ask(content, options);
    } catch (error) {
      console.error("Error asking confirmation:", error);
      return confirm(content);
    }
  }

  /**
   * Mostra messaggio per modalità sviluppo
   */
  async showDevelopmentMessage() {
    await this.showMessage(
      "Update check not available in development mode.\n\nGli aggiornamenti funzioneranno solo nell'applicazione compilata.",
      {
        title: "Modalità Sviluppo",
        kind: "info",
      }
    );
  }

  /**
   * Avvia il controllo automatico degli aggiornamenti
   */
  startAutoCheck() {
    if (this.autoCheck && !this.checkInterval && !this.isDevelopmentMode()) {
      // Controlla ogni ora
      this.checkInterval = setInterval(
        () => {
          this.checkForUpdates(false); // silent check
        },
        60 * 60 * 1000
      );

      // Controllo iniziale dopo 30 secondi
      setTimeout(() => {
        this.checkForUpdates(false);
      }, 30000);

      console.log("🔄 Automatic update check started");
    }
  }

  /**
   * Ferma il controllo automatico degli aggiornamenti
   */
  stopAutoCheck() {
    if (this.checkInterval) {
      clearInterval(this.checkInterval);
      this.checkInterval = null;
      console.log("⏹️ Automatic update check stopped");
    }
  }

  /**
   * Controlla se sono disponibili aggiornamenti usando l'API ufficiale di Tauri v2
   * @param {boolean} showDialog - Se mostrare dialoghi all'utente
   * @returns {Promise<boolean>} - True se sono disponibili aggiornamenti
   */
  async checkForUpdates(showDialog = true) {
    if (this.isChecking) {
      console.log("⏳ Update check already in progress");
      return false;
    }

    this.isChecking = true;
    this.emit("checkStarted");

    try {
      console.log("🔄 Checking for updates with Tauri v2 API...");

      // Verifica ambiente
      const isDevMode = this.isDevelopmentMode();
      const hasTestMode = localStorage.getItem("presto_force_update_test") === "true";

      if (isDevMode && !hasTestMode) {
        console.warn("⚠️ Update check not available in development mode");
        this.emit("updateNotAvailable");
        if (showDialog) {
          await this.showDevelopmentMessage();
        }
        return false;
      }

      // Se in test mode, simula invece di usare l'API reale
      if (hasTestMode) {
        return await this.checkForUpdatesSimulated(showDialog);
      }

      // Usa l'API ufficiale di Tauri v2
      console.log("📞 Chiamata API: check()");
      const update = await check({
        // Opzioni per macOS (se necessarie)
        target: "darwin-x86_64", // o 'darwin-aarch64' per Apple Silicon
      });

      console.log("📦 Update check response:", update);

      if (update?.available) {
        console.log("✅ Update available:", update.version);
        this.updateAvailable = true;
        this.currentUpdate = update;
        this.emit("updateAvailable", update);

        if (showDialog) {
          await this.showUpdateDialog(update);
        }

        return true;
      } else {
        console.log("✅ No updates available");
        this.updateAvailable = false;
        this.currentUpdate = null;
        this.emit("updateNotAvailable");

        if (showDialog) {
          await this.showMessage("Stai usando la versione più recente!", {
            title: "Nessun aggiornamento",
            kind: "info",
          });
        }

        return false;
      }
    } catch (error) {
      console.error("❌ Error during update check:", error);
      this.emit("checkError", error);

      let errorMessage = "Errore durante il controllo degli aggiornamenti.";

      if (error?.message) {
        if (error.message.includes("network") || error.message.includes("request")) {
          errorMessage = "Errore di rete. Verifica la connessione Internet e riprova.";
        } else if (error.message.includes("permission")) {
          errorMessage = "Permessi insufficienti per controllare gli aggiornamenti.";
        } else if (error.message.includes("not available")) {
          errorMessage = "Sistema di aggiornamenti non disponibile in questa versione.";
        }
      }

      if (showDialog) {
        await this.showMessage(
          `${errorMessage}\n\nDettagli: ${error?.message || "Errore sconosciuto"}`,
          {
            title: "Errore Aggiornamenti",
            kind: "error",
          }
        );
      }

      return false;
    } finally {
      this.isChecking = false;
      this.emit("checkFinished");
    }
  }

  /**
   * Controlla aggiornamenti in modalità simulata (per test)
   */
  async checkForUpdatesSimulated(showDialog = true) {
    console.log("🧪 Simulated update check...");

    try {
      // Simula delay di rete
      await new Promise((resolve) => {
        setTimeout(resolve, 1000);
      });

      const currentVersion = await this.getCurrentVersion();
      console.log("📋 Current version:", currentVersion);

      // Simula controllo con versione fittizia più alta
      const simulatedNewVersion = this.incrementVersion(currentVersion);

      console.log(`✅ Simulation: Update available! ${currentVersion} → ${simulatedNewVersion}`);

      const update = {
        available: true,
        version: simulatedNewVersion,
        date: new Date().toISOString(),
        body: `🧪 AGGIORNAMENTO SIMULATO\n\nQuesto è un aggiornamento di test da ${currentVersion} a ${simulatedNewVersion}.\n\nIn modalità produzione, qui verrebbero mostrate le note di rilascio reali.`,
        downloadAndInstall: this.simulateDownloadAndInstall.bind(this),
      };

      this.updateAvailable = true;
      this.currentUpdate = update;
      this.emit("updateAvailable", update);

      if (showDialog) {
        await this.showUpdateDialog(update);
      }

      return true;
    } catch (error) {
      console.error("❌ Simulation error:", error);
      throw error;
    }
  }

  /**
   * Incrementa una versione per simulazione
   */
  incrementVersion(version) {
    const parts = version.split(".").map((n) => parseInt(n, 10) || 0);
    parts[2]++; // Incrementa la patch version
    return parts.join(".");
  }

  /**
   * Mostra il dialogo di conferma aggiornamento
   */
  async showUpdateDialog(update) {
    const shouldUpdate = await this.askUser(
      `È disponibile una nuova versione (${update.version}).\n\n${update.body ? `${update.body.substring(0, 200)}...` : ""}\n\nVuoi scaricare e installare l'aggiornamento ora?`,
      {
        title: "Update available",
        kind: "info",
      }
    );

    if (shouldUpdate) {
      await this.downloadAndInstall();
    }
  }

  /**
   * Scarica e installa l'aggiornamento usando l'API ufficiale di Tauri v2
   */
  async downloadAndInstall() {
    if (!this.currentUpdate || this.isDownloading) {
      return;
    }

    this.isDownloading = true;
    this.downloadProgress = 0;
    this.emit("downloadStarted");

    try {
      console.log("📥 Starting update download with Tauri v2 API...");

      // Usa l'API ufficiale downloadAndInstall con callback di progresso
      await this.currentUpdate.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            console.log("📥 Download iniziato");
            this.emit("downloadProgress", {
              progress: 0,
              contentLength: event.data.contentLength,
            });
            break;
          case "Progress":
            this.downloadProgress = Math.round(
              (event.data.chunkLength / event.data.contentLength) * 100
            );
            console.log(`📊 Progresso download: ${this.downloadProgress}%`);
            this.emit("downloadProgress", {
              progress: this.downloadProgress,
              chunkLength: event.data.chunkLength,
              contentLength: event.data.contentLength,
            });
            break;
          case "Finished":
            console.log("✅ Download complete");
            this.downloadProgress = 100;
            this.emit("downloadFinished");
            break;
        }
      });

      console.log("🔄 Update installed, restarting...");
      this.emit("installFinished");

      // Mostra messaggio di successo prima del riavvio
      await this.showMessage(
        "Aggiornamento installato con successo!\n\nL'applicazione verrà riavviata ora.",
        {
          title: "Aggiornamento completato",
          kind: "info",
        }
      );

      // Riavvia l'applicazione usando l'API ufficiale
      await relaunch();
    } catch (error) {
      console.error("❌ Error during installation:", error);
      this.emit("downloadError", error);

      await this.showMessage(
        `Errore durante l'installazione dell'aggiornamento: ${error.message}`,
        {
          title: "Errore",
          kind: "error",
        }
      );
    } finally {
      this.isDownloading = false;
    }
  }

  /**
   * Simula il download e installazione per test
   */
  async simulateDownloadAndInstall(progressCallback) {
    console.log("🧪 Simulating download and install...");

    const totalSize = 5 * 1024 * 1024; // 5MB simulati
    let downloaded = 0;

    // Simula l'evento di inizio
    if (progressCallback) {
      progressCallback({
        event: "Started",
        data: { contentLength: totalSize },
      });
    }

    // Simula il download con progresso
    const chunks = 20;
    const chunkSize = totalSize / chunks;

    for (let i = 0; i < chunks; i++) {
      await new Promise((resolve) => {
        setTimeout(resolve, 200);
      });
      downloaded += chunkSize;

      if (progressCallback) {
        progressCallback({
          event: "Progress",
          data: {
            chunkLength: downloaded,
            contentLength: totalSize,
          },
        });
      }
    }

    // Simula completamento
    if (progressCallback) {
      progressCallback({
        event: "Finished",
        data: {},
      });
    }

    console.log("🧪 Simulated download complete!");

    // Simula riavvio
    await this.showMessage(
      "🧪 MODALITÀ TEST: In un'app vera, ora verrebbe riavviata automaticamente.",
      {
        title: "Simulazione Riavvio",
        kind: "info",
      }
    );
  }

  /**
   * Ottiene la versione corrente dell'applicazione usando l'API ufficiale
   */
  async getCurrentVersion() {
    try {
      // Usa l'API ufficiale di Tauri v2
      const version = await getVersion();
      console.log("📋 Current version from Tauri API:", version);
      return version;
    } catch (error) {
      console.warn("❌ Error retrieving version with Tauri API:", error);
      // Fallback alla versione hardcoded
      return "0.2.2";
    }
  }

  /**
   * Apre la pagina delle release su GitHub
   */
  async openReleasePage() {
    try {
      const { open } = await import("@tauri-apps/plugin-opener");
      await open("https://github.com/murdercode/presto/releases");
    } catch (error) {
      console.error("Error opening release page:", error);
      window.open("https://github.com/murdercode/presto/releases", "_blank");
    }
  }

  /**
   * Ottiene lo stato corrente degli aggiornamenti
   */
  getStatus() {
    return {
      updateAvailable: this.updateAvailable,
      currentUpdate: this.currentUpdate,
      isChecking: this.isChecking,
      isDownloading: this.isDownloading,
      downloadProgress: this.downloadProgress,
      autoCheck: this.autoCheck,
      developmentMode: this.isDevelopmentMode(),
      version: "v2", // Indica che stiamo usando il manager v2
    };
  }

  /**
   * Attiva/disattiva il controllo automatico
   */
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
      console.warn("Could not save auto-check preference:", error);
    }
  }

  /**
   * Carica le preferenze dell'utente
   */
  loadPreferences() {
    try {
      const autoCheck = localStorage.getItem("presto_auto_check_updates");
      if (autoCheck !== null) {
        this.setAutoCheck(autoCheck === "true");
      }
    } catch (error) {
      console.warn("Could not load preferences:", error);
    }
  }

  /**
   * Registra un listener per gli eventi
   */
  on(event, callback) {
    this.eventTarget.addEventListener(event, callback);
  }

  /**
   * Rimuove un listener per gli eventi
   */
  off(event, callback) {
    this.eventTarget.removeEventListener(event, callback);
  }

  /**
   * Emette un evento personalizzato
   */
  emit(event, data = null) {
    this.eventTarget.dispatchEvent(new CustomEvent(event, { detail: data }));
  }

  /**
   * Cleanup delle risorse
   */
  destroy() {
    this.stopAutoCheck();
    this.eventTarget = null;
  }
}

// Esporta un'istanza singleton
export const updateManager = new UpdateManagerV2();

// Debug utilities per test
if (typeof window !== "undefined") {
  window.updateManagerV2Debug = {
    enableTestMode: () => updateManager.enableTestMode(),
    disableTestMode: () => updateManager.disableTestMode(),
    checkForUpdates: () => updateManager.checkForUpdates(true),
    getStatus: () => {
      const status = updateManager.getStatus();
      console.table(status);
      return status;
    },
    getCurrentVersion: () => updateManager.getCurrentVersion(),
    openReleasePage: () => updateManager.openReleasePage(),
    testUpdate: async () => {
      console.log("🧪 Full update test...");
      updateManager.enableTestMode();
      return await updateManager.checkForUpdates(true);
    },
  };

  console.log("🔧 UpdateManager V2 Debug available: window.updateManagerV2Debug");
  console.log("📋 Comandi disponibili:");
  console.log("  - window.updateManagerV2Debug.testUpdate()");
  console.log("  - window.updateManagerV2Debug.checkForUpdates()");
  console.log("  - window.updateManagerV2Debug.getStatus()");
}
