import { describe, it, expect, afterEach } from "vitest";

const {
  getAllThemes,
  getThemeById,
  getCompatibleThemes,
  isThemeCompatible,
  getDefaultTheme,
  registerTheme,
  unregisterTheme,
} = await import("../utils/timer-themes.js");

describe("timer-themes", () => {
  afterEach(() => {
    unregisterTheme("test-theme");
    unregisterTheme("another-theme");
  });

  describe("getThemeById", () => {
    it("returns the correct theme for a known ID", () => {
      const theme = getThemeById("espresso");
      expect(theme.name).toBe("Espresso");
    });

    it("falls back to espresso for an unknown ID", () => {
      const theme = getThemeById("nonexistent-theme");
      expect(theme.name).toBe("Espresso");
    });

    it("returns pipboy theme by ID", () => {
      const theme = getThemeById("pipboy");
      expect(theme.name).toBe("PipBoy");
    });
  });

  describe("getAllThemes", () => {
    it("returns an array with id property on each theme", () => {
      const themes = getAllThemes();
      expect(Array.isArray(themes)).toBe(true);
      themes.forEach((theme) => {
        expect(typeof theme.id).toBe("string");
        expect(typeof theme.name).toBe("string");
        expect(Array.isArray(theme.supports)).toBe(true);
      });
    });

    it("includes the three built-in themes", () => {
      const themes = getAllThemes();
      const ids = themes.map((t) => t.id);
      expect(ids).toContain("espresso");
      expect(ids).toContain("pommodore64");
      expect(ids).toContain("pipboy");
    });
  });

  describe("getCompatibleThemes", () => {
    it("returns themes that support light mode", () => {
      const themes = getCompatibleThemes("light");
      const ids = themes.map((t) => t.id);
      expect(ids).toContain("espresso");
      expect(ids).toContain("pommodore64");
      expect(ids).not.toContain("pipboy");
    });

    it("returns themes that support dark mode", () => {
      const themes = getCompatibleThemes("dark");
      const ids = themes.map((t) => t.id);
      expect(ids).toContain("espresso");
      expect(ids).toContain("pipboy");
      expect(ids).not.toContain("pommodore64");
    });

    it("defaults to light mode when no argument given", () => {
      const themes = getCompatibleThemes();
      expect(themes.length).toBeGreaterThan(0);
      themes.forEach((theme) => {
        expect(theme.supports).toContain("light");
      });
    });
  });

  describe("isThemeCompatible", () => {
    it("returns true for a compatible theme/mode pair", () => {
      expect(isThemeCompatible("espresso", "light")).toBe(true);
      expect(isThemeCompatible("espresso", "dark")).toBe(true);
      expect(isThemeCompatible("pipboy", "dark")).toBe(true);
      expect(isThemeCompatible("pommodore64", "light")).toBe(true);
    });

    it("returns false for an incompatible theme/mode pair", () => {
      expect(isThemeCompatible("pipboy", "light")).toBe(false);
      expect(isThemeCompatible("pommodore64", "dark")).toBe(false);
    });

    it("falls back to espresso for unknown theme IDs", () => {
      expect(isThemeCompatible("nonexistent", "light")).toBe(true);
    });
  });

  describe("getDefaultTheme", () => {
    it("returns espresso as the default theme", () => {
      const theme = getDefaultTheme();
      expect(theme.id).toBe("espresso");
      expect(theme.isDefault).toBe(true);
    });
  });

  describe("registerTheme", () => {
    it("registers a new theme and makes it retrievable", () => {
      const config = {
        name: "Test Theme",
        description: "A test theme",
        supports: ["light"],
        isDefault: false,
        preview: { focus: "#ff0000", break: "#00ff00", longBreak: "#0000ff" },
      };
      registerTheme("test-theme", config);
      const theme = getThemeById("test-theme");
      expect(theme.name).toBe("Test Theme");
    });

    it("returns the registered theme config", () => {
      const config = { name: "Test Theme", supports: ["dark"], isDefault: false };
      const result = registerTheme("test-theme", config);
      expect(result.name).toBe("Test Theme");
      expect(result.supports).toEqual(["dark"]);
    });

    it("applies defaults for missing config fields", () => {
      registerTheme("test-theme", {});
      const theme = getThemeById("test-theme");
      expect(theme.name).toBe("test-theme");
      expect(theme.supports).toEqual(["light", "dark"]);
      expect(theme.isDefault).toBe(false);
    });

    it("overrides an existing non-default theme", () => {
      registerTheme("test-theme", { name: "First" });
      registerTheme("test-theme", { name: "Second" });
      const theme = getThemeById("test-theme");
      expect(theme.name).toBe("Second");
    });

    it("includes the new theme in getAllThemes", () => {
      registerTheme("test-theme", { name: "Test Theme" });
      const ids = getAllThemes().map((t) => t.id);
      expect(ids).toContain("test-theme");
    });
  });

  describe("unregisterTheme", () => {
    it("removes a registered non-default theme and returns true", () => {
      registerTheme("test-theme", { name: "Test" });
      const result = unregisterTheme("test-theme");
      expect(result).toBe(true);
      const theme = getThemeById("test-theme");
      expect(theme.name).toBe("Espresso");
    });

    it("refuses to remove default themes and returns false", () => {
      const result = unregisterTheme("espresso");
      expect(result).toBe(false);
      const theme = getThemeById("espresso");
      expect(theme.name).toBe("Espresso");
    });

    it("returns false for non-existent theme IDs", () => {
      const result = unregisterTheme("does-not-exist");
      expect(result).toBe(false);
    });
  });
});
