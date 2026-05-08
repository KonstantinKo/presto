/**
 * Update Manager
 * 
 * Gestisce il controllo e l'installazione degli aggiornamenti dell'applicazione
 * tramite GitHub releases utilizzando il plugin updater di Tauri.
 */

export class UpdateManager {
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
    }

    /**
     * Verifica se siamo in modalità sviluppo
     */
    isDevelopmentMode() {
        return !window.__TAURI__ || window.location.hostname === 'localhost';
    }

    /**
     * Ottiene l'API updater di Tauri in modo sicuro
     */
    async getTauriUpdater() {
        if (!window.__TAURI__) {
            throw new Error('API Tauri non disponibili');
        }

        try {
            // Prova l'import dinamico (metodo consigliato per Tauri v2)
            const { check, relaunch } = await import('@tauri-apps/plugin-updater');
            return { check, relaunch };
        } catch (error) {
            console.error('Error loading updater API:', error);

            // Fallback all'API globale se disponibile
            if (window.__TAURI__.updater) {
                return window.__TAURI__.updater;
            }

            throw new Error('Plugin updater non disponibile');
        }
    }

    /**
     * Mostra un messaggio all'utente in modo sicuro
     */
    async showMessage(content, options = {}) {
        try {
            // Prova prima con l'API globale
            if (window.__TAURI__?.dialog?.message) {
                return await window.__TAURI__.dialog.message(content, options);
            }

            // Altrimenti prova l'import dinamico
            const { message } = await import('@tauri-apps/plugin-dialog');
            return await message(content, options);
        } catch (error) {
            console.error('Error showing message:', error);
            // Fallback al browser alert
            alert(content);
        }
    }

    /**
     * Chiede conferma all'utente
     */
    async askUser(content, options = {}) {
        try {
            // Prova prima con l'API globale
            if (window.__TAURI__?.dialog?.ask) {
                return await window.__TAURI__.dialog.ask(content, options);
            }

            // Altrimenti prova l'import dinamico
            const { ask } = await import('@tauri-apps/plugin-dialog');
            return await ask(content, options);
        } catch (error) {
            console.error('Error asking confirmation:', error);
            // Fallback al browser confirm
            return confirm(content);
        }
    }

    /**
     * Mostra messaggio per modalità sviluppo
     */
    async showDevelopmentMessage() {
        await this.showMessage(
            'Update check not available in development mode.\n\nGli aggiornamenti funzioneranno solo nell\'applicazione compilata.',
            {
                title: 'Modalità Sviluppo',
                kind: 'info'
            }
        );
    }

    /**
     * Avvia il controllo automatico degli aggiornamenti
     */
    startAutoCheck() {
        if (this.autoCheck && !this.checkInterval && !this.isDevelopmentMode()) {
            // Controlla ogni ora
            this.checkInterval = setInterval(() => {
                this.checkForUpdates(false); // silent check
            }, 60 * 60 * 1000);

            // Controllo iniziale dopo 30 secondi
            setTimeout(() => {
                this.checkForUpdates(false);
            }, 30000);
        }
    }

    /**
     * Ferma il controllo automatico degli aggiornamenti
     */
    stopAutoCheck() {
        if (this.checkInterval) {
            clearInterval(this.checkInterval);
            this.checkInterval = null;
        }
    }

    /**
     * Controlla se sono disponibili aggiornamenti
     * @param {boolean} showDialog - Se mostrare dialoghi all'utente
     * @returns {Promise<boolean>} - True se sono disponibili aggiornamenti
     */
    async checkForUpdates(showDialog = true) {
        if (this.isChecking) {
            return false;
        }

        this.isChecking = true;
        this.emit('checkStarted');

        try {
            console.log('🔄 Checking for updates...');

            // Verifica se siamo in un ambiente Tauri
            if (this.isDevelopmentMode()) {
                console.warn('⚠️  Update check not available in development mode');
                this.emit('updateNotAvailable');
                if (showDialog) {
                    await this.showDevelopmentMessage();
                }
                return false;
            }

            // Usa l'API updater di Tauri
            const { check } = await this.getTauriUpdater();

            console.log('🔍 Sending update check request...');
            const update = await check();
            console.log('📦 Update check response:', update);

            if (update?.available) {
                console.log('✅ Update available:', update.version);
                this.updateAvailable = true;
                this.currentUpdate = update;
                this.emit('updateAvailable', update);

                if (showDialog) {
                    await this.showUpdateDialog(update);
                }

                return true;
            } else {
                console.log('✅ No updates available');
                this.updateAvailable = false;
                this.currentUpdate = null;
                this.emit('updateNotAvailable');

                if (showDialog) {
                    await this.showMessage('Stai usando la versione più recente!', {
                        title: 'Nessun aggiornamento',
                        kind: 'info'
                    });
                }

                return false;
            }
        } catch (error) {
            console.error('❌ Error during update check:', error);
            console.error('❌ Stack trace:', error.stack);
            this.emit('checkError', error);

            let errorMessage = 'Errore durante il controllo degli aggiornamenti.';

            // Fornisci messaggi di errore più specifici
            if (error.message.includes('network') || error.message.includes('request')) {
                errorMessage = 'Errore di rete durante il controllo degli aggiornamenti. Verifica la connessione a Internet.';
            } else if (error.message.includes('permission')) {
                errorMessage = 'Permessi insufficienti per controllare gli aggiornamenti.';
            } else if (error.message.includes('not available')) {
                errorMessage = 'Sistema di aggiornamenti non disponibile in questa versione.';
            }

            if (showDialog) {
                await this.showMessage(`${errorMessage}\n\nDettagli: ${error.message}`, {
                    title: 'Errore Aggiornamenti',
                    kind: 'error'
                });
            }

            return false;
        } finally {
            this.isChecking = false;
            this.emit('checkFinished');
        }
    }

    /**
     * Mostra il dialogo di conferma aggiornamento
     * @param {object} update - Informazioni sull'aggiornamento
     */
    async showUpdateDialog(update) {
        const shouldUpdate = await this.askUser(
            `È disponibile una nuova versione (${update.version}).\n\nVuoi scaricare e installare l'aggiornamento ora?`,
            {
                title: 'Update available',
                kind: 'info'
            }
        );

        if (shouldUpdate) {
            await this.downloadAndInstall();
        }
    }

    /**
     * Scarica e installa l'aggiornamento
     */
    async downloadAndInstall() {
        if (!this.currentUpdate || this.isDownloading) {
            return;
        }

        this.isDownloading = true;
        this.downloadProgress = 0;
        this.emit('downloadStarted');

        try {
            console.log('📥 Starting update download...');

            // Monitora il progresso del download
            await this.currentUpdate.downloadAndInstall((event) => {
                switch (event.event) {
                    case 'Started':
                        console.log('📥 Download iniziato');
                        this.emit('downloadProgress', { progress: 0, contentLength: event.data.contentLength });
                        break;
                    case 'Progress':
                        this.downloadProgress = Math.round((event.data.chunkLength / event.data.contentLength) * 100);
                        console.log(`📊 Progresso download: ${this.downloadProgress}%`);
                        this.emit('downloadProgress', {
                            progress: this.downloadProgress,
                            chunkLength: event.data.chunkLength,
                            contentLength: event.data.contentLength
                        });
                        break;
                    case 'Finished':
                        console.log('✅ Download complete');
                        this.downloadProgress = 100;
                        this.emit('downloadFinished');
                        break;
                }
            });

            console.log('🔄 Update installed, restarting...');
            this.emit('installFinished');

            // Mostra messaggio di successo prima del riavvio
            await this.showMessage('Aggiornamento installato con successo!\n\nL\'applicazione verrà riavviata ora.', {
                title: 'Aggiornamento completato',
                kind: 'info'
            });

            // Riavvia l'applicazione
            await this.relaunchApplication();

        } catch (error) {
            console.error('❌ Error during installation:', error);
            this.emit('downloadError', error);

            await this.showMessage(`Errore durante l'installazione dell'aggiornamento: ${error.message}`, {
                title: 'Errore',
                kind: 'error'
            });
        } finally {
            this.isDownloading = false;
        }
    }

    /**
     * Riavvia l'applicazione in modo sicuro
     */
    async relaunchApplication() {
        try {
            // Usa l'API updater per il riavvio
            const { relaunch } = await this.getTauriUpdater();
            await relaunch();
        } catch (error) {
            console.error('Error during restart:', error);
            await this.showMessage('L\'aggiornamento è stato installato ma c\'è stato un problema con il riavvio automatico.\n\nRiavvia manualmente l\'applicazione.', {
                title: 'Riavvio manuale richiesto',
                kind: 'warning'
            });
        }
    }

    /**
     * Ottiene la versione corrente dell'applicazione
     */
    async getCurrentVersion() {
        try {
            if (window.__TAURI__?.app?.getVersion) {
                return await window.__TAURI__.app.getVersion();
            }
            return '0.1.0'; // fallback
        } catch (error) {
            console.warn('Could not retrieve current version:', error);
            return '0.1.0';
        }
    }

    /**
     * Apre la pagina delle release su GitHub
     */
    async openReleasePage() {
        try {
            const url = 'https://github.com/murdercode/presto/releases';
            if (window.__TAURI__?.shell?.open) {
                await window.__TAURI__.shell.open(url);
            } else {
                window.open(url, '_blank');
            }
        } catch (error) {
            console.error('Error opening release page:', error);
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
            developmentMode: this.isDevelopmentMode()
        };
    }

    /**
     * Attiva/disattiva il controllo automatico
     * @param {boolean} enabled 
     */
    setAutoCheck(enabled) {
        this.autoCheck = enabled;
        if (enabled) {
            this.startAutoCheck();
        } else {
            this.stopAutoCheck();
        }

        // Salva la preferenza nell'storage locale
        try {
            localStorage.setItem('presto_auto_check_updates', enabled.toString());
        } catch (error) {
            console.warn('Could not save auto-check preference:', error);
        }
    }

    /**
     * Carica le preferenze dell'utente
     */
    loadPreferences() {
        try {
            const autoCheck = localStorage.getItem('presto_auto_check_updates');
            if (autoCheck !== null) {
                this.setAutoCheck(autoCheck === 'true');
            }
        } catch (error) {
            console.warn('Could not load preferences:', error);
        }
    }

    /**
     * Registra un listener per gli eventi
     * @param {string} event 
     * @param {function} callback 
     */
    on(event, callback) {
        this.eventTarget.addEventListener(event, callback);
    }

    /**
     * Rimuove un listener per gli eventi
     * @param {string} event 
     * @param {function} callback 
     */
    off(event, callback) {
        this.eventTarget.removeEventListener(event, callback);
    }

    /**
     * Emette un evento personalizzato
     * @private
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
export const updateManager = new UpdateManager();
