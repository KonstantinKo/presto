<img src="https://github.com/murdercode/presto/raw/HEAD/art/banner.png" width="100%" alt="Presto banner" style="max-width: 100%;">

# Presto — Pomodoro Timer

A modern, cross-platform Pomodoro timer built with **Tauri + Leptos** (Rust + WebAssembly). Presto helps you stay focused using the Pomodoro Technique with a clean, native desktop interface.

> Forked from [murdercode/presto](https://github.com/murdercode/presto) (upstream abandoned). This fork is actively maintained on a Leptos frontend post-cutover (single hard migration in 2026-05). See [`VISION.md`](VISION.md) for product scope and [`.specify/memory/constitution.md`](.specify/memory/constitution.md) for the engineering principles.

## ✨ Features

### 🍅 Pomodoro Technique

- Standard 25-minute focus cycles
- 5-minute short breaks; 20-minute long break every 4 cycles
- Daily goal tracking with visual progress dots
- Configurable durations in Settings

### ⏱️ Timer Management

- Start, pause, resume, skip, reset
- Mode-aware UI (focus / short break / long break)
- Audio + system notifications on transitions
- Smart-pause on inactivity (optional)
- Background-throttling-resistant timekeeping

### 📋 Tasks & Tags

- Per-session task list with completion tracking
- Tag any session for context grouping
- Local-first persistence via Tauri

### 📊 Statistics & History

- Per-day session count, weekly view, calendar of completed pomodoros
- Manual session entry for retroactive logging
- All history available offline

### 🔒 Local-Only

- Single-user, fully local. No accounts, no sync, no telemetry.
- The application does not send user data or telemetry; the only outbound network activity of that kind is the auto-updater's release check (no user data on the wire).

### ⌨️ Keyboard Shortcuts

- **Cmd/Ctrl + Alt + Space**: Start / Pause
- **Cmd/Ctrl + Alt + R**: Reset
- **Cmd/Ctrl + Alt + S**: Skip
- **Cmd/Ctrl + H**: History modal
- **Space**: Start / Pause (when no other shortcut is active)

### 🎨 UI

- Dark-mode design tuned for long focus sessions
- Pluggable themes (CSS files under `src/style/themes/`)
- Visual regression suite locks the UI contract — baseline PNGs in `tests/e2e/__screenshots__/visual-regression/` are part of the merge gate

## 🚀 Getting Started

### Installation via Homebrew (recommended, macOS)

```bash
brew install --cask murdercode/presto/presto
```

#### Troubleshooting: "Presto is damaged and can't be opened"

The app lacks an Apple Developer signature (no $99 fee paid). Clear the quarantine attribute:

```bash
xattr -d com.apple.quarantine /Applications/presto.app
```

### Build from source

#### Prerequisites

- [Rust](https://rustup.rs/) **1.95+** (hard floor — `Duration::from_hours` is used as a `const fn` in `src/src/managers/update.rs`). Pinned via `.tool-versions` at the repo root.
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [Trunk](https://trunkrs.dev/): `cargo install trunk --locked` (or `dnf install trunk` on Fedora)
- [Tauri CLI v2](https://tauri.app/): `cargo install tauri-cli --version "^2.0" --locked`
- Xcode Command Line Tools (macOS): `xcode-select --install`
- Linux: `webkit2gtk-4.1`, `libsoup3`, `libappindicator3-1` (or distro equivalents)

There is no Node.js or npm dependency at the repo root post-cutover. The end-to-end test suite has its own scoped `package.json` under `tests/e2e/` that pins `@playwright/test`.

#### Steps

1. **Clone the repository**

   ```bash
   git clone https://github.com/KonstantinKo/presto.git
   cd presto
   ```

2. **Run in development mode**

   ```bash
   cargo tauri dev
   ```

   Tauri's `beforeDevCommand` spawns `trunk serve --port 1420` against the `src/` crate (the cwd is anchored to the repo root via `git rev-parse --show-toplevel` because tauri-cli's auto-detected cwd lands on `tests/e2e/package.json`). The first run downloads ~400 crates and compiles both the Leptos frontend and the Tauri backend; subsequent runs are incremental.

3. **Build for production**

   ```bash
   cargo tauri build
   ```

   `beforeBuildCommand` produces the production WASM bundle in `src/dist/` via `trunk build --release`. The Tauri build then packages the bundle into a platform installer (`.dmg`, `.msi`, `.AppImage`, …) under `src-tauri/target/release/bundle/`.

#### Troubleshooting

- **`No version is set for command cargo` / `rustc`** — you are using `asdf` (or another version manager) and no Rust version is selected for this directory. The repo pins `rust 1.95.0` in `.tool-versions`; run `asdf install rust 1.95.0` (or whatever pin you see) to materialise it.

- **`error[E0658]: use of unstable library feature 'duration_constructors_lite'`** — your toolchain is older than 1.95. Upgrade (see above); `from_hours` only stabilised as a `const fn` in 1.95.

- **`error: failed to find tool. Is 'wasm32-unknown-unknown' installed?`** — `rustup target add wasm32-unknown-unknown`.

- **`error: no such command: tauri`** — `cargo install tauri-cli --version "^2.0" --locked`. When bumping `.tool-versions`, re-install (`--force`) so the binary lands under the new toolchain's `bin/` and asdf shims resolve it.

- **`trunk: command not found`** — `cargo install trunk --locked` (the workspace's `tools/externalize-boot/` post-build hook also expects `trunk` on PATH). Re-install on toolchain bumps for the same shim reason as above.

- **Devtools in dev mode** — devtools are enabled via the `devtools` feature on the `tauri` crate in `src-tauri/Cargo.toml`. Right-click → _Inspect Element_ (or `Cmd+Opt+I` on macOS).

## 🏗️ Project Structure

```
presto/
├── Cargo.toml                   # Workspace root (4 members)
├── Cargo.lock                   # Workspace lockfile (single source of truth)
├── src/                         # Leptos frontend crate (`presto-web`)
│   ├── Cargo.toml
│   ├── Trunk.toml               # Trunk build config (post-build CSP hook)
│   ├── build.rs                 # Cargo build script: scans style/themes/*.css → OUT_DIR/themes.rs
│   ├── index.html               # Trunk entry; boot script externalized post-build
│   ├── src/                     # Rust source — components / managers / engine / bridge
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   ├── app.rs               # Top-level router + persistence sinks
│   │   ├── engine/              # Pure timer state machine (no DOM access)
│   │   ├── managers/            # Settings / Session / Tag / Update / Navigation
│   │   ├── components/          # Leptos UI components (one per screen / tab)
│   │   ├── bridge/              # Tauri command + event wrappers (typed)
│   │   └── theme/               # Theme loader (themes.rs generated by build.rs into OUT_DIR)
│   ├── style/                   # CSS modules (one per feature area + theme/)
│   └── assets/                  # Icons (remixicon vendored)
├── src-tauri/                   # Tauri backend crate (`presto-lib`)
│   ├── src/
│   │   ├── lib.rs               # `#[tauri::command]` handlers + plugin setup
│   │   ├── exports.rs           # XLSX export via `rust_xlsxwriter`
│   │   └── helpers.rs           # Activity monitor + persistence helpers
│   ├── Cargo.toml
│   └── tauri.conf.json
├── tools/
│   ├── build-themes/            # Dev utility: writes themes.rs to src tree (superseded by build.rs)
│   └── externalize-boot/        # Trunk post-build hook: lifts inline boot to dist/boot.js (CSP)
├── tests/
│   └── e2e/                     # Playwright suite (scoped package.json + lockfile)
│       ├── package.json         # Pins @playwright/test only
│       ├── package-lock.json
│       ├── playwright.config.js
│       ├── fixtures/            # Shared fixtures (blockExternal, tauriMock, screens)
│       ├── __screenshots__/     # Visual regression baselines (chromium-linux)
│       └── *.spec.js            # 17 specs total (one per screen / major flow)
├── scripts/                     # CI gate scripts (mock-drift, baseline-cap, engine-purity, lockfile-drift)
├── .githooks/pre-commit         # Local lockfile-drift gate (install via scripts/install-git-hooks.sh)
├── .specify/                    # Spec-kit artefacts (constitution, templates, extensions)
├── AGENTS.md                    # Agent reading order, operational rules
├── CLAUDE.md                    # Workflow conventions, where to find things
├── VISION.md                    # Product scope + roadmap
└── README.md                    # This file
```

## 🔧 Technical Details

### Frontend (Leptos + WebAssembly)

- **Leptos** with fine-grained signal reactivity; no JS framework
- **`wasm-bindgen` + `web-sys`** for browser/DOM access; engine kept pure (no DOM imports under `src/src/engine/`, enforced by a CI gate)
- **`serde-wasm-bindgen`** for typed Tauri command round-trips — argument and return shapes are compile-time-checked between the Leptos call site and the Tauri-side handler

### Backend (Rust + Tauri 2.x)

- Tauri 2.x framework with the auto-updater, dialog, global-shortcut, opener, and notification plugins
- File-based JSON storage in the Tauri app-data directory
- No network egress for user data; the auto-updater is the only outbound traffic

### Quality gates

- **`cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic`** (and `-W clippy::nursery`) — non-negotiable lint posture across both crates
- **`cargo fmt --all --check`** — no formatting drift on merge
- **`wasm-bindgen-test` (node)** for DOM-bound unit tests; **`cargo test`** for pure logic
- **Playwright** for end-to-end UI flows and visual regression (≤2% pixel-ratio drift per `playwright.config.js`)
- **CI gate scripts** (`scripts/check-*.sh`): mock-drift, baseline-cap (≤2 baseline re-captures per PR), engine-purity (no DOM crates in the engine module), lockfile-drift (manifest ↔ lock pairs)
- **Pre-commit hook** at `.githooks/pre-commit` runs the lockfile-drift gate locally; install once via `bash scripts/install-git-hooks.sh`

## 🎯 The Pomodoro Technique

A time-management method developed by Francesco Cirillo:

1. Choose a task
2. Set a 25-minute timer (one "Pomodoro")
3. Work on the task until the timer rings
4. Take a 5-minute break
5. Repeat steps 1–4
6. After 4 Pomodoros, take a longer 20-minute break

## 🛠️ Development

### Common commands

| Command | What it does |
|---|---|
| `cargo tauri dev` | Run the app locally (Trunk serves the frontend, Tauri opens the window) |
| `cargo tauri build` | Produce a signed installer |
| `cargo test --workspace --frozen` | Run host-side unit + integration tests |
| `(cd src && wasm-pack test --node)` | Run wasm-bindgen-tests (DOM-bound logic) |
| `(cd src && trunk build)` | Build the WASM bundle without packaging |
| `(cd tests/e2e && npx playwright test)` | Run the e2e suite (17 specs) |
| `(cd tests/e2e && npx playwright test visual-regression.spec.js)` | Run the visual regression suite (12 baselines) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Strict-deny pedantic + nursery lint pass |
| `cargo fmt --all --check` | Formatting drift check |
| `bash scripts/check-mock-drift.sh` | Confirm `tauriMock.js` mirrors the Tauri handler set |

### Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
- [Tauri Extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)

## 📱 Platform Support

- **macOS** (10.13+) — primary target
- **Linux** (recent webkit2gtk-4.1) — best-effort
- **Windows** — best-effort; the Tauri auto-updater and global-shortcut plugins are exercised here too

## 🤝 Contributing

1. Read [`VISION.md`](VISION.md), [`CLAUDE.md`](CLAUDE.md), and [`.specify/memory/constitution.md`](.specify/memory/constitution.md) in that order
2. Fork the repository
3. Create a feature branch (`git checkout -b NNN-short-slug`)
4. Use the spec-kit slash commands (`/speckit-specify`, `/speckit-plan`, `/speckit-tasks`, `/speckit-implement`) for any multi-file feature
5. Commit your changes (lockfile drift gate runs on pre-commit)
6. Push and open a Pull Request

## 📄 License

This project is licensed under the MIT License — see the [LICENSE](LICENSE.md) file.

## 🙏 Acknowledgments

- [Francesco Cirillo](https://francescocirillo.com/) for the Pomodoro Technique
- [Tauri](https://tauri.app/) for the framework
- [Leptos](https://leptos.dev/) for the WASM frontend stack
- [murdercode](https://github.com/murdercode) for the original presto, now archived

## 🔄 Automatic Updates

Presto includes an automatic update system that delivers signed releases directly through the app.

### Features

- **Hourly checking** while the app is open (configurable; default on)
- **Non-invasive banner** when an update is available
- **Background download** with a progress indicator
- **Signed installs** — every release is verified before being applied on next launch

### Developer Configuration

If you fork this repo and want your own update channel:

1. **Automated setup**: `./setup-updates.sh`
2. **Manual setup**:
   - Generate keys: `./generate-keys.sh`
   - Add the public key to `src-tauri/tauri.conf.json`
   - Add the private key as a GitHub Actions secret
   - Update repository references in code
3. **Publishing**:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

See [`UPDATES.md`](UPDATES.md) for the full process.

### For Users

- Updates are checked automatically (toggle in Settings → Updates)
- Manual checks are available in the same panel
- Downloads happen in the background; the new version is applied on restart

---

**Start your productive journey with Presto!** 🍅✨
