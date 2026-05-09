import { vi, describe, it, expect, afterEach, beforeEach } from "vitest";
import { NavigationManager } from "../../src/managers/navigation-manager.js";
import { resetTauriMock } from "../setup/tauri-mock.js";

const MOCK_XLSX = {
  utils: {
    json_to_sheet: vi.fn(() => ({})),
    book_new: vi.fn(() => ({})),
    book_append_sheet: vi.fn(),
  },
  write: vi.fn(() => "base64data"),
  writeFile: vi.fn(),
};

vi.mock("xlsx", () => ({ default: MOCK_XLSX }));

describe("NavigationManager – lazy XLSX loading", () => {
  let originalTauri;
  let originalAlert;

  afterEach(() => {
    delete window.sessionManager;
    if (originalAlert !== undefined) {
      window.alert = originalAlert;
    }
    if (originalTauri !== undefined) {
      globalThis.__TAURI__ = originalTauri;
    } else {
      delete globalThis.__TAURI__;
    }
    vi.clearAllMocks();
  });

  it("XLSX global is absent at module load time", () => {
    expect(window.XLSX).toBeUndefined();
  });

  it("loads XLSX via dynamic import on first export call", async () => {
    originalAlert = window.alert;
    originalTauri = globalThis.__TAURI__;

    window.alert = vi.fn();
    globalThis.__TAURI__ = {
      ...globalThis.__TAURI__,
      dialog: {
        save: vi.fn(async () => "/tmp/presto-test.xlsx"),
      },
    };

    window.sessionManager = {
      getSessionsForDate: vi.fn(() => [
        {
          id: "s1",
          start_time: "09:00",
          end_time: "09:25",
          duration: 25,
          created_at: new Date().toISOString(),
        },
      ]),
    };

    const manager = new NavigationManager();
    manager.getSessionTags = vi.fn(async () => []);
    manager.selectedDate = null;
    manager.currentDate = new Date();

    await manager.exportSessionsToExcel();

    expect(MOCK_XLSX.utils.json_to_sheet).toHaveBeenCalled();
    expect(MOCK_XLSX.utils.book_new).toHaveBeenCalled();
  });
});

describe("NavigationManager – date arithmetic", () => {
  let manager;

  beforeEach(() => {
    manager = new NavigationManager();
  });

  afterEach(() => {
    document.body.classList.remove("timer-active");
    document.documentElement.classList.remove("timer-active");
  });

  describe("isSameDay", () => {
    it("returns true for the same calendar day", () => {
      expect(manager.isSameDay(new Date(2026, 4, 6, 9, 0), new Date(2026, 4, 6, 17, 30))).toBe(
        true
      );
    });

    it("returns false for different days", () => {
      expect(manager.isSameDay(new Date(2026, 4, 6), new Date(2026, 4, 7))).toBe(false);
    });

    it("returns true for different times on the same day", () => {
      expect(
        manager.isSameDay(new Date(2026, 4, 6, 0, 0, 0), new Date(2026, 4, 6, 23, 59, 59))
      ).toBe(true);
    });
  });

  describe("getWeekStart", () => {
    it("returns the Monday of the same week for a Wednesday", () => {
      // May 6, 2026 is a Wednesday
      const result = manager.getWeekStart(new Date(2026, 4, 6));
      expect(result.getFullYear()).toBe(2026);
      expect(result.getMonth()).toBe(4);
      expect(result.getDate()).toBe(4); // Monday May 4
    });

    it("returns the previous Monday for a Sunday (day === 0 branch)", () => {
      // May 10, 2026 is a Sunday
      const result = manager.getWeekStart(new Date(2026, 4, 10));
      expect(result.getFullYear()).toBe(2026);
      expect(result.getMonth()).toBe(4);
      expect(result.getDate()).toBe(4); // Monday May 4
    });
  });

  describe("calculatePercentageChange", () => {
    it("returns 0 when both values are 0", () => {
      expect(manager.calculatePercentageChange(0, 0)).toBe(0);
    });

    it("returns 100 when previous is 0 and current is positive", () => {
      expect(manager.calculatePercentageChange(5, 0)).toBe(100);
    });

    it("returns 50 for 100→150 growth", () => {
      expect(manager.calculatePercentageChange(150, 100)).toBe(50);
    });

    it("returns -50 for 100→50 decline", () => {
      expect(manager.calculatePercentageChange(50, 100)).toBe(-50);
    });

    it("returns 20 for 100→120 growth", () => {
      expect(manager.calculatePercentageChange(120, 100)).toBe(20);
    });
  });

  describe("isFocusOrCustomSession", () => {
    it("returns true for session_type 'focus'", () => {
      expect(manager.isFocusOrCustomSession({ session_type: "focus" })).toBe(true);
    });

    it("returns false for session_type 'break'", () => {
      expect(manager.isFocusOrCustomSession({ session_type: "break" })).toBe(false);
    });

    it("returns true for legacy type field 'custom'", () => {
      expect(manager.isFocusOrCustomSession({ type: "custom" })).toBe(true);
    });

    it("returns false for an empty session object", () => {
      expect(manager.isFocusOrCustomSession({})).toBe(false);
    });
  });
});

describe("NavigationManager – chart data shaping (computeFocusSummary)", () => {
  afterEach(() => {
    delete window.sessionManager;
    resetTauriMock();
    document.body.classList.remove("timer-active");
    document.documentElement.classList.remove("timer-active");
  });

  it("sums focus session durations for the week (happy path)", async () => {
    const monday = new Date(2026, 4, 4); // May 4, 2026 (Monday)
    const tuesday = new Date(2026, 4, 5);

    window.sessionManager = {
      getSessionsForDate: vi.fn((date) => {
        if (date.toDateString() === monday.toDateString()) {
          return [{ id: "s1", session_type: "focus", duration: 25 }];
        }
        if (date.toDateString() === tuesday.toDateString()) {
          return [{ id: "s2", session_type: "focus", duration: 50 }];
        }
        return [];
      }),
    };

    const m = new NavigationManager();
    m.currentDate = monday;
    const result = await m.computeFocusSummary(m.getWeekStart(m.currentDate));

    expect(result.current.totalTime).toBe((25 + 50) * 60);
    expect(result.current.sessions).toBe(2);
    expect(result.current.avgFocus).toBe(((25 + 50) * 60) / 2);
  });

  it("returns zeros when sessionManager.getSessionsForDate throws (failure path)", async () => {
    window.sessionManager = {
      getSessionsForDate: () => {
        throw new Error("read failed");
      },
    };

    const m = new NavigationManager();
    m.currentDate = new Date(2026, 4, 4);
    const result = await m.computeFocusSummary(m.getWeekStart(m.currentDate));

    expect(result.current).toEqual({ totalTime: 0, sessions: 0, avgFocus: 0 });
    expect(result.previous).toEqual({ totalTime: 0, sessions: 0, avgFocus: 0 });
  });

  it("returns zeros for an empty week (edge case)", async () => {
    window.sessionManager = {
      getSessionsForDate: vi.fn(() => []),
    };

    const m = new NavigationManager();
    m.currentDate = new Date(2026, 4, 4);
    const result = await m.computeFocusSummary(m.getWeekStart(m.currentDate));

    expect(result.current.totalTime).toBe(0);
    expect(result.current.avgFocus).toBe(0);
  });
});
