import { logger } from "./logger.js";
import { toError } from "./to-error.js";

const SUPABASE_URL = "https://lopgwwppinkqvttozqfx.supabase.co";

function waitForSupabase() {
  return new Promise((/** @type {(val?: any) => void} */ resolve, reject) => {
    const check = () => {
      if (window.supabase) {
        resolve();
      } else {
        setTimeout(check, 50);
      }
    };
    check();
    setTimeout(() => reject(new Error("Supabase timeout")), 5000);
  });
}

// Initialize Supabase client
/** @type {any} */
let supabase = null;
/** @type {any} */
let authHelpers = null;

async function initSupabase() {
  await waitForSupabase();

  const { createClient } = /** @type {NonNullable<Window['supabase']>} */ (window.supabase);

  const supabaseAnonKey =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImxvcGd3d3BwaW5rcXZ0dG96cWZ4Iiwicm9sZSI6ImFub24iLCJpYXQiOjE3NTA2NzgxMDIsImV4cCI6MjA2NjI1NDEwMn0.DqPcwsBJdPeV5iWsMkZLMn6-xZ_A9l-Xh7R-wi7kc2k";

  // Create Supabase client
  supabase = createClient(SUPABASE_URL, supabaseAnonKey, {
    auth: {
      // Configure redirect URLs for OAuth
      redirectTo: window.location.origin,
      // Enable deep links for OAuth in Tauri
      detectSessionInUrl: true,
      persistSession: true,
      autoRefreshToken: true,
    },
  });

  // Auth helper functions
  authHelpers = {
    /** @param {string} email @param {string} password */
    async signInWithEmail(email, password) {
      const { data, error } = await supabase.auth.signInWithPassword({
        email,
        password,
      });
      return { data, error };
    },

    /** @param {string} email @param {string} password */
    async signUpWithEmail(email, password) {
      const { data, error } = await supabase.auth.signUp({
        email,
        password,
      });
      return { data, error };
    },

    /** @param {string} provider */
    async signInWithProvider(provider) {
      try {
        if (!window.__TAURI__) {
          // Fallback to original Supabase OAuth for web
          logger.debug("Not in Tauri, using Supabase OAuth...");
          const { data, error } = await supabase.auth.signInWithOAuth({
            provider,
            options: {
              redirectTo: window.location.origin,
            },
          });
          return { data, error };
        }

        const { invoke } = /** @type {{ invoke: (cmd: string, args?: any) => Promise<any> }} */ (
          window.__TAURI__?.core ?? {}
        );

        logger.debug(`Starting Tauri OAuth flow for ${provider}...`);

        // Start OAuth flow using tauri-plugin-oauth
        logger.debug("Invoking OAuth start...");

        /** @type {number | undefined} */
        let port;
        const cancelOauthServer = async () => {
          if (port != null) {
            try {
              await invoke("plugin:oauth|cancel", { port });
            } catch (cancelError) {
              logger.info("Cancel command failed (this is usually fine):", cancelError);
            }
          }
        };

        try {
          // Start the OAuth server using our custom command
          port = await invoke("start_oauth_server");
          logger.debug("OAuth server started on port:", port);

          // Generate redirect URI using the port
          const redirectUri = `http://localhost:${port}`;

          // Build OAuth URL
          const authUrl = `${SUPABASE_URL}/auth/v1/authorize?provider=${provider}&redirect_to=${encodeURIComponent(redirectUri)}`;

          // Return a promise that resolves when OAuth completes
          return new Promise((resolve, reject) => {
            /** @type {ReturnType<typeof setTimeout> | undefined} */
            let timeout;
            const unlisteners = /** @type {(() => void)[]} */ ([]);
            const cleanupListeners = () => {
              for (const off of unlisteners) {
                try {
                  off();
                } catch (_) {
                  // ignore individual unlisten errors
                }
              }
              unlisteners.length = 0;
            };
            let processed = false;
            (async () => {
              try {
                // Set up a timeout
                timeout = setTimeout(() => {
                  cleanupListeners();
                  cancelOauthServer().catch(() => {});
                  reject(new Error("OAuth flow timed out"));
                }, 120000); // 2 minutes

                logger.debug("Redirect URI:", redirectUri);

                // Set up event listener for OAuth callback
                const { listen } =
                  /** @type {{ listen: (event: string, handler: (e: any) => void) => Promise<() => void> }} */ (
                    window.__TAURI__?.event ?? {}
                  );

                logger.debug("Setting up OAuth event listeners...");

                // Try multiple possible event names, prioritizing our custom event
                const possibleEvents = [
                  "oauth-callback",
                  "oauth-url",
                  "redirect_uri",
                  "oauth_callback",
                  "oauth:callback",
                ];

                for (const eventName of possibleEvents) {
                  try {
                    logger.debug(`Trying to listen for event: ${eventName}`);
                    const tempUnlisten = await listen(
                      eventName,
                      async (/** @type {any} */ event) => {
                        if (processed) {
                          return;
                        }
                        processed = true;
                        cleanupListeners();
                        logger.debug("Received OAuth callback event", {
                          eventName,
                          hasPayload: Boolean(event?.payload),
                        });

                        // Process the callback
                        await processOAuthCallback(
                          event.payload,
                          resolve,
                          reject,
                          /** @type {ReturnType<typeof setTimeout>} */ (timeout)
                        );
                      }
                    );

                    unlisteners.push(tempUnlisten);
                  } catch (listenError) {
                    logger.debug(`Failed to listen for ${eventName}:`, listenError);
                  }
                }

                if (unlisteners.length === 0) {
                  clearTimeout(timeout);
                  await cancelOauthServer();
                  reject(new Error("OAuth listener registration failed"));
                  return;
                }

                // Open the OAuth URL in the default browser (after listeners are registered)
                logger.debug("Opening OAuth URL:", authUrl);
                try {
                  // Try the correct opener command format
                  await invoke("plugin:opener|open_url", { url: authUrl });
                } catch (openerError) {
                  logger.debug("opener plugin failed, trying alternative methods...", openerError);
                  try {
                    // Try without plugin prefix
                    await invoke("open_url", { url: authUrl });
                  } catch (openerError2) {
                    logger.debug("open_url failed, trying shell.open...", openerError2);
                    // Fallback to shell open
                    if (window.__TAURI__?.shell) {
                      await window.__TAURI__.shell.open(authUrl);
                    } else {
                      throw new Error("Cannot open browser - no opener available");
                    }
                  }
                }

                logger.debug("OAuth URL opened in browser. Please complete authentication...");

                // Function to process OAuth callback
                /**
                 * @param {string} callbackUrl
                 * @param {(val: any) => void} resolve
                 * @param {(err: Error) => void} reject
                 * @param {ReturnType<typeof setTimeout>} timeout
                 */
                async function processOAuthCallback(callbackUrl, resolve, reject, timeout) {
                  try {
                    clearTimeout(timeout);

                    logger.debug("Processing OAuth callback");

                    // Parse the callback URL to extract tokens
                    const url = new URL(callbackUrl);
                    const fragment = url.hash.substring(1);
                    const searchParams = new URLSearchParams(url.search);
                    const hashParams = new URLSearchParams(fragment);

                    // Try to get tokens from either search params or hash params
                    const accessToken =
                      searchParams.get("access_token") || hashParams.get("access_token");
                    const refreshToken =
                      searchParams.get("refresh_token") || hashParams.get("refresh_token");
                    const error = searchParams.get("error") || hashParams.get("error");

                    logger.debug("Parsed tokens:", {
                      hasAccessToken: !!accessToken,
                      hasRefreshToken: !!refreshToken,
                      error,
                    });

                    if (error) {
                      logger.error("OAuth error in callback:", error);
                      await cancelOauthServer();
                      reject(new Error(`OAuth error: ${error}`));
                      return;
                    }

                    if (accessToken) {
                      logger.debug("Access token found, setting Supabase session...");

                      try {
                        const { data, error: sessionError } = await supabase.auth.setSession({
                          access_token: accessToken,
                          refresh_token: refreshToken,
                        });

                        await cancelOauthServer();

                        if (sessionError) {
                          logger.error("Supabase session error:", sessionError);
                          reject(new Error(`Supabase session error: ${sessionError.message}`));
                        } else {
                          logger.info("OAuth success! Session established.");
                          resolve({ data, error: null });
                        }
                      } catch (sessionSetupError) {
                        logger.error("Session setup failed:", sessionSetupError);
                        await cancelOauthServer();
                        reject(
                          new Error(`Session setup failed: ${toError(sessionSetupError).message}`)
                        );
                      }
                    } else {
                      logger.error("No access token found in callback URL");
                      await cancelOauthServer();
                      reject(new Error("No access token found in OAuth callback"));
                    }
                  } catch (parseError) {
                    logger.error("Error parsing OAuth callback:", parseError);
                    clearTimeout(timeout);
                    await cancelOauthServer();
                    reject(
                      new Error(`Failed to parse OAuth callback: ${toError(parseError).message}`)
                    );
                  }
                }

                logger.debug("OAuth event listeners set up. Waiting for callback...");
              } catch (setupError) {
                logger.error("Error setting up OAuth listeners:", setupError);
                clearTimeout(timeout);
                cleanupListeners();
                await cancelOauthServer();
                reject(new Error(`OAuth setup failed: ${toError(setupError).message}`));
              }
            })();
          });
        } catch (invokeError) {
          logger.error("Error calling OAuth plugin:", invokeError);
          await cancelOauthServer();
          return { data: null, error: toError(invokeError).message };
        }
      } catch (error) {
        logger.error(`OAuth ${provider} error:`, error);
        return { data: null, error: toError(error).message };
      }
    },

    // Sign out
    async signOut() {
      const { error } = await supabase.auth.signOut();
      return { error };
    },

    // Get current user
    getCurrentUser() {
      return supabase.auth.getUser();
    },

    // Get current session
    getSession() {
      return supabase.auth.getSession();
    },

    // Listen for auth state changes
    /** @param {any} callback */
    onAuthStateChange(callback) {
      return supabase.auth.onAuthStateChange(callback);
    },
  };
}

// Export functions to get initialized instances
export function getSupabase() {
  return supabase;
}

export function getAuthHelpers() {
  return authHelpers;
}

// Initialize and export
export { initSupabase };
