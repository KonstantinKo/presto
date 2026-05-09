/**
 * Blocks all non-loopback HTTP requests so external CDNs (Supabase, Google Fonts,
 * jsDelivr, GitHub API) are never reached during tests.
 * @param {import('@playwright/test').Page} page
 */
export async function applyBlockExternal(page) {
  await page.route("**/*", (route) => {
    const url = new URL(route.request().url());
    const isLoopback =
      url.hostname === "127.0.0.1" ||
      url.hostname === "localhost" ||
      url.hostname === "::1" ||
      url.hostname === "0:0:0:0:0:0:0:1";
    const isSafeProtocol = url.protocol === "data:" || url.protocol === "blob:";
    if (isLoopback || isSafeProtocol) {
      return route.continue();
    }
    return route.abort();
  });
}
