# BitFun Installer

A fully custom, branded installer for BitFun — built with **Tauri 2 + React** for maximum UI flexibility.

## Why a Custom Installer?

Instead of relying on the generic NSIS wizard UI from Tauri's built-in bundler, this project provides:

- **100% custom UI** — React-based, with smooth animations, dark theme, and brand consistency
- **Modern experience** — Similar to Discord, Figma, and VS Code installers
- **Full control** — Custom installation logic, right-click context menu, PATH integration
- **Cross-platform potential** — Same codebase can target Windows, macOS, and Linux

## Common tasks

### Install dependencies

```bash
pnpm install
```

Production installer builds call workspace desktop build scripts, so root dependencies are required.

### Run in dev mode

```bash
pnpm run tauri:dev
```

### Build the full installer

```bash
pnpm run installer:build
```

Use this as the release entrypoint. `pnpm run tauri:build` does not prepare validated payload assets for production.

### Build installer only

```bash
pnpm run installer:build:only
```

`installer:build:only` requires an existing valid desktop executable in the expected target output path.

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
│   └── build-installer.cjs    # End-to-end build script
├── index.html
├── package.json
├── vite.config.ts
└── tsconfig.json
```

## Installation flow

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
- pnpm

### Setup

```bash
pnpm install
```

### Repository Hygiene

Keep generated artifacts out of commits. This project ignores:

- `node_modules/`
- `dist/`
- `src-tauri/target/`
- `src-tauri/payload/`

### Dev Mode

Run the installer in development mode with hot reload:

```bash
pnpm run tauri:dev
```

### Uninstall Mode (Dev + Runtime)

Key behavior:

- Install phase creates `uninstall.exe` in the install directory.
- Windows uninstall registry entry points to `"<installPath>\\uninstall.exe" --uninstall "<installPath>"`.
- Launching with `--uninstall` opens the dedicated uninstall UI flow.
- Launching `uninstall.exe` directly also enters uninstall mode automatically.

Local debug command:

```bash
npx tauri dev -- -- --uninstall "D:\\tmp\\example-install-dir"
```

Core implementation:

- Launch arg parsing + uninstall execution: [commands.rs](src-tauri/src/installer/commands.rs)
- Uninstall registry command: [registry.rs](src-tauri/src/installer/registry.rs)
- Uninstall UI page: [Uninstall.tsx](src/pages/Uninstall.tsx)
- Frontend mode switching and state: [useInstaller.ts](src/hooks/useInstaller.ts)

## Build

### Full release build

```bash
pnpm run installer:build
```

Release artifacts embed payload files into the installer binary, so runtime installation does not depend on an external `payload` folder.

### Full fast build

```bash
pnpm run installer:build:fast
```

### Installer-only build

```bash
pnpm run installer:build:only
```

If payload validation fails, the build exits with an error.

### Installer-only fast build

```bash
pnpm run installer:build:only:fast
```

### Output

Default release output:

```text
src-tauri/target/release/bitfun-installer.exe
```

Fast build output:

```text
src-tauri/target/release-fast/bitfun-installer.exe
```

## Customization guide

### Changing the UI Theme

Edit [variables.css](src/styles/variables.css). Colors, spacing, and animations are controlled by CSS custom properties.

### Adding Install Steps

1. Add a new step key to `InstallStep` in [installer.ts](src/types/installer.ts)
2. Create a new page component in [src/pages](src/pages)
3. Add the step to the `STEPS` array in [useInstaller.ts](src/hooks/useInstaller.ts)
4. Add the page render case in [App.tsx](src/App.tsx)

### Modifying Install Logic

- **File extraction** → [extract.rs](src-tauri/src/installer/extract.rs)
- **Registry operations** → [registry.rs](src-tauri/src/installer/registry.rs)
- **Shortcuts** → [shortcut.rs](src-tauri/src/installer/shortcut.rs)
- **Tauri commands** → [commands.rs](src-tauri/src/installer/commands.rs)

### Adding Installer Payload

Place the built BitFun application files in `src-tauri/payload/` before building the installer. The build script handles this automatically.
During `cargo build`, the payload directory is packed into an embedded zip inside `bitfun-installer.exe`.

## Integration with CI/CD

Add to your GitHub Actions workflow:

```yaml
- name: Build Installer
  run: |
    cd BitFun-Installer
    pnpm install
    pnpm run installer:build:only

- name: Upload Installer
  uses: actions/upload-artifact@v4
  with:
    name: BitFun-Installer-Exe
    path: BitFun-Installer/src-tauri/target/release/bitfun-installer.exe
```
