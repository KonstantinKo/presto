import { defineConfig, devices } from "@playwright/test";

// Spec 001-leptos-migration Phase 6 (T241): post-cutover the JS toolchain at
// the repo root is gone. The e2e suite owns its own scope under tests/e2e/
// (own package.json + lockfile), and the dev server is now Trunk serving the
// Leptos crate at src/Trunk.toml — port 1420 matches the prior vite default.
//
// `command: "trunk serve"` runs from `cwd: "../../src"` (= <repo>/src), where
// Trunk.toml lives. Trunk's `[serve] port = 1420` keeps `baseURL` unchanged.

export default defineConfig({
  testDir: ".",
  snapshotPathTemplate: "{testDir}/__screenshots__/{arg}{-projectName}-{platform}{ext}",
  expect: {
    toHaveScreenshot: { maxDiffPixelRatio: 0.02, threshold: 0.2, animations: "disabled" },
  },
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: process.env.CI ? [["github"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: "http://127.0.0.1:1420",
    trace: "retain-on-failure",
    video: "retain-on-failure",
    screenshot: "only-on-failure",
    actionTimeout: 5000,
    navigationTimeout: 15000,
  },
  webServer: {
    // Trunk serves index.html + the wasm-bindgen glue + the Leptos WASM bundle.
    // The pre_build hook in src/Trunk.toml runs the theme generator before
    // each build. First-run cold start can take ~60–120s on a fresh checkout
    // (cargo build of presto-web + tools/build-themes) — the timeout below is
    // conservative enough to absorb that without flaking on hot starts.
    //
    // `--no-autoreload` disables Trunk's hot-reload WebSocket. The visual-
    // regression suite is sensitive to mid-run page reloads (each reload
    // re-emits the `tauri://update-available` mock event, un-dismissing the
    // banner and breaking downstream baselines). Tests don't depend on
    // hot-reload because Playwright waits for `webServer.url` to be reachable
    // before running — by which point Trunk has completed its initial build.
    command: "trunk serve --no-autoreload",
    cwd: "../../src",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
