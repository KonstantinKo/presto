declare interface Window {
  // Tauri global – typed loosely; tightening namespaces is out of scope
  __TAURI__?: {
    core?: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
    event?: { listen: (event: string, handler: (e: any) => void) => Promise<() => void> };
    dialog?: { message: (msg: string, opts?: any) => Promise<void>; [k: string]: any };
    notification?: {
      isPermissionGranted: () => Promise<boolean>;
      requestPermission: () => Promise<string>;
      sendNotification: (opts: any) => Promise<void>;
      [k: string]: any;
    };
    updater?: any;
    shell?: { open: (url: string) => Promise<void>; [k: string]: any };
    [k: string]: any;
  };

  // UMD bundles loaded from CDN
  supabase?: { createClient: (url: string, key: string, opts?: any) => any } | undefined;
  XLSX?: any;

  // AudioContext vendor prefix
  webkitAudioContext?: typeof AudioContext;

  // Manager singletons assigned at runtime
  app?: any;
  appLog?: any;
  pomodoroTimer?: any;
  settingsManager?: any;
  sessionManager?: any;
  navigationManager?: any;
  teamManager?: any;
  tagManager?: any;
  authManager?: any;
  updateManager?: any;
  updateManagerInstance?: any;
  UpdateManagerV2?: any;
  updateManagerV2Debug?: any;
  updateNotification?: any;

  // Initialization flags
  avatarListenersSetup?: boolean;

  // E2E test bridge: populated by tests/e2e/fixtures via addInitScript. Production
  // builds never set this; runtime checks gate any use. See tests/e2e/fixtures/tauriMock.js.
  __E2E_CONFIG__?: {
    updaterCallCount?: number;
    updaterSecondCallUpdate?: { version?: string; currentVersion?: string };
    [k: string]: any;
  };

  // Global functions assigned in main.js
  saveSettings?: () => Promise<void>;
  resetToDefaults?: () => Promise<void>;
  confirmTotalReset?: () => Promise<void>;
  performTotalReset?: () => Promise<void>;
}
