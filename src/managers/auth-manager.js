import { initSupabase, getSupabase, getAuthHelpers } from "../utils/supabase.js";
import { logger } from "../utils/logger.js";
import { toError } from "../utils/to-error.js";

/** @param {number} a @param {number} b @returns {number} */
function add32(a, b) {
  return (a + b) & 0xffffffff;
}

/** @param {number} q @param {number} a @param {number} b @param {number} x @param {number} s @param {number} t @returns {number} */
function cmn(q, a, b, x, s, t) {
  a = add32(add32(a, q), add32(x, t));
  return add32((a << s) | (a >>> (32 - s)), b);
}

/** @param {number} a @param {number} b @param {number} c @param {number} d @param {number} x @param {number} s @param {number} t @returns {number} */
const ff = (a, b, c, d, x, s, t) => cmn((b & c) | (~b & d), a, b, x, s, t);
/** @param {number} a @param {number} b @param {number} c @param {number} d @param {number} x @param {number} s @param {number} t @returns {number} */
const gg = (a, b, c, d, x, s, t) => cmn((b & d) | (c & ~d), a, b, x, s, t);
/** @param {number} a @param {number} b @param {number} c @param {number} d @param {number} x @param {number} s @param {number} t @returns {number} */
const hh = (a, b, c, d, x, s, t) => cmn(b ^ c ^ d, a, b, x, s, t);
/** @param {number} a @param {number} b @param {number} c @param {number} d @param {number} x @param {number} s @param {number} t @returns {number} */
const ii = (a, b, c, d, x, s, t) => cmn(c ^ (b | ~d), a, b, x, s, t);

