# E2E Test Suite Ground Rules

## Rule 1: Pure UI — no programmatic state injection mid-flow

1.1 Every test navigates to the app exactly once via `page.goto('/index.html')` at the top of the test body. No subsequent `goto` calls.

1.2 Tests drive the app exclusively through UI interactions (clicks, keyboard input, form fills). The only exception: `page.addInitScript` calls in *fixtures* (not test bodies) may seed initial state (e.g. tags, session data, feature flags) before the initial navigation. Mid-flow `page.evaluate` calls that manipulate page state are forbidden.

1.3 Tests never use `page.waitForTimeout()`. All waiting uses Playwright's built-in `expect(...).toBeVisible()`, `expect(...).toHaveText()`, `page.waitForSelector()`, etc. with explicit timeouts when needed.

1.4 All assertions target visible UI state: text content, element visibility, CSS classes, attribute values. Never assert on in-memory store contents directly.

## Rule 2: No external network traffic

All non-loopback HTTP requests are blocked by the `_blockExternal` auto-fixture. No test may override this to reach the internet. CDN scripts (Supabase, Google Fonts, jsDelivr), GitHub releases API, and analytics are all blocked.

## Rule 3: Every test is a multi-step user journey

Each spec file contains one `test()` that walks a complete user-visible flow from start to finish, with assertions at each meaningful step. No "arrange-act-assert once" unit test style in spec files.

---

## Dev-server choice

**Vite** is used to serve `src/` as a static HTTP server because `src/index.html` uses `<script type="module">` imports, which require a server that sets correct `text/javascript` MIME types and supports ES module resolution. A plain `file://` serve does not work.

**`tauri dev` was rejected** for E2E because it requires `tauri-driver` + WebKit2GTK + extra OS packages in CI, with no behavioral upside given that all Tauri commands are mocked at the JS bridge level. The production Tauri IPC contract is already tested by Phase 3 cargo MockRuntime tests.

**Stack-swap survivability** is the primary design constraint. This suite targets only user-visible behavior expressed through ARIA roles, accessible names, and stable element IDs that exist in `src/index.html`. Only `tests/e2e/fixtures/tauriMock.js` is implementation-coupled; it is annotated `// TODO(stack-swap):` and must be re-implemented against whatever bridge boundary the Leptos/WASM rewrite uses. Spec files contain no implementation-detail selectors and will survive the swap with at most selector-handle updates.
