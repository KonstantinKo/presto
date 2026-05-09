# Quickstart: Leptos frontend (post-migration)

**Audience**: a new contributor (or returning developer) cloning the post-cutover branch for the first time.
**Time budget**: ~10 minutes from clone to running app.

This guide assumes a Unix-like host (Linux/macOS). Windows works via WSL2.

---

## 1. Prerequisites

### Rust toolchain

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
rustup target add wasm32-unknown-unknown
```

Required: Rust 1.83 or newer (workspace `edition = "2021"`).

### Trunk

```bash
cargo install trunk --locked
```

Trunk is the WASM bundler + dev server for the Leptos crate.

### Node.js (e2e scope only)

```bash
# Use your favourite installer; v20 LTS or newer
node --version  # v20.x or later
npm --version   # 10.x or later
```

Node is needed **only** for `tests/e2e/`. The Leptos build chain is Rust-only.

### Tauri prerequisites

Per `https://v2.tauri.app/start/prerequisites/`. On Linux:

```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

On macOS: Xcode Command Line Tools (`xcode-select --install`).

---

## 2. Clone & build

```bash
git clone <repo-url>
cd presto

# Build everything in the workspace, frozen against Cargo.lock
cargo build --workspace --frozen

# Build the Leptos WASM bundle in development mode
(cd src && trunk build)
```

The first build downloads all crates; expect 3–5 minutes on a fresh clone, faster on incrementals.

---

## 3. Run the app

### Option A — Tauri dev (the full app)

```bash
# From the repo root
cargo tauri dev
```

This launches the Tauri host with the Leptos WebView. Hot-reload of Rust code triggers a rebuild + restart; hot-reload of CSS / HTML / WASM source triggers Trunk's incremental rebuild.

### Option B — Trunk-only (browser dev, no Tauri host)

```bash
(cd src && trunk serve)
# Browse to http://127.0.0.1:1420
```

Useful for fast UI iteration. The Tauri bridge is unavailable in this mode — every `bridge::commands::*` call short-circuits via the `BridgeAvailable::Absent` path, returning sentinel values. Theme/UI changes render immediately.

---

## 4. Run the tests

### Rust unit tests (pure logic)

```bash
cargo test --workspace --frozen
```

Covers the timer engine (`src/src/engine/`), manager state machines (`src/src/managers/`), persistence helpers, the theme code-gen binary (`tools/build-themes/`), and the existing `src-tauri/` tests.

### WASM tests (DOM-coupled Leptos modules)

```bash
(cd src && wasm-pack test --headless --chrome)
```

Or, equivalently, Trunk's test runner if configured. Covers `bridge/availability`, `bridge/events`, and a few `managers/*` paths that touch `web-sys`.

### Lint + format

```bash
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic
cargo fmt --check
```

The Leptos crate matches the backend's pedantic posture per Principle III. `#[allow(...)]` attributes must carry an inline justification comment.

### E2E + visual regression

```bash
cd tests/e2e
npm ci
npx playwright install --with-deps chromium  # first-time only
npx playwright test                            # all 17 specs
npx playwright test visual-regression.spec.js  # visual regression suite alone
```

The e2e suite runs against a Trunk dev server (or a Vite-served fixture pre-cutover) at `127.0.0.1:1420`. The visual regression suite is the green-light gate per Principle IV — it must pass against the 14 baselines in `tests/e2e/__screenshots__/visual-regression/` within 2% tolerance.

If a visual diff fails:
- **Unintended regression**: fix the code; re-run.
- **Intended visual change**: regenerate the affected baseline(s) only via `npx playwright test --update-snapshots <spec>`, visually review the new PNG(s), and add a one-line PR note explaining the change. **Do not** re-capture all 14 — that defeats the gate.

---

## 5. Make a change

### Add a Tauri command

Per FR-010 and Principle VI, the order is:

1. Extend `tests/e2e/fixtures/tauriMock.js` with a `case "<command_name>":` entry. Commit.
2. Add a failing `wasm-bindgen-test` exercising the bridge wrapper. Commit (RED).
3. Add the Rust handler in `src-tauri/src/lib.rs` and the typed Leptos wrapper in `src/src/bridge/commands.rs`. Run the test; it should pass. Commit (GREEN).

### Add a UI component

UI rendering is exempt from test-first per Principle V. Add the component under `src/src/components/`, wire it into `app.rs` or its parent component, run `cargo tauri dev` to verify behaviour, run the visual regression suite to verify pixel-equivalence (or accept a baseline update for an intended visual change with a one-line PR note).

### Modify the timer engine

Per Principle V (and SC-007), failing tests precede implementation. Add the test in `src/src/engine/tests.rs` first, watch it fail, commit. Then implement, watch it pass, commit (separate commit — diff must show RED first, then GREEN).

### Add or change a theme

Drop a CSS file in `src/style/themes/<name>.css`. The Trunk pre-build hook re-runs `tools/build-themes/`, regenerating `src/src/theme/themes.rs`. The new theme appears in the theme picker on next build (FR-021).

---

## 6. Commit hygiene

The pre-commit hook (husky-equivalent) runs:

- `cargo fmt --check` on touched Rust files.
- `cargo clippy --workspace --frozen -- -D warnings` (PR-level CI runs full pedantic).
- Lockfile-drift check on both `Cargo.lock` (vs. workspace members' `Cargo.toml`s) and `tests/e2e/package-lock.json` (vs. `tests/e2e/package.json`).

Per AGENTS.md §Test-first commit ordering, `--no-verify` is used **only** in genuine emergencies and the next commit fixes the bypass. Do not split a RED-and-GREEN change into one commit — the hook will accept it but the per-PR audit will reject it.

---

## 7. Pre-release validation

Before merging the cutover PR (and once per major-version release thereafter), the maintainer runs the auto-updater validation checklist documented in [`plan.md`](./plan.md) §"Pre-release validation checklist". Briefly: install the prior release on a clean profile, populate it with test data + non-default settings, build the post-cutover bundle, run the auto-update path, and confirm auth state, sessions, tasks, tags, manual sessions, and settings (including theme and `status_bar_display`) all survive — and that the localStorage migration is idempotent on a second launch. The full step list is in the plan.

---

## 8. Where to look when stuck

- **Constitution**: `.specify/memory/constitution.md` — the 9 principles. Re-read for non-trivial work.
- **AGENTS.md**: operational rules for AI / human contributors.
- **Spec for this feature**: `specs/001-leptos-migration/spec.md`.
- **This plan**: `specs/001-leptos-migration/plan.md`.
- **Bridge contract**: `specs/001-leptos-migration/contracts/tauri-bridge.md` — every Tauri command's typed shape.
- **Data model**: `specs/001-leptos-migration/data-model.md`.
- **Tauri command sources**: `src-tauri/src/lib.rs`, `src-tauri/src/helpers.rs`.
- **Tauri mock**: `tests/e2e/fixtures/tauriMock.js`.
- **Visual baselines**: `tests/e2e/__screenshots__/visual-regression/`.
- **CI pipeline**: `.agentex.yml`.

For Leptos-specific questions, the Leptos book (`https://book.leptos.dev/`) covers signals, components, and the CSR build model. For Trunk specifics, see `https://trunkrs.dev/`.
