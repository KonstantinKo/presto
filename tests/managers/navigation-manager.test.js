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

describe("NavigationManager – lazy XLSX loading", () => {
  it("XLSX global is absent at module load time", () => {
    expect(window.XLSX).toBeUndefined();
  });

  it("loads XLSX dynamically on first export call", async () => {
    // Intercept appendChild to simulate script load without hitting the network.
    const appendChildSpy = vi.spyOn(document.head, "appendChild").mockImplementation((el) => {
      if (el instanceof HTMLScriptElement && el.src.includes("xlsx")) {
        window.XLSX = MOCK_XLSX;
        el.onload();
      }
      return el;
    });

    // Suppress alert and provide Tauri dialog mock that confirms a file path.
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

    expect(window.XLSX).toBe(MOCK_XLSX);
    expect(appendChildSpy).toHaveBeenCalled();

    appendChildSpy.mockRestore();
    delete window.XLSX;
    delete window.sessionManager;
    delete window.alert;
  });
});
