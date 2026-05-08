import { describe, it, expect, beforeEach, vi } from "vitest";

vi.mock("@tauri-apps/plugin-log", () => ({
  debug: vi.fn().mockResolvedValue(undefined),
  info: vi.fn().mockResolvedValue(undefined),
  warn: vi.fn().mockResolvedValue(undefined),
  error: vi.fn().mockResolvedValue(undefined),
}));

const { TagStatistics } = await import("../utils/tag-statistics.js");

describe("TagStatistics", () => {
  let stats;

  beforeEach(() => {
    stats = new TagStatistics();
  });

  describe("formatDuration", () => {
    it("formats sub-minute durations as seconds", () => {
      expect(stats.formatDuration(0)).toBe("0s");
      expect(stats.formatDuration(30)).toBe("30s");
      expect(stats.formatDuration(59)).toBe("59s");
    });

    it("formats minute-level durations as minutes", () => {
      expect(stats.formatDuration(60)).toBe("1m");
      expect(stats.formatDuration(90)).toBe("1m");
      expect(stats.formatDuration(3540)).toBe("59m");
    });

    it("formats exact hour durations without minutes", () => {
      expect(stats.formatDuration(3600)).toBe("1h");
      expect(stats.formatDuration(7200)).toBe("2h");
    });

    it("formats hours with remaining minutes", () => {
      expect(stats.formatDuration(3660)).toBe("1h 1m");
      expect(stats.formatDuration(5400)).toBe("1h 30m");
      expect(stats.formatDuration(7260)).toBe("2h 1m");
    });
  });

  describe("generatePieChartGradient", () => {
    it("returns gray gradient for empty stats", () => {
      expect(stats.generatePieChartGradient([])).toBe(
        "conic-gradient(#e5e7eb 0deg 360deg)",
      );
      expect(stats.generatePieChartGradient(null)).toBe(
        "conic-gradient(#e5e7eb 0deg 360deg)",
      );
    });

    it("generates a full-circle gradient for a single 100% tag", () => {
      const tagStats = [{ color: "#3b82f6", percentage: 100 }];
      const gradient = stats.generatePieChartGradient(tagStats);
      expect(gradient).toBe("conic-gradient(#3b82f6 0deg 360deg)");
    });

    it("generates correct angle stops for multiple tags", () => {
      const tagStats = [
        { color: "#3b82f6", percentage: 50 },
        { color: "#10b981", percentage: 50 },
      ];
      const gradient = stats.generatePieChartGradient(tagStats);
      expect(gradient).toBe(
        "conic-gradient(#3b82f6 0deg 180deg, #10b981 180deg 360deg)",
      );
    });

    it("generates correct gradient for unequal percentages", () => {
      const tagStats = [
        { color: "#3b82f6", percentage: 25 },
        { color: "#10b981", percentage: 75 },
      ];
      const gradient = stats.generatePieChartGradient(tagStats);
      expect(gradient).toBe(
        "conic-gradient(#3b82f6 0deg 90deg, #10b981 90deg 360deg)",
      );
    });
  });

  describe("getTagUsageStatistics", () => {
    const tag1 = { id: "tag-1", name: "Work", icon: "ri-briefcase-line", color: "#3b82f6" };
    const tag2 = { id: "tag-2", name: "Study", icon: "ri-book-line", color: "#10b981" };
    const startDate = new Date("2024-01-01T00:00:00Z");
    const endDate = new Date("2024-01-31T23:59:59Z");

    it("returns empty stats for empty sessions", () => {
      const result = stats.getTagUsageStatistics([], [tag1], startDate, endDate);
      expect(result.stats).toHaveLength(0);
      expect(result.totalDuration).toBe(0);
      expect(result.totalSessions).toBe(0);
    });

    it("assigns sessions without tags to Untagged category", () => {
      const sessions = [
        { date: "2024-01-15T10:00:00Z", duration: 25, tags: [] },
        { date: "2024-01-16T10:00:00Z", duration: 25, tags: null },
      ];
      const result = stats.getTagUsageStatistics(sessions, [], startDate, endDate);
      expect(result.stats).toHaveLength(1);
      expect(result.stats[0].tagId).toBe("untagged");
      expect(result.stats[0].tag.name).toBe("Untagged");
      expect(result.stats[0].sessions).toBe(2);
      expect(result.totalDuration).toBe(Math.round(50 * 60));
    });

    it("calculates 100% for a single tag", () => {
      const sessions = [
        { date: "2024-01-15T10:00:00Z", duration: 25, tags: ["tag-1"] },
      ];
      const result = stats.getTagUsageStatistics(sessions, [tag1], startDate, endDate);
      expect(result.stats).toHaveLength(1);
      expect(result.stats[0].tagId).toBe("tag-1");
      expect(result.stats[0].percentage).toBe(100);
    });

    it("splits duration equally among multiple tags on a session", () => {
      const sessions = [
        { date: "2024-01-15T10:00:00Z", duration: 20, tags: ["tag-1", "tag-2"] },
      ];
      const result = stats.getTagUsageStatistics(sessions, [tag1, tag2], startDate, endDate);
      const t1 = result.stats.find((s) => s.tagId === "tag-1");
      const t2 = result.stats.find((s) => s.tagId === "tag-2");
      expect(t1.percentage).toBe(50);
      expect(t2.percentage).toBe(50);
      expect(t1.duration).toBe(Math.round(10 * 60));
    });

    it("filters out sessions outside the date range", () => {
      const sessions = [
        { date: "2024-01-15T10:00:00Z", duration: 25, tags: ["tag-1"] },
        { date: "2024-02-15T10:00:00Z", duration: 25, tags: ["tag-1"] },
      ];
      const result = stats.getTagUsageStatistics(sessions, [tag1], startDate, endDate);
      expect(result.totalSessions).toBe(1);
    });

    it("ignores sessions with zero duration", () => {
      const sessions = [
        { date: "2024-01-15T10:00:00Z", duration: 0, tags: ["tag-1"] },
        { date: "2024-01-16T10:00:00Z", duration: 25, tags: ["tag-1"] },
      ];
      const result = stats.getTagUsageStatistics(sessions, [tag1], startDate, endDate);
      expect(result.stats).toHaveLength(1);
      expect(result.stats[0].sessions).toBe(1);
    });

    it("sorts stats by duration descending", () => {
      const sessions = [
        { date: "2024-01-15T10:00:00Z", duration: 10, tags: ["tag-1"] },
        { date: "2024-01-16T10:00:00Z", duration: 30, tags: ["tag-2"] },
      ];
      const result = stats.getTagUsageStatistics(sessions, [tag1, tag2], startDate, endDate);
      expect(result.stats[0].tagId).toBe("tag-2");
      expect(result.stats[1].tagId).toBe("tag-1");
    });

    it("handles tag objects directly on the session (not just tag IDs)", () => {
      const sessions = [
        {
          date: "2024-01-15T10:00:00Z",
          duration: 25,
          tags: [{ id: "tag-1", name: "Work", icon: "ri-briefcase-line", color: "#3b82f6" }],
        },
      ];
      const result = stats.getTagUsageStatistics(sessions, [tag1], startDate, endDate);
      expect(result.stats).toHaveLength(1);
      expect(result.stats[0].tag.name).toBe("Work");
    });

    it("uses created_at when date field is absent", () => {
      const sessions = [
        { created_at: "2024-01-15T10:00:00Z", duration: 25, tags: ["tag-1"] },
      ];
      const result = stats.getTagUsageStatistics(sessions, [tag1], startDate, endDate);
      expect(result.totalSessions).toBe(1);
    });

    it("assigns colors cyclically from the tagColors palette", () => {
      const result = stats.getTagUsageStatistics(
        [{ date: "2024-01-15T10:00:00Z", duration: 25, tags: ["tag-1"] }],
        [tag1],
        startDate,
        endDate,
      );
      expect(result.stats[0].color).toBe(stats.tagColors[0]);
    });
  });
});
