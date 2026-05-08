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
});

describe("TagStatistics.generatePieChartGradient", () => {
  let ts;

  beforeEach(() => {
    ts = new TagStatistics();
  });

  it("returns gray fallback for empty array", () => {
    expect(ts.generatePieChartGradient([])).toBe("conic-gradient(#e5e7eb 0deg 360deg)");
  });

  it("returns full-circle gradient for a single 100% stat", () => {
    const result = ts.generatePieChartGradient([{ percentage: 100, color: "#3b82f6" }]);
    expect(result).toContain("0deg 360deg");
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
});
