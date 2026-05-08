import { TimeUtils, KeyboardUtils } from "../../src/utils/common-utils.js";

describe("TimeUtils.formatTime", () => {
  it("returns '0m' for 0", () => {
    expect(TimeUtils.formatTime(0)).toBe("0m");
  });

  it("returns '0m' for negative values", () => {
    expect(TimeUtils.formatTime(-1)).toBe("0m");
  });

  it("returns '0m' for null", () => {
    expect(TimeUtils.formatTime(null)).toBe("0m");
  });

  it("returns seconds when under a minute", () => {
    expect(TimeUtils.formatTime(45)).toBe("45s");
  });

  it("returns minutes for exact minute count", () => {
    expect(TimeUtils.formatTime(300)).toBe("5m");
  });

  it("returns hours and minutes", () => {
    expect(TimeUtils.formatTime(5400)).toBe("1h 30m");
  });

  it("returns exact hours without minutes", () => {
    expect(TimeUtils.formatTime(7200)).toBe("2h");
  });
});

describe("TimeUtils.formatTimeDetailed", () => {
  it("returns '0h 0m' for 0", () => {
    expect(TimeUtils.formatTimeDetailed(0)).toBe("0h 0m");
  });

  it("returns '0h 0m' for null", () => {
    expect(TimeUtils.formatTimeDetailed(null)).toBe("0h 0m");
  });

  it("returns hours and minutes", () => {
    expect(TimeUtils.formatTimeDetailed(5400)).toBe("1h 30m");
  });
});

describe("TimeUtils.getWeekStart", () => {
  it("returns Monday for a Wednesday input", () => {
    // May 6, 2026 is a Wednesday
    const wednesday = new Date(2026, 4, 6);
    const start = TimeUtils.getWeekStart(wednesday);
    expect(start.getFullYear()).toBe(2026);
    expect(start.getMonth()).toBe(4); // May (0-indexed)
    expect(start.getDate()).toBe(4); // Monday May 4
  });

  it("returns previous Monday for a Sunday (day === 0 branch)", () => {
    // May 10, 2026 is a Sunday
    const sunday = new Date(2026, 4, 10);
    const start = TimeUtils.getWeekStart(sunday);
    expect(start.getFullYear()).toBe(2026);
    expect(start.getMonth()).toBe(4);
    expect(start.getDate()).toBe(4); // Monday May 4
  });
});

describe("TimeUtils.isSameDay", () => {
  it("returns true for two times on the same day", () => {
    const morning = new Date(2026, 4, 6, 9, 0);
    const evening = new Date(2026, 4, 6, 21, 0);
    expect(TimeUtils.isSameDay(morning, evening)).toBe(true);
  });

  it("returns false for different days", () => {
    const d1 = new Date(2026, 4, 6);
    const d2 = new Date(2026, 4, 7);
    expect(TimeUtils.isSameDay(d1, d2)).toBe(false);
  });
});

describe("KeyboardUtils.parseShortcut", () => {
  it("returns null for null input", () => {
    expect(KeyboardUtils.parseShortcut(null)).toBeNull();
  });

  it("returns null for empty string", () => {
    expect(KeyboardUtils.parseShortcut("")).toBeNull();
  });

  it("returns null for undefined", () => {
    expect(KeyboardUtils.parseShortcut(undefined)).toBeNull();
  });

  it("parses CommandOrControl+Alt+Space correctly", () => {
    const result = KeyboardUtils.parseShortcut("CommandOrControl+Alt+Space");
    expect(result).toEqual({ meta: true, ctrl: true, alt: true, shift: false, key: " " });
  });

  it("parses Shift+R correctly", () => {
    const result = KeyboardUtils.parseShortcut("Shift+R");
    expect(result).toEqual({ meta: false, ctrl: false, alt: false, shift: true, key: "r" });
  });
});

describe("KeyboardUtils.matchesShortcut", () => {
  it("matches Space with meta and alt modifiers", () => {
    const event = {
      key: " ",
      code: "Space",
      metaKey: true,
      ctrlKey: false,
      altKey: true,
      shiftKey: false,
    };
    expect(KeyboardUtils.matchesShortcut(event, "CommandOrControl+Alt+Space")).toBe(true);
  });

  it("does not match when a required modifier is missing", () => {
    const event = {
      key: " ",
      code: "Space",
      metaKey: true,
      ctrlKey: false,
      altKey: false, // missing alt
      shiftKey: false,
    };
    expect(KeyboardUtils.matchesShortcut(event, "CommandOrControl+Alt+Space")).toBe(false);
  });
});
