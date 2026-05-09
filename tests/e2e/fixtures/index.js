import { test as base, expect } from "@playwright/test";
import { applyBlockExternal } from "./blockExternal.js";
import { applyTauriMock } from "./tauriMock.js";

export const test = base.extend({
  // Blocks all non-loopback HTTP for every test automatically
  _blockExternal: [
    async ({ page }, use) => {
      await applyBlockExternal(page);
      await use();
    },
    { auto: true },
  ],

  // Installs the Tauri bridge mock and returns a pre-navigation harness object
  tauriMock: [
    async ({ page }, use) => {
      const harness = await applyTauriMock(page);
      await use(harness);
    },
    { auto: true },
  ],
});

export { expect };
