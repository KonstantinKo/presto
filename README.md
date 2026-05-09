Due to numerous commitments, the project has unfortunately been abandoned.

<img src="https://github.com/murdercode/presto/raw/HEAD/art/banner.png" width="100%" alt="Presto banner" style="max-width: 100%;">

# Presto - Pomodoro Timer

A modern, cross-platform Pomodoro timer application built with Tauri (Rust + HTML/CSS/JavaScript). Presto helps you boost productivity using the proven Pomodoro Technique with a beautiful, intuitive interface.

## ✨ Features

### 🍅 Pomodoro Technique

- **Standard Pomodoro cycles**: 25-minute work sessions
- **Smart breaks**: 5-minute short breaks, 20-minute long breaks every 4 cycles
- **Daily goal**: Track progress through 10 daily Pomodoro sessions
- **Visual progress**: Dot indicators showing session completion

### ⏱️ Timer Management

- **Flexible controls**: Start, pause, reset, and skip functionality
- **Visual feedback**: Dynamic UI that changes based on session type (work/break)
- **Audio notifications**: Sound alerts for session transitions
- **Desktop notifications**: System notifications to keep you informed

### 📋 Task Management

- **Task tracking**: Add and manage tasks for each Pomodoro session
- **Task completion**: Mark tasks as completed with visual feedback
- **Persistence**: Tasks are automatically saved and restored

### 📊 Statistics & History

- **Weekly statistics**: Track your productivity patterns
- **Session history**: View detailed history of completed sessions
- **Progress tracking**: Monitor your daily and weekly Pomodoro completion

### ⌨️ Keyboard Shortcuts

- **Cmd/Ctrl + Alt + Space**: Start/Pause timer
- **Cmd/Ctrl + Alt + R**: Reset current session
- **Cmd/Ctrl + Alt + S**: Skip current session
- **Cmd/Ctrl + H**: Show/hide history modal
- **Space**: Start/Pause timer (fallback when no custom shortcut is active)

### 🎨 Modern UI

- **Dark mode design**: Easy on the eyes for long work sessions
- **Responsive layout**: Works on different screen sizes
- **Smooth animations**: Polished user experience
- **Protection**: Prevents accidental closure during active sessions

## 🚀 Getting Started

### Installation via Homebrew (Recommended)

The easiest way to install Presto on macOS is through Homebrew:

```bash
brew install --cask murdercode/presto/presto
```

#### ⚠️ Troubleshooting: "Presto is damaged and can't be opened"

If you see this error when launching Presto for the first time, it's a temporary issue that occurs because the app lacks an Apple Developer signature (which requires paying $99 to Apple). This is a common situation for open-source applications. To resolve it, run this command in Terminal:

```bash
xattr -d com.apple.quarantine /Applications/presto.app
```

Then you can launch Presto normally from your Applications folder or Spotlight.

### Installation from Source

If you prefer to build from source, you'll need:

#### Prerequisites

