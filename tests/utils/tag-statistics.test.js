import { TagStatistics } from "../../src/utils/tag-statistics.js";

const TAG1 = { id: "tag1", name: "Work", icon: "ri-work-line", color: "#3b82f6" };
const TAG2 = { id: "tag2", name: "Study", icon: "ri-book-line", color: "#10b981" };
const TAGS = [TAG1, TAG2];
const START = new Date(2026, 4, 1); // May 1, 2026
const END = new Date(2026, 4, 31, 23, 59, 59); // May 31, 2026 end of day

describe("TagStatistics.formatDuration", () => {
  let ts;

  beforeEach(() => {
    ts = new TagStatistics();
  });

  it("formats seconds under a minute", () => {
    expect(ts.formatDuration(30)).toBe("30s");
  });

  it("formats whole minutes", () => {
    expect(ts.formatDuration(90)).toBe("1m");
  });

  it("formats exact hours", () => {
    expect(ts.formatDuration(3600)).toBe("1h");
  });

  it("formats hours and remaining minutes", () => {
    expect(ts.formatDuration(5400)).toBe("1h 30m");
  });

  // Edge cases for boundary values.
  it("formats zero seconds", () => {
    expect(ts.formatDuration(0)).toBe("0s");
  });

  it("formats 59s as the last sub-minute value", () => {
    expect(ts.formatDuration(59)).toBe("59s");
  });

  it("formats 60s as the first whole minute", () => {
    expect(ts.formatDuration(60)).toBe("1m");
  });

  it("formats 59m as the last sub-hour value", () => {
    expect(ts.formatDuration(3540)).toBe("59m");
  });

  it("formats just-over-an-hour with a single trailing minute", () => {
    expect(ts.formatDuration(3660)).toBe("1h 1m");
  });
});

describe("TagStatistics.generatePieChartGradient", () => {
  let ts;

  beforeEach(() => {
    ts = new TagStatistics();
  });

  it("returns gray fallback for empty array", () => {
    expect(ts.generatePieChartGradient([])).toBe("conic-gradient(#e5e7eb 0deg 360deg)");
  });

  it("returns gray fallback for null input", () => {
    expect(ts.generatePieChartGradient(null)).toBe("conic-gradient(#e5e7eb 0deg 360deg)");
  });

  it("returns full-circle gradient for a single 100% stat", () => {
    const result = ts.generatePieChartGradient([{ percentage: 100, color: "#3b82f6" }]);
    expect(result).toContain("0deg 360deg");
  });

  it("emits matching angle stops for two equal slices", () => {
    const gradient = ts.generatePieChartGradient([
      { color: "#3b82f6", percentage: 50 },
      { color: "#10b981", percentage: 50 },
    ]);
    expect(gradient).toBe("conic-gradient(#3b82f6 0deg 180deg, #10b981 180deg 360deg)");
  });

  it("emits proportional angle stops for unequal slices", () => {
    const gradient = ts.generatePieChartGradient([
      { color: "#3b82f6", percentage: 25 },
      { color: "#10b981", percentage: 75 },
    ]);
    expect(gradient).toBe("conic-gradient(#3b82f6 0deg 90deg, #10b981 90deg 360deg)");
  });
});

describe("TagStatistics.getTagUsageStatistics", () => {
  let ts;

  beforeEach(() => {
    ts = new TagStatistics();
  });

  it("produces correct totals for two single-tag sessions", () => {
    const sessions = [
      { duration: 30, tags: [TAG1], date: "2026-05-06" },
      { duration: 30, tags: [TAG2], date: "2026-05-06" },
    ];
    const result = ts.getTagUsageStatistics(sessions, TAGS, START, END);
    // totalDuration = 60 minutes × 60 = 3600 seconds
    expect(result.totalDuration).toBe(3600);
    expect(result.stats).toHaveLength(2);
    const totalPct = result.stats.reduce((sum, s) => sum + s.percentage, 0);
    expect(totalPct).toBeCloseTo(100, 1);
  });

  it("places an untagged session in the untagged bucket", () => {
    const sessions = [{ duration: 30, tags: [], date: "2026-05-06" }];
    const result = ts.getTagUsageStatistics(sessions, TAGS, START, END);
    expect(result.stats).toHaveLength(1);
    expect(result.stats[0].tagId).toBe("untagged");
  });

  it("splits multi-tag session duration evenly between tags", () => {
    const sessions = [{ duration: 30, tags: [TAG1, TAG2], date: "2026-05-06" }];
    const result = ts.getTagUsageStatistics(sessions, TAGS, START, END);
    expect(result.stats).toHaveLength(2);
    // Each tag gets 15 minutes → 15 × 60 = 900 seconds
    const durations = result.stats.map((s) => s.duration).sort((a, b) => a - b);
    expect(durations).toEqual([900, 900]);
  });

  it("excludes sessions outside the date range", () => {
    const sessions = [
      { duration: 30, tags: [TAG1], date: "2026-04-30" }, // before range
      { duration: 30, tags: [TAG2], date: "2026-05-06" }, // in range
    ];
    const result = ts.getTagUsageStatistics(sessions, TAGS, START, END);
    expect(result.stats).toHaveLength(1);
    expect(result.stats[0].tagId).toBe("tag2");
  });

  it("returns empty stats for an empty session list", () => {
    const result = ts.getTagUsageStatistics([], TAGS, START, END);
    expect(result.stats).toHaveLength(0);
    expect(result.totalDuration).toBe(0);
    expect(result.totalSessions).toBe(0);
  });

  it("treats null tags as untagged", () => {
    const sessions = [
      { duration: 30, tags: null, date: "2026-05-06" },
      { duration: 30, tags: [], date: "2026-05-07" },
    ];
    const result = ts.getTagUsageStatistics(sessions, TAGS, START, END);
    expect(result.stats).toHaveLength(1);
    expect(result.stats[0].tagId).toBe("untagged");
    expect(result.stats[0].sessions).toBe(2);
  });

  it("ignores sessions with zero duration", () => {
    const sessions = [
      { duration: 0, tags: [TAG1], date: "2026-05-06" },
      { duration: 30, tags: [TAG1], date: "2026-05-07" },
    ];
    const result = ts.getTagUsageStatistics(sessions, TAGS, START, END);
    expect(result.stats).toHaveLength(1);
    expect(result.stats[0].sessions).toBe(1);
  });

  it("sorts stats by total duration descending", () => {
    const sessions = [
      { duration: 10, tags: [TAG1], date: "2026-05-06" },
      { duration: 30, tags: [TAG2], date: "2026-05-07" },
    ];
    const result = ts.getTagUsageStatistics(sessions, TAGS, START, END);
    expect(result.stats[0].tagId).toBe("tag2");
    expect(result.stats[1].tagId).toBe("tag1");
  });

  it("falls back to created_at when date is absent", () => {
    const sessions = [
      { duration: 30, tags: [TAG1], created_at: "2026-05-06T10:00:00Z" },
    ];
    const result = ts.getTagUsageStatistics(sessions, TAGS, START, END);
    expect(result.totalSessions).toBe(1);
  });

  it("assigns colors cyclically from the tagColors palette", () => {
    const sessions = [{ duration: 30, tags: [TAG1], date: "2026-05-06" }];
    const result = ts.getTagUsageStatistics(sessions, TAGS, START, END);
    expect(result.stats[0].color).toBe(ts.tagColors[0]);
  });
});
