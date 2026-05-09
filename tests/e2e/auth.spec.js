import { test, expect } from "./fixtures/index.js";
import { gotoTimer } from "./fixtures/screens.js";

test("auth flow: sign in via email form, user name updates, sign out returns to guest", async ({
  page,
}) => {
  // Supabase is already mocked by tauriMock fixture; guest mode is set in localStorage
  await gotoTimer(page);

  // Open user dropdown
  await page.locator("#user-avatar-btn").click();
  await expect(page.locator("#user-dropdown")).toBeVisible({ timeout: 3000 });

  // In guest mode, Sign In button should be visible
  await expect(page.locator("#user-sign-in")).toBeVisible();

  // Click Sign In to open the auth overlay (created dynamically by main.js)
  await page.locator("#user-sign-in").click();
  await expect(page.locator("#auth-overlay")).toBeVisible({ timeout: 3000 });

  // Fill in email and password
  await page.locator("#email").fill("test@example.com");
  await page.locator("#password").fill("test-password");

  // Submit the form via the Login button
  await page.locator("#auth-form").getByRole("button", { name: /login/i }).click();

  // The Supabase mock returns a session with name "Test User"
  await expect(page.locator("#user-name")).toHaveText("Test User", { timeout: 5000 });
  await expect(page.locator("#auth-overlay")).toBeHidden({ timeout: 3000 });

  // Sign out
  await page.locator("#user-avatar-btn").click();
  await expect(page.locator("#user-dropdown")).toBeVisible();
  await page.locator("#user-sign-out").click();

  // After sign out, user name returns to Guest
  await expect(page.locator("#user-name")).toHaveText("Guest", { timeout: 5000 });
});
