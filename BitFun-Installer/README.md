# BitFun Installer

A fully custom, branded installer for BitFun — built with **Tauri 2 + React** for maximum UI flexibility.

## Why a Custom Installer?

Instead of relying on the generic NSIS wizard UI from Tauri's built-in bundler, this project provides:

- **100% custom UI** — React-based, with smooth animations, dark theme, and brand consistency
- **Modern experience** — Similar to Discord, Figma, and VS Code installers
- **Full control** — Custom installation logic, right-click context menu, PATH integration
- **Cross-platform potential** — Same codebase can target Windows, macOS, and Linux

## Architecture

```
BitFun-Installer/
├── src-tauri/                 # Tauri / Rust backend
│   ├── src/
│   │   ├── main.rs            # Entry point
│   │   ├── lib.rs             # Tauri app setup
│   │   └── installer/
│   │       ├── commands.rs    # Tauri IPC commands
│   │       ├── extract.rs     # Archive extraction
│   │       ├── registry.rs    # Windows registry (uninstall, context menu, PATH)
│   │       ├── shortcut.rs    # Desktop & Start Menu shortcuts
│   │       └── types.rs       # Shared types
│   ├── capabilities/
│   ├── icons/
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                       # React frontend
│   ├── pages/
│   │   ├── LanguageSelect.tsx # First screen language picker
│   │   ├── Options.tsx        # Path picker + install options
│   │   ├── Progress.tsx       # Install progress + confirm
│   │   ├── ModelSetup.tsx     # Optional model provider setup
│   │   └── ThemeSetup.tsx     # Theme preview + finish
│   ├── components/
│   │   ├── WindowControls.tsx # Custom titlebar
│   │   ├── Checkbox.tsx       # Styled checkbox
│   │   └── ProgressBar.tsx    # Animated progress bar
│   ├── hooks/
│   │   └── useInstaller.ts    # Core installer state machine
│   ├── styles/
│   │   ├── global.css         # Base styles
│   │   ├── variables.css      # Design tokens
│   │   └── animations.css     # Keyframe animations
│   ├── types/
│   │   └── installer.ts       # TypeScript types
│   ├── App.tsx
│   └── main.tsx
├── scripts/
│   └── build-installer.cjs   # End-to-end build script
├── index.html
├── package.json
├── vite.config.ts
└── tsconfig.json
```

## Installation Flow

```
Language Select → Options → Progress → Model Setup → Theme Setup
       │             │          │            │              │
   choose UI      path +     run real    optional AI     save theme,
    language      options    install      model config    launch/close
```

## Development

### Prerequisites

- Node.js 18+
- Rust (latest stable)
- pnpm (or npm)

### Setup

```bash
cd ..
npm ci
cd BitFun-Installer
npm ci
```

Or from repository root:

```bash
npm --prefix BitFun-Installer ci
```

Production installer builds call workspace desktop build scripts, so root dependencies are required.

### Repository Hygiene

Keep generated artifacts out of commits. This project ignores:

- `node_modules/`
- `dist/`
- `src-tauri/target/`
- `src-tauri/payload/`

### Dev Mode

Run the installer in development mode with hot reload:

```bash
npm run tauri:dev
```

### Uninstall Mode (Dev + Runtime)

Key behavior:

- Install phase creates `uninstall.exe` in the install directory.
- Windows uninstall registry entry points to:
  `"<installPath>\\uninstall.exe" --uninstall "<installPath>"`.
- Launching with `--uninstall` opens the dedicated uninstall UI flow.
- Launching `uninstall.exe` directly also enters uninstall mode automatically.

Local debug command:

```bash
npx tauri dev -- -- --uninstall "D:\\tmp\\bitfun-uninstall-test"
```

Core implementation:

- Launch arg parsing + uninstall execution: `src-tauri/src/installer/commands.rs`
- Uninstall registry command: `src-tauri/src/installer/registry.rs`
- Uninstall UI page: `src/pages/Uninstall.tsx`
- Frontend mode switching/state: `src/hooks/useInstaller.ts`

### Build

Build the complete installer in release mode (default, optimized):

```bash
npm run installer:build
```

Use this as the release entrypoint. `npm run tauri:build` does not prepare validated payload assets for production.
Release artifacts embed payload files into the installer binary, so runtime installation does not depend on an external `payload` folder.

Build the complete installer in fast mode (faster compile, less optimization):

```bash
npm run installer:build:fast
```

Build installer only (skip main app build):

```bash
npm run installer:build:only
```

`installer:build:only` now requires an existing valid desktop executable in target output paths. If payload validation fails, build exits with an error.

Build installer only with fast mode:

```bash
npm run installer:build:only:fast
```

### Output

The built executable will be at:

```
src-tauri/target/release/bitfun-installer.exe
```

Fast mode output path:

```
src-tauri/target/release-fast/bitfun-installer.exe
```

## Customization Guide

### Changing the UI Theme

Edit `src/styles/variables.css` — all colors, spacing, and animations are controlled by CSS custom properties.

### Adding Install Steps

1. Add a new step key to `InstallStep` type in `src/types/installer.ts`
2. Create a new page component in `src/pages/`
3. Add the step to the `STEPS` array in `src/hooks/useInstaller.ts`
4. Add the page render case in `src/App.tsx`

### Modifying Install Logic

- **File extraction** → `src-tauri/src/installer/extract.rs`
- **Registry operations** → `src-tauri/src/installer/registry.rs`
- **Shortcuts** → `src-tauri/src/installer/shortcut.rs`
- **Tauri commands** → `src-tauri/src/installer/commands.rs`

### Adding Installer Payload

Place the built BitFun application files in `src-tauri/payload/` before building the installer. The build script handles this automatically.
During `cargo build`, the payload directory is packed into an embedded zip inside `bitfun-installer.exe`.

## Integration with CI/CD

Add to your GitHub Actions workflow:

```yaml
- name: Build Installer
  run: |
    cd BitFun-Installer
    npm ci
    npm run installer:build:only

- name: Upload Installer
  uses: actions/upload-artifact@v4
  with:
    name: BitFun-Installer-Exe
    path: BitFun-Installer/src-tauri/target/release/bitfun-installer.exe
```
