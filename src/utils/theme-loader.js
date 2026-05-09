import { logger } from "./logger.js";

class ThemeLoader {
  constructor() {
    this.loadedThemes = new Set();
    this.themeStyles = new Map();
  }

  async loadAllThemes() {
    try {
      const themeFiles = this.discoverThemeFiles();

      logger.debug(`🎨 Discovered ${themeFiles.length} theme files:`, themeFiles);

      for (const themeFile of themeFiles) {
        await this.loadThemeFile(themeFile);
      }

      logger.debug(`🎨 Auto-loaded ${this.loadedThemes.size} themes successfully`);
      return Array.from(this.loadedThemes);
    } catch (error) {
      logger.error("❌ Failed to auto-load themes:", error);
      return [];
    }
  }

  discoverThemeFiles() {
    // Since we can't directly read the filesystem, we'll use a predefined list
    // that gets updated by the build process or manually maintained
    const knownThemes = ["espresso.css", "pipboy.css", "pommodore64.css"];

    return knownThemes;
  }

  /** @param {string} filename */
  async loadThemeFile(filename) {
    const themeId = filename.replace(".css", "");

    if (this.loadedThemes.has(themeId)) {
      logger.debug(`🎨 Theme ${themeId} already loaded, skipping`);
      return;
    }

    try {
      // Since CSS is already imported statically in main.css,
      // we just need to register the theme in our loaded themes
      logger.info(`✅ Theme registered: ${themeId}`);
      this.loadedThemes.add(themeId);

      await this.extractThemeMetadata(themeId);
    } catch (error) {
      logger.error(`❌ Error registering theme ${themeId}:`, error);
    }
  }

  /** @param {string} themeId */
  async extractThemeMetadata(themeId) {
    try {
      const response = await fetch(`./src/styles/themes/${themeId}.css`);
      const cssContent = await response.text();

      const metadata = this.parseThemeMetadata(cssContent);

      if (metadata) {
        const { TIMER_THEMES } = await import("./timer-themes.js");

        if (!TIMER_THEMES[themeId]) {
          TIMER_THEMES[themeId] = {
            name: metadata.name || this.capitalizeFirst(themeId),
            description: metadata.description || `Auto-discovered theme: ${themeId}`,
            supports: metadata.supports || /** @type {('light'|'dark')[]} */ (["light", "dark"]),
            isDefault: false,
            preview: metadata.preview || {
              focus: "#e74c3c",
              break: "#2ecc71",
              longBreak: "#3498db",
            },
          };

          logger.info(`📝 Auto-registered theme: ${themeId}`, TIMER_THEMES[themeId]);
        }
      }
    } catch (error) {
      logger.warn(`⚠️ Could not extract metadata for theme ${themeId}:`, error);
    }
  }

  /** @param {string} cssContent */
  parseThemeMetadata(cssContent) {
    try {
      const metadataRegex =
        /\/\*\s*Timer Theme:\s*(.+?)\s*\*\s*Author:\s*(.+?)\s*\*\s*Description:\s*(.+?)\s*\*\s*Supports:\s*(.+?)\s*\*\//s;
      const match = cssContent.match(metadataRegex);

      if (match) {
        const [, name, , description, supports] = match;

        const supportsModes = /** @type {('light'|'dark')[]} */ (
          supports.toLowerCase().includes("light") && supports.toLowerCase().includes("dark")
            ? ["light", "dark"]
            : supports.toLowerCase().includes("dark")
              ? ["dark"]
              : ["light"]
        );

        const preview = this.extractPreviewColors(cssContent);

        return {
          name: name.trim(),
          description: description.trim(),
          supports: supportsModes,
          preview,
        };
      }
    } catch (error) {
      logger.warn("Could not parse theme metadata:", error);
    }

    return null;
  }

  /** @param {string} cssContent @returns {{ focus: string; break: string; longBreak: string }} */
  extractPreviewColors(cssContent) {
    const colors = {
      focus: "#e74c3c",
      break: "#2ecc71",
      longBreak: "#3498db",
    };

    try {
      const focusMatch = cssContent.match(/--focus-color:\s*([^;]+);/);
      const breakMatch = cssContent.match(/--break-color:\s*([^;]+);/);
      const longBreakMatch = cssContent.match(/--long-break-color:\s*([^;]+);/);

      if (focusMatch) {
        colors.focus = focusMatch[1].trim();
      }
      if (breakMatch) {
        colors.break = breakMatch[1].trim();
      }
      if (longBreakMatch) {
        colors.longBreak = longBreakMatch[1].trim();
      }
    } catch (error) {
      logger.warn("Could not extract preview colors:", error);
    }

    return colors;
  }

  /** @param {string} str @returns {string} */
  capitalizeFirst(str) {
    return str.charAt(0).toUpperCase() + str.slice(1);
  }

  /** @param {string} themeId */
  unloadTheme(themeId) {
    const linkElement = this.themeStyles.get(themeId);
    if (linkElement) {
      document.head.removeChild(linkElement);
      this.loadedThemes.delete(themeId);
      this.themeStyles.delete(themeId);
      logger.info(`🗑️ Unloaded theme: ${themeId}`);
    }
  }

  getLoadedThemes() {
    return Array.from(this.loadedThemes);
  }

  /** @param {string} themeId @returns {boolean} */
  isThemeLoaded(themeId) {
    return this.loadedThemes.has(themeId);
  }
}

export const themeLoader = new ThemeLoader();

export async function initializeAutoThemeLoader() {
  logger.debug("🎨 Initializing auto theme loader...");
  const loadedThemes = await themeLoader.loadAllThemes();
  return loadedThemes;
}

export default themeLoader;
