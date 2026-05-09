import { initSupabase, getSupabase, getAuthHelpers } from "../utils/supabase.js";
import { logger } from "../utils/logger.js";
import { toError } from "../utils/to-error.js";
import { md5 } from "../utils/md5.js";

class AuthManager {
  constructor() {
    this.currentUser = null;
    this.isGuest = false;
    this.authListeners = /** @type {Function[]} */ ([]);
    this.supabase = null;
    this.authHelpers = null;
    this.initialized = false;
    this.initPromise = null;
  }

  async init() {
    if (this.initialized) {
      return Promise.resolve();
    }
    if (this.initPromise) {
      return this.initPromise;
    }

    this.initPromise = (async () => {
      try {
        await initSupabase();
        this.supabase = getSupabase();
        this.authHelpers = getAuthHelpers();

        const {
          data: { session },
        } = await this.supabase.auth.getSession();
        if (session) {
          this.currentUser = session.user;
          this.isGuest = false;
          this.notifyAuthListeners("authenticated", this.currentUser);
        } else {
          // Check if user chose to continue as guest
          const guestMode = localStorage.getItem("presto-guest-mode");
          if (guestMode === "true") {
            this.isGuest = true;
            this.notifyAuthListeners("guest", null);
          } else {
            this.notifyAuthListeners("unauthenticated", null);
          }
        }

        this.supabase.auth.onAuthStateChange(
          (/** @type {any} */ event, /** @type {any} */ session) => {
            if (event === "SIGNED_IN" && session) {
              this.currentUser = session.user;
              this.isGuest = false;
              localStorage.removeItem("presto-guest-mode");
              this.notifyAuthListeners("authenticated", this.currentUser);
            } else if (event === "SIGNED_OUT") {
              this.currentUser = null;
              this.isGuest = false;
              localStorage.removeItem("presto-guest-mode");
              this.notifyAuthListeners("unauthenticated", null);
            }
          }
        );

        this.initialized = true;
        logger.info("✅ AuthManager initialized with Supabase");
      } catch (error) {
        logger.error("Error checking authentication status:", error);
        this.initialized = false;
        this.notifyAuthListeners("unauthenticated", null);
        throw error;
      } finally {
        this.initPromise = null;
      }
    })();

    return this.initPromise;
  }

  isFirstRun() {
    const hasSeenAuth = localStorage.getItem("presto-auth-seen");
    return !hasSeenAuth;
  }

  markAuthSeen() {
    localStorage.setItem("presto-auth-seen", "true");
  }

  continueAsGuest() {
    this.isGuest = true;
    this.currentUser = null;
    localStorage.setItem("presto-guest-mode", "true");
    this.markAuthSeen();
    this.notifyAuthListeners("guest", null);
  }

  isAuthenticated() {
    return this.currentUser !== null;
  }

  isGuestMode() {
    return this.isGuest;
  }

  getCurrentUser() {
    return this.currentUser;
  }

  /** @param {string} email @param {string} password */
  async signInWithEmail(email, password) {
    try {
      if (!this.initialized) {
        await this.init();
      }
      const { data, error } = await this.authHelpers.signInWithEmail(email, password);
      if (error) {
        throw error;
      }

      this.markAuthSeen();
      return { success: true, data };
    } catch (error) {
      return { success: false, error: toError(error).message };
    }
  }

  /** @param {string} email @param {string} password */
  async signUpWithEmail(email, password) {
    try {
      if (!this.initialized) {
        await this.init();
      }
      const { data, error } = await this.authHelpers.signUpWithEmail(email, password);
      if (error) {
        throw error;
      }

      this.markAuthSeen();
      return { success: true, data };
    } catch (error) {
      return { success: false, error: toError(error).message };
    }
  }

  /** @param {string} provider */
  async signInWithProvider(provider) {
    try {
      if (!this.initialized) {
        await this.init();
      }
      const { data, error } = await this.authHelpers.signInWithProvider(provider);
      if (error) {
        throw error;
      }

      this.markAuthSeen();
      return { success: true, data };
    } catch (error) {
      return { success: false, error: toError(error).message };
    }
  }

  async signOut() {
    try {
      if (!this.initialized) {
        await this.init();
      }
      const { error } = await this.authHelpers.signOut();
      if (error) {
        throw error;
      }

      return { success: true };
    } catch (error) {
      return { success: false, error: toError(error).message };
    }
  }

  getUserAvatarUrl() {
    if (!this.currentUser) {
      return null;
    }

    const avatarUrl = this.currentUser.user_metadata?.avatar_url;
    if (avatarUrl) {
      return avatarUrl;
    }

    const picture = this.currentUser.user_metadata?.picture;
    if (picture) {
      return picture;
    }

    const email = this.currentUser.email;
    if (email) {
      const hash = md5(email.toLowerCase().trim());
      return `https://www.gravatar.com/avatar/${hash}?d=404&s=48`;
    }

    return null;
  }

  /** @param {string} email @returns {Promise<boolean>} */
  async checkGravatarExists(email) {
    if (!email) {
      return false;
    }

    const hash = md5(email.toLowerCase().trim());
    const gravatarUrl = `https://www.gravatar.com/avatar/${hash}?d=404&s=48`;

    try {
      const response = await fetch(gravatarUrl, { method: "HEAD" });
      return response.ok;
    } catch (_error) {
      return false;
    }
  }

  getUserDisplayName() {
    if (!this.currentUser) {
      return "Guest";
    }

    const name =
      this.currentUser.user_metadata?.full_name ||
      this.currentUser.user_metadata?.name ||
      this.currentUser.email?.split("@")[0] ||
      "User";

    return name;
  }

  /** @param {Function} callback */
  onAuthChange(callback) {
    this.authListeners.push(callback);
  }

  /** @param {Function} callback */
  removeAuthListener(callback) {
    this.authListeners = this.authListeners.filter((listener) => listener !== callback);
  }

  /** @param {string} status @param {any} user */
  notifyAuthListeners(status, user) {
    this.authListeners.forEach((callback) => {
      callback(status, user);
    });
  }
}

// Create singleton instance
export const authManager = new AuthManager();