- [Node.js](https://nodejs.org/) (v16 or higher)
- [Rust](https://rustup.rs/) (latest stable; the project is verified against `1.89.0`)
- Xcode Command Line Tools (macOS): `xcode-select --install`

The Tauri CLI is pulled in automatically via `npm install` (`@tauri-apps/cli`); no separate install is needed.

#### Steps

1. **Clone the repository**

   ```bash
   git clone https://github.com/murdercode/presto.git
   cd presto
   ```

2. **Install dependencies**

   ```bash
   npm install
   ```

3. **Run in development mode**

   ```bash
   npm run tauri dev
   ```

   The first run downloads ~200 crates and compiles the Rust backend, which takes several minutes. Subsequent runs are incremental.

4. **Build for production**
   ```bash
   npm run tauri build
   ```

#### Troubleshooting

- **`No version is set for command cargo` / `rustc`** — you are using `asdf` (or another version manager) and no Rust version is selected for this directory. Add a `.tool-versions` file at the **repo root** (not inside `src-tauri/`; the Tauri CLI invokes `rustc` from the repo root):

  ```
  rust 1.89.0
  ```

  Or set a global default with `asdf set -u rust 1.89.0`.

- **`thread '<unnamed>' panicked at ... rust.rs:... called Option::unwrap() on a None value`** — same root cause as above. The Tauri CLI parses `rustc -vV` output and panics if `rustc` cannot be resolved on `PATH`. Fix the toolchain resolution and re-run.

- **`Found version mismatched Tauri packages`** — the npm packages and the Rust crates have drifted out of sync (e.g. `@tauri-apps/api` vs `tauri`). Pin the npm side to match the Rust crate versions resolved by Cargo (check `src-tauri/Cargo.lock`), e.g.:

  ```bash
  npm install --save-exact @tauri-apps/api@2.6.0 @tauri-apps/plugin-updater@2.8.1
  ```

- **Devtools in dev mode** — devtools are enabled via the `devtools` feature on the `tauri` crate in `src-tauri/Cargo.toml`. Open them with right-click → _Inspect Element_ (or `Cmd+Opt+I` on macOS) to surface frontend errors that the app's generic "An error occurred" toast hides.

## 🏗️ Project Structure

```
presto/
├── src/                         # Frontend source files
│   ├── index.html               # Main HTML interface
│   ├── main.js                  # Application entry point
│   ├── version.js               # Version constants
│   ├── globals.d.ts             # Global TypeScript declarations
│   ├── assets/                  # Static assets (icons, images)
│   ├── components/              # Reusable UI components
│   │   └── update-notification.js
│   ├── config/                  # Configuration constants
│   │   └── storage-keys.js
│   ├── core/                    # Core application logic
│   │   └── pomodoro-timer.js    # Timer state machine and controls
│   ├── docs/                    # Developer documentation
│   ├── managers/                # Feature-area managers
│   │   ├── auth-manager.js
│   │   ├── navigation-manager.js
│   │   ├── session-manager.js
│   │   ├── settings-manager.js
│   │   ├── tag-manager.js
│   │   ├── team-manager.js
│   │   └── update-manager-global.js
│   ├── styles/                  # CSS stylesheets
│   │   ├── themes/              # Pluggable timer themes
│   │   └── *.css                # Feature-scoped stylesheets
│   └── utils/                   # Shared utilities
│       ├── analytics.js
│       ├── theme-loader.js
│       ├── timer-themes.js
│       └── ...
├── src-tauri/                   # Rust backend
│   ├── src/
│   │   └── lib.rs               # Tauri commands and data persistence
│   ├── Cargo.toml               # Rust dependencies
│   └── tauri.conf.json          # Tauri configuration
├── package.json                 # Node.js dependencies and scripts
└── README.md                    # This file
```

## 🔧 Technical Details

### Frontend (HTML/CSS/TypeScript-checked JavaScript)

- **Typed vanilla JavaScript**: No frameworks; JSDoc annotations with `checkJs: true` provide full TypeScript type coverage
- **CSS Grid & Flexbox**: Modern responsive layouts
- **CSS Custom Properties**: Consistent theming and easy customization
- **Local Storage**: Client-side data persistence

### Backend (Rust/Tauri)

- **Tauri framework**: Secure, fast native app wrapper
- **File-based storage**: JSON files for data persistence
- **Small bundle size**: Efficient Rust backend
  <br /><strike>- **Cross-platform**: Works on Windows, macOS, and Linux</strike>

### Data Persistence

The application stores data in the following locations:

- **Session data**: Current timer state and progress
- **Tasks**: User-created task list
- **Statistics**: Daily and weekly productivity stats
- **History**: Historical session data

## 🎯 The Pomodoro Technique

The Pomodoro Technique is a time management method developed by Francesco Cirillo:

1. **Choose a task** to work on
2. **Set timer for 25 minutes** (one "Pomodoro")
3. **Work on the task** until timer rings
4. **Take a 5-minute break**
5. **Repeat steps 1-4**
6. **After 4 Pomodoros**, take a longer 20-minute break

### Benefits

- Improved focus and concentration
- Better time estimation skills
- Reduced mental fatigue
- Enhanced productivity
- Better work-life balance

## 🛠️ Development

### Available Scripts

- `npm run tauri dev` - Start development server
- `npm run tauri build` - Build production app
- `npm test` - Run JavaScript unit tests (Vitest)
- `npm run typecheck` - TypeScript type-check all JS sources (`tsc --noEmit`)
- `npm run test:e2e` - Run Playwright E2E suite (UI-driven, browser-level)
- `cargo check` - Check Rust code (in src-tauri/)
- `cargo test` - Run Rust tests (in src-tauri/)

### Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/)
- [Tauri Extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## 📱 Platform Support

- **macOS** (10.13+)
- _**Windows** (coming soon TBA)_
- _**Linux** (coming soon TBA)_

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Francesco Cirillo](https://francescocirillo.com/) for creating the Pomodoro Technique
- [Tauri](https://tauri.app/) for the amazing framework
- The Rust and web development communities

---

**Start your productive journey with Presto!** 🍅✨

## 🔄 Automatic Updates

Presto includes an automatic update system that allows you to receive new versions directly from the app interface.

### Features

- **Automatic checking**: The app checks every hour for available updates
- **Non-invasive notifications**: Elegant notification that appears when an update is available
- **Progressive download**: Progress bar during download
- **Automatic installation**: Update is applied on restart
- **Security**: All updates are digitally signed

### Developer Configuration

If you want to configure the update system for your fork:

1. **Automatic setup**:

   ```bash
   ./setup-updates.sh
   ```

2. **Manual setup**:
   - Generate keys: `./generate-keys.sh`
   - Configure `src-tauri/tauri.conf.json` with your public key
   - Add GitHub secrets for the private key
   - Update repository references in the code

3. **Publishing**:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

For more details see [UPDATES.md](UPDATES.md).

### For Users

- Updates are checked automatically
- You can disable automatic checking in settings
- You can manually check in the "Updates" section of settings
- Downloads happen in background without interrupting work
