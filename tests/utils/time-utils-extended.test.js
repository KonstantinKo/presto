import { TimeUtils } from "../../src/utils/common-utils.js";

describe("TimeUtils.formatDateRange", () => {
  it("includes the end year in the output", () => {
    const start = new Date(2026, 4, 4); // May 4
    const end = new Date(2026, 4, 10); // May 10
    expect(TimeUtils.formatDateRange(start, end)).toContain("2026");
  });

  it("includes both start and end month abbreviations", () => {
    const start = new Date(2026, 4, 4); // May 4
    const end = new Date(2026, 4, 10); // May 10
    const result = TimeUtils.formatDateRange(start, end);
    expect(result).toContain("May");
  });

  it("uses the end date's year when the range spans two years", () => {
    const start = new Date(2026, 11, 28); // Dec 28, 2026
    const end = new Date(2027, 0, 3); // Jan 3, 2027
    expect(TimeUtils.formatDateRange(start, end)).toContain("2027");
  });

  it("includes both start and end day numbers", () => {
    const start = new Date(2026, 4, 4);
    const end = new Date(2026, 4, 10);
    const result = TimeUtils.formatDateRange(start, end);
    expect(result).toContain("4");
    expect(result).toContain("10");
  });

  it("contains a separator between the two dates", () => {
    const start = new Date(2026, 4, 4);
    const end = new Date(2026, 4, 10);
    expect(TimeUtils.formatDateRange(start, end)).toContain("-");
  });
});
