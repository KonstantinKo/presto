import { logger } from "./logger.js";

/**
 * @typedef {{ name: string; description: string; supports: ('light'|'dark')[]; isDefault: boolean; preview: { focus: string; break: string; longBreak: string } }} ThemeConfig
 */

const DEFAULT_PREVIEW = {
  focus: "#e74c3c",
  break: "#2ecc71",
  longBreak: "#3498db",
};

/** @type {Record<string, ThemeConfig>} */
export const TIMER_THEMES = {
  espresso: {
    name: "Espresso",
    description: "Warm, coffee-inspired colors with rich earth tones",
    supports: ["light", "dark"],
    isDefault: true,
    preview: DEFAULT_PREVIEW,
  },
  pommodore64: {
    name: "Pommodore64",
    description: "Un tema retrò ispirato al Commodore 64 con colori nostalgici e font pixelato",
    supports: ["light"],
    isDefault: false,
    preview: {
      focus: "#6c5ce7",
      break: "#0984e3",
      longBreak: "#00b894",
    },
  },
  pipboy: {
    name: "PipBoy",
    description:
      "A retro-futuristic theme inspired by Fallout's PipBoy interface with green terminal colors and digital effects",
    supports: ["dark"],
    isDefault: false,
    preview: {
      focus: "#00ff41",
      break: "#39ff14",
      longBreak: "#00cc33",
    },
  },
};

/**
 * @param {string} themeId
 * @param {Partial<ThemeConfig>} themeConfig
 * @returns {ThemeConfig}
 */
export function registerTheme(themeId, themeConfig) {
  const existingTheme = TIMER_THEMES[themeId];

  if (existingTheme) {
    logger.warn(`🎨 Theme ${themeId} already exists, overriding...`);
  }

  const theme = {
    name: themeConfig.name || themeId,
    description: themeConfig.description || `Theme: ${themeId}`,
    supports: themeConfig.supports || ["light", "dark"],
    isDefault: themeConfig.isDefault ?? existingTheme?.isDefault ?? false,
    preview: themeConfig.preview || DEFAULT_PREVIEW,
  };

  TIMER_THEMES[themeId] = theme;
  logger.info(`✅ Registered theme: ${themeId}`, theme);
  return theme;
}

/** @param {string} themeId @returns {boolean} */
export function unregisterTheme(themeId) {
  if (TIMER_THEMES[themeId] && !TIMER_THEMES[themeId].isDefault) {
    delete TIMER_THEMES[themeId];
    logger.info(`🗑️ Unregistered theme: ${themeId}`);
    return true;
  }
  return false;
}

/** @param {string} themeId @returns {ThemeConfig} */
export function getThemeById(themeId) {
  return TIMER_THEMES[themeId] || TIMER_THEMES.espresso;
}

export function getAllThemes() {
  return Object.entries(TIMER_THEMES).map(([id, theme]) => ({
    id,
    ...theme,
  }));
}

/** @param {string} [colorMode] @returns {(ThemeConfig & { id: string })[]} */
export function getCompatibleThemes(colorMode = "light") {
  return getAllThemes().filter((theme) =>
    theme.supports.includes(/** @type {'light'|'dark'} */ (colorMode))
  );
}

/**
 * @param {string} themeId
 * @param {string} [colorMode]
 * @returns {boolean}
 */
export function isThemeCompatible(themeId, colorMode = "light") {
  const theme = getThemeById(themeId);
  return theme.supports.includes(/** @type {'light'|'dark'} */ (colorMode));
}

export function getDefaultTheme() {
  return getAllThemes().find((theme) => theme.isDefault);
}