/** @param {number[]} x @param {number[]} k */
function md5cycle(x, k) {
  let a = x[0],
    b = x[1],
    c = x[2],
    d = x[3];
  a = ff(a, b, c, d, k[0], 7, -680876936);
  d = ff(d, a, b, c, k[1], 12, -389564586);
  c = ff(c, d, a, b, k[2], 17, 606105819);
  b = ff(b, c, d, a, k[3], 22, -1044525330);
  a = ff(a, b, c, d, k[4], 7, -176418897);
  d = ff(d, a, b, c, k[5], 12, 1200080426);
  c = ff(c, d, a, b, k[6], 17, -1473231341);
  b = ff(b, c, d, a, k[7], 22, -45705983);
  a = ff(a, b, c, d, k[8], 7, 1770035416);
  d = ff(d, a, b, c, k[9], 12, -1958414417);
  c = ff(c, d, a, b, k[10], 17, -42063);
  b = ff(b, c, d, a, k[11], 22, -1990404162);
  a = ff(a, b, c, d, k[12], 7, 1804603682);
  d = ff(d, a, b, c, k[13], 12, -40341101);
  c = ff(c, d, a, b, k[14], 17, -1502002290);
  b = ff(b, c, d, a, k[15], 22, 1236535329);
  a = gg(a, b, c, d, k[1], 5, -165796510);
  d = gg(d, a, b, c, k[6], 9, -1069501632);
  c = gg(c, d, a, b, k[11], 14, 643717713);
  b = gg(b, c, d, a, k[0], 20, -373897302);
  a = gg(a, b, c, d, k[5], 5, -701558691);
  d = gg(d, a, b, c, k[10], 9, 38016083);
  c = gg(c, d, a, b, k[15], 14, -660478335);
  b = gg(b, c, d, a, k[4], 20, -405537848);
  a = gg(a, b, c, d, k[9], 5, 568446438);
  d = gg(d, a, b, c, k[14], 9, -1019803690);
  c = gg(c, d, a, b, k[3], 14, -187363961);
  b = gg(b, c, d, a, k[8], 20, 1163531501);
  a = gg(a, b, c, d, k[13], 5, -1444681467);
  d = gg(d, a, b, c, k[2], 9, -51403784);
  c = gg(c, d, a, b, k[7], 14, 1735328473);
  b = gg(b, c, d, a, k[12], 20, -1926607734);
  a = hh(a, b, c, d, k[5], 4, -378558);
  d = hh(d, a, b, c, k[8], 11, -2022574463);
  c = hh(c, d, a, b, k[11], 16, 1839030562);
  b = hh(b, c, d, a, k[14], 23, -35309556);
  a = hh(a, b, c, d, k[1], 4, -1530992060);
  d = hh(d, a, b, c, k[4], 11, 1272893353);
  c = hh(c, d, a, b, k[7], 16, -155497632);
  b = hh(b, c, d, a, k[10], 23, -1094730640);
  a = hh(a, b, c, d, k[13], 4, 681279174);
  d = hh(d, a, b, c, k[0], 11, -358537222);
  c = hh(c, d, a, b, k[3], 16, -722521979);
  b = hh(b, c, d, a, k[6], 23, 76029189);
  a = hh(a, b, c, d, k[9], 4, -640364487);
  d = hh(d, a, b, c, k[12], 11, -421815835);
  c = hh(c, d, a, b, k[15], 16, 530742520);
  b = hh(b, c, d, a, k[2], 23, -995338651);
  a = ii(a, b, c, d, k[0], 6, -198630844);
  d = ii(d, a, b, c, k[7], 10, 1126891415);
  c = ii(c, d, a, b, k[14], 15, -1416354905);
  b = ii(b, c, d, a, k[5], 21, -57434055);
  a = ii(a, b, c, d, k[12], 6, 1700485571);
  d = ii(d, a, b, c, k[3], 10, -1894986606);
  c = ii(c, d, a, b, k[10], 15, -1051523);
  b = ii(b, c, d, a, k[1], 21, -2054922799);
  a = ii(a, b, c, d, k[8], 6, 1873313359);
  d = ii(d, a, b, c, k[15], 10, -30611744);
  c = ii(c, d, a, b, k[6], 15, -1560198380);
  b = ii(b, c, d, a, k[13], 21, 1309151649);
  a = ii(a, b, c, d, k[4], 6, -145523070);
  d = ii(d, a, b, c, k[11], 10, -1120210379);
  c = ii(c, d, a, b, k[2], 15, 718787259);
  b = ii(b, c, d, a, k[9], 21, -343485551);
  x[0] = add32(a, x[0]);
  x[1] = add32(b, x[1]);
  x[2] = add32(c, x[2]);
  x[3] = add32(d, x[3]);
}

/** @param {string} s @returns {number[]} */
function md5blk(s) {
  const md5blks = [];
  for (let i = 0; i < 64; i += 4) {
    md5blks[i >> 2] =
      s.charCodeAt(i) +
      (s.charCodeAt(i + 1) << 8) +
      (s.charCodeAt(i + 2) << 16) +
      (s.charCodeAt(i + 3) << 24);
  }
  return md5blks;
}

/** @param {number} n @returns {string} */
function rhex(n) {
  let s = "",
    j = 0;
  for (let i = 0; i < 4; i++) {
    s += ((n >> (j + 4)) & 0x0f).toString(16) + ((n >> j) & 0x0f).toString(16);
    j += 8;
  }
  return s;
}

/** @param {string} input @returns {string} */
function md5(input) {
  const n = input.length;
  const state = [1732584193, -271733879, -1732584194, 271733878];
  let i;
  for (i = 64; i <= n; i += 64) {
    md5cycle(state, md5blk(input.substring(i - 64, i)));
  }
  const s = input.substring(i - 64);
  const tail = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  for (i = 0; i < s.length; i++) {
    tail[i >> 2] |= s.charCodeAt(i) << ((i % 4) << 3);
  }
  tail[i >> 2] |= 0x80 << ((i % 4) << 3);
  if (i > 55) {
    md5cycle(state, tail);
    for (i = 0; i < 16; i++) {
      tail[i] = 0;
    }
  }
  tail[14] = n * 8;
  md5cycle(state, tail);
  return state.map(rhex).join("");
}

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
      return;
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
