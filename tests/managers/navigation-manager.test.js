import { vi, describe, it, expect, afterEach } from "vitest";
import { NavigationManager } from "../../src/managers/navigation-manager.js";

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
