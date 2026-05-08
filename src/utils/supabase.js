// Wait for Supabase to be available
function waitForSupabase() {
  return new Promise((resolve, reject) => {
    const check = () => {
      if (window.supabase && window.supabase.createClient) {
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
let supabase = null;
let authHelpers = null;

async function initSupabase() {
  await waitForSupabase();

  const { createClient } = window.supabase;

  // Hardcoded Supabase configuration
  const supabaseUrl = "https://lopgwwppinkqvttozqfx.supabase.co";
  const supabaseAnonKey =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImxvcGd3d3BwaW5rcXZ0dG96cWZ4Iiwicm9sZSI6ImFub24iLCJpYXQiOjE3NTA2NzgxMDIsImV4cCI6MjA2NjI1NDEwMn0.DqPcwsBJdPeV5iWsMkZLMn6-xZ_A9l-Xh7R-wi7kc2k";

  // Create Supabase client
  supabase = createClient(supabaseUrl, supabaseAnonKey, {
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
    // Sign in with email/password
    async signInWithEmail(email, password) {
      const { data, error } = await supabase.auth.signInWithPassword({
        email,
        password,
      });
      return { data, error };
    },

    // Sign up with email/password
    async signUpWithEmail(email, password) {
      const { data, error } = await supabase.auth.signUp({
        email,
        password,
      });
      return { data, error };
    },

    // Sign in with OAuth providers using tauri-plugin-oauth
    async signInWithProvider(provider) {
      try {
        if (!window.__TAURI__) {
          // Fallback to original Supabase OAuth for web
          console.log("Not in Tauri, using Supabase OAuth...");
          const { data, error } = await supabase.auth.signInWithOAuth({
            provider,
            options: {
              redirectTo: window.location.origin,
            },
          });
          return { data, error };
        }

        const { invoke } = window.__TAURI__.core;

        console.log(`Starting Tauri OAuth flow for ${provider}...`);

        // Get Supabase configuration
        const supabaseUrl = "https://lopgwwppinkqvttozqfx.supabase.co";

        // Start OAuth flow using tauri-plugin-oauth
        console.log("Invoking OAuth start...");

        try {
          // Start the OAuth server using our custom command
          const port = await invoke("start_oauth_server");
          console.log("OAuth server started on port:", port);

          // Generate redirect URI using the port
          const redirectUri = `http://localhost:${port}`;

          // Build OAuth URL
          const authUrl = `${supabaseUrl}/auth/v1/authorize?provider=${provider}&redirect_to=${encodeURIComponent(redirectUri)}`;

          console.log("Opening OAuth URL:", authUrl);

          // Open the OAuth URL in the default browser
          try {
            // Try the correct opener command format
            await invoke("plugin:opener|open_url", { url: authUrl });
          } catch (openerError) {
            console.log("opener plugin failed, trying alternative methods...", openerError);
            try {
              // Try without plugin prefix
              await invoke("open_url", { url: authUrl });
            } catch (openerError2) {
              console.log("open_url failed, trying shell.open...", openerError2);
              // Fallback to shell open
              if (window.__TAURI__?.shell) {
                await window.__TAURI__.shell.open(authUrl);
              } else {
                throw new Error("Cannot open browser - no opener available");
              }
            }
          }

          // Return a promise that resolves when OAuth completes
          return new Promise((resolve, reject) => {
            let timeout;
            let unlisten;
            (async () => {
              try {
                // Set up a timeout
                timeout = setTimeout(() => {
                  if (unlisten) {
                    unlisten();
                  }
                  try {
                    invoke("plugin:oauth|cancel", { port });
                  } catch (cancelError) {
                    console.log("Cancel command failed (this is usually fine):", cancelError);
                  }
                  reject(new Error("OAuth flow timed out"));
                }, 120000); // 2 minutes

                console.log("OAuth URL opened in browser. Please complete authentication...");
                console.log("Redirect URI:", redirectUri);

                // Set up event listener for OAuth callback
                const { listen } = window.__TAURI__.event;

                console.log("Setting up OAuth event listeners...");

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
                    console.log(`Trying to listen for event: ${eventName}`);
                    const tempUnlisten = await listen(eventName, async (event) => {
                      console.log(`Received ${eventName} event:`, event);

                      // Process the callback
                      await processOAuthCallback(
                        event.payload,
                        resolve,
                        reject,
                        timeout,
                        tempUnlisten
                      );
                    });

                    if (!unlisten) {
                      unlisten = tempUnlisten;
                    }
                  } catch (listenError) {
                    console.log(`Failed to listen for ${eventName}:`, listenError);
                  }
                }

                // Function to process OAuth callback
                async function processOAuthCallback(
                  callbackUrl,
                  resolve,
                  reject,
                  timeout,
                  unlisten
                ) {
                  try {
                    clearTimeout(timeout);
                    if (unlisten) {
                      unlisten();
                    }

                    console.log("Processing callback URL:", callbackUrl);

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

                    console.log("Parsed tokens:", {
                      hasAccessToken: !!accessToken,
                      hasRefreshToken: !!refreshToken,
                      error,
                    });

                    if (error) {
                      console.error("OAuth error in callback:", error);
                      try {
                        await invoke("plugin:oauth|cancel", { port });
                      } catch (cancelError) {
                        console.log("Cancel command failed (this is usually fine):", cancelError);
                      }
                      reject(new Error(`OAuth error: ${error}`));
                      return;
                    }

                    if (accessToken) {
                      console.log("Access token found, setting Supabase session...");

                      try {
                        const { data, error: sessionError } = await supabase.auth.setSession({
                          access_token: accessToken,
                          refresh_token: refreshToken,
                        });

                        try {
                          await invoke("plugin:oauth|cancel", { port });
                        } catch (cancelError) {
                          console.log("Cancel command failed (this is usually fine):", cancelError);
                        }

                        if (sessionError) {
                          console.error("Supabase session error:", sessionError);
                          reject(new Error(`Supabase session error: ${sessionError.message}`));
                        } else {
                          console.log("OAuth success! Session set:", data);
                          resolve({ data, error: null });
                        }
                      } catch (sessionSetupError) {
                        console.error("Session setup failed:", sessionSetupError);
                        try {
                          await invoke("plugin:oauth|cancel", { port });
                        } catch (cancelError) {
                          console.log("Cancel command failed (this is usually fine):", cancelError);
                        }
                        reject(new Error(`Session setup failed: ${sessionSetupError.message}`));
                      }
                    } else {
                      console.error("No access token found in callback URL");
                      try {
                        await invoke("plugin:oauth|cancel", { port });
                      } catch (cancelError) {
                        console.log("Cancel command failed (this is usually fine):", cancelError);
                      }
                      reject(new Error("No access token found in OAuth callback"));
                    }
                  } catch (parseError) {
                    console.error("Error parsing OAuth callback:", parseError);
                    clearTimeout(timeout);
                    if (unlisten) {
                      unlisten();
                    }
                    await invoke("plugin:oauth|cancel");
                    reject(new Error(`Failed to parse OAuth callback: ${parseError.message}`));
                  }
                }

                console.log("OAuth event listeners set up. Waiting for callback...");
              } catch (setupError) {
                console.error("Error setting up OAuth listeners:", setupError);
                clearTimeout(timeout);
                if (unlisten) {
                  unlisten();
                }
                reject(new Error(`OAuth setup failed: ${setupError.message}`));
              }
            })();
          });
        } catch (invokeError) {
          console.error("Error calling OAuth plugin:", invokeError);
          return { data: null, error: invokeError.message };
        }
      } catch (error) {
        console.error(`OAuth ${provider} error:`, error);
        return { data: null, error: error.message };
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
