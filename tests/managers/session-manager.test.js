import { SessionManager } from "../../src/managers/session-manager.js";

// Minimal DOM satisfying SessionManager.setupEventListeners() and openAddSessionModal().
const SESSION_DOM = `
  <button id="add-session-btn"></button>
  <div id="session-modal-overlay">
    <h2 id="session-modal-title"></h2>
    <button id="close-session-modal"></button>
    <button id="cancel-session-btn"></button>
    <form id="session-form">
      <input id="session-start-time" name="startTime" type="time" />
      <input id="session-end-time"   name="endTime"   type="time" />
      <input id="session-duration"   name="duration"  type="number" />
      <button id="save-session-btn" type="submit"></button>
      <button id="delete-session-btn" type="button"></button>
    </form>
  </div>
`;

function makeManager() {
  document.body.innerHTML = SESSION_DOM;
  const manager = new SessionManager(null);
  // Override sessions immediately so async backend result doesn't matter.
  manager.sessions = {};
  return manager;
}

describe("SessionManager.timeToMinutes", () => {
  let manager;

  beforeEach(() => {
    manager = makeManager();
  });

  it("converts midnight to 0 minutes", () => {
    expect(manager.timeToMinutes("00:00")).toBe(0);
  });

  it("converts noon to 720 minutes", () => {
    expect(manager.timeToMinutes("12:00")).toBe(720);
  });

  it("converts 09:30 to 570 minutes", () => {
    expect(manager.timeToMinutes("09:30")).toBe(570);
  });

  it("converts the last minute of the day correctly", () => {
    expect(manager.timeToMinutes("23:59")).toBe(23 * 60 + 59);
  });
});

describe("SessionManager.minutesToTime", () => {
  let manager;

  beforeEach(() => {
    manager = makeManager();
  });

  it("converts 0 minutes to 00:00", () => {
    expect(manager.minutesToTime(0)).toBe("00:00");
  });

  it("converts 720 minutes to 12:00", () => {
    expect(manager.minutesToTime(720)).toBe("12:00");
  });

  it("pads single-digit hours and minutes with leading zero", () => {
    expect(manager.minutesToTime(65)).toBe("01:05");
  });

  it("wraps around at exactly 24 hours", () => {
    expect(manager.minutesToTime(24 * 60)).toBe("00:00");
  });

  it("round-trips through timeToMinutes", () => {
    expect(manager.minutesToTime(manager.timeToMinutes("14:37"))).toBe("14:37");
  });
});

describe("SessionManager.calculateEndTime", () => {
  let manager;

  beforeEach(() => {
    manager = makeManager();
  });

  it("adds duration minutes to the start time", () => {
    expect(manager.calculateEndTime("09:00", 25)).toBe("09:25");
  });

  it("correctly crosses an hour boundary", () => {
    expect(manager.calculateEndTime("09:45", 25)).toBe("10:10");
  });

  it("caps at 23:59 when duration would push past midnight", () => {
    expect(manager.calculateEndTime("23:45", 30)).toBe("23:59");
  });

  it("returns the start time unchanged for zero duration", () => {
    expect(manager.calculateEndTime("10:00", 0)).toBe("10:00");
  });

  it("returns 23:59 for a start time exactly at 23:59 with positive duration", () => {
    expect(manager.calculateEndTime("23:59", 1)).toBe("23:59");
  });
});

describe("SessionManager.getSessionsForDate", () => {
  let manager;

  beforeEach(() => {
    manager = makeManager();
  });

  it("returns an empty array for a date with no sessions", () => {
    const date = new Date(2026, 4, 6);
    expect(manager.getSessionsForDate(date)).toEqual([]);
  });

  it("returns sessions stored under the matching date string", () => {
    const date = new Date(2026, 4, 6);
    const session = { id: "s1", duration: 25 };
    manager.sessions[date.toDateString()] = [session];
    expect(manager.getSessionsForDate(date)).toEqual([session]);
  });

  it("does not return sessions from a different day", () => {
    const dateA = new Date(2026, 4, 6);
    const dateB = new Date(2026, 4, 7);
    manager.sessions[dateA.toDateString()] = [{ id: "s1" }];
    expect(manager.getSessionsForDate(dateB)).toEqual([]);
  });
});

describe("SessionManager.generateSessionId", () => {
  let manager;

  beforeEach(() => {
    manager = makeManager();
  });

  it("returns a non-empty string", () => {
    expect(typeof manager.generateSessionId()).toBe("string");
    expect(manager.generateSessionId().length).toBeGreaterThan(0);
  });

  it("produces a unique ID on every call", () => {
    const ids = new Set(Array.from({ length: 10 }, () => manager.generateSessionId()));
    expect(ids.size).toBe(10);
  });
});

describe("SessionManager.isModalOpen", () => {
  let manager;

  beforeEach(() => {
    manager = makeManager();
  });

  it("returns false when the modal overlay lacks the show class", () => {
    expect(manager.isModalOpen()).toBeFalsy();
  });

  it("returns true when the modal overlay has the show class", () => {
    document.getElementById("session-modal-overlay").classList.add("show");
    expect(manager.isModalOpen()).toBeTruthy();
  });
});

describe("SessionManager.closeModal", () => {
  let manager;

  beforeEach(() => {
    manager = makeManager();
  });

  it("removes the show class from the modal overlay", () => {
    document.getElementById("session-modal-overlay").classList.add("show");
    manager.closeModal();
    expect(document.getElementById("session-modal-overlay").classList.contains("show")).toBe(false);
  });

  it("resets currentEditingSession to null", () => {
    manager.currentEditingSession = { id: "s1" };
    manager.closeModal();
    expect(manager.currentEditingSession).toBeNull();
  });

  it("resets selectedDate to null", () => {
    manager.selectedDate = new Date();
    manager.closeModal();
    expect(manager.selectedDate).toBeNull();
  });
});
