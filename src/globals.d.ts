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
  _appInitializing?: boolean;
  _appFullyInitialized?: boolean;
  avatarListenersSetup?: boolean;

  // Global functions assigned in main.js
  saveSettings?: () => Promise<void>;
  resetToDefaults?: () => Promise<void>;
  confirmTotalReset?: () => void;
  performTotalReset?: () => Promise<void>;
}
