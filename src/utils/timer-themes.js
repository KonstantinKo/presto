// Timer Themes Configuration
// This file defines all available timer themes and their properties
import { logger } from "./logger.js";

export const TIMER_THEMES = {
  espresso: {
    name: "Espresso",
    description: "Warm, coffee-inspired colors with rich earth tones",
    supports: ["light", "dark"],
    isDefault: true,
    preview: {
      focus: "#e74c3c",
      break: "#2ecc71",
      longBreak: "#3498db",
    },
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

// Function to dynamically register a new theme
export function registerTheme(themeId, themeConfig) {
  if (TIMER_THEMES[themeId]) {
    logger.warn(`🎨 Theme ${themeId} already exists, overriding...`);
  }

  TIMER_THEMES[themeId] = {
    name: themeConfig.name || themeId,
    description: themeConfig.description || `Theme: ${themeId}`,
    supports: themeConfig.supports || ["light", "dark"],
    isDefault: themeConfig.isDefault || false,
    preview: themeConfig.preview || {
      focus: "#e74c3c",
      break: "#2ecc71",
      longBreak: "#3498db",
    },
  };

  logger.info(`✅ Registered theme: ${themeId}`, TIMER_THEMES[themeId]);
  return TIMER_THEMES[themeId];
}

// Function to unregister a theme
export function unregisterTheme(themeId) {
  if (TIMER_THEMES[themeId] && !TIMER_THEMES[themeId].isDefault) {
    delete TIMER_THEMES[themeId];
    logger.info(`🗑️ Unregistered theme: ${themeId}`);
    return true;
  }
  return false;
}

// Get theme by ID
export function getThemeById(themeId) {
  return TIMER_THEMES[themeId] || TIMER_THEMES.espresso;
}

// Get all available themes
export function getAllThemes() {
  return Object.entries(TIMER_THEMES).map(([id, theme]) => ({
    id,
    ...theme,
  }));
}

// Get compatible themes for current color mode
export function getCompatibleThemes(colorMode = "light") {
  return getAllThemes().filter((theme) => theme.supports.includes(colorMode));
}

// Check if theme is compatible with color mode
export function isThemeCompatible(themeId, colorMode = "light") {
  const theme = getThemeById(themeId);
  return theme.supports.includes(colorMode);
}

// Get default theme
export function getDefaultTheme() {
  const defaultTheme = getAllThemes().find((theme) => theme.isDefault);
  return defaultTheme || getAllThemes()[0];
}
