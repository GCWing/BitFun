[中文](AGENTS-CN.md) | **English**

# AGENTS.md

## Scope

This file applies to `src/apps/desktop`. Use the top-level `AGENTS.md` for repository-wide rules.

## What matters here

`src/apps/desktop` is the Tauri host / integration layer.

Main areas:

- `src/api/`: Tauri commands
- `src/api/peer_host_invoke.rs`: Peer Device Mode host-invoke bridge + control attach;
  allow/deny and capabilities come from the Product Operation Registry
  (`openbitfun_product_domains::remote_surface`), not from a local table
- `src/api/remote_workspace_policy.rs`: closure test proving every registered Tauri
  command has one registry row
- `src/lib.rs`, `src/main.rs`: app setup and wiring
- `src/computer_use/`: OS-specific automation support

Peer Device Mode ownership and boundaries:
`docs/architecture/peer-device-mode.md`.
Frontend regression guards:
`src/web-ui/src/infrastructure/peer-device/README.md`.

GitHub identity is shared through `account_identity_api.rs`. Relay device
registration and lifecycle live in `src/api/remote_connect_api.rs`; settings
remain on their owning device and there is no cloud/local sync choice.

The Relay deployment wizard is retired. Preserve the developer scripts under
`src/apps/relay-server` and their [operator guide](../relay-server/README.md).
The retained Tauri wrapper and services orchestration are compatibility tools,
not an entry point to restore in the product UI.

If a change affects behavior shared by multiple runtimes, place stable contracts,
execution policy, and services in their owning lower-layer crates. Keep only
product wiring and compatibility bridges in `src/crates/assembly/core`.

## Local rules

- Keep desktop-only integrations here; do not move them into shared core
- Window lifecycle behavior, including close/minimize-to-tray defaults, is a
  desktop surface concern. Preserve saved user preferences when changing it.
- `window_state_support` owns main-window geometry validation and atomic
  persistence in the legacy `.window-state.json` format. Do not reinstall the
  window-state plugin alongside it: the plugin's exit cache can overwrite repairs.

## Commands

Use these for the desktop development loop. Verification commands are kept in
the Verification section below.

```bash
pnpm run desktop:dev
pnpm run desktop:preview:debug
pnpm run prepare:dsh-profile   # optional: local DeepSeek Harness sessions
```

Data Migrator runs independently. Desktop development launchers do not build,
launch, or supervise the migrator; use its own development and release entry
points under `src/apps/data-migrator`.

## Fast builds

| Command | When to use |
|---|---|
| `pnpm run desktop:build:fast` | Debug build without bundling; fastest compile for manual testing |
| `pnpm run desktop:build:release-fast` | Release-like no-bundle build with reduced LTO; run it in place and never distribute the raw executable alone |
| `pnpm run desktop:build:nsis:fast` | Windows installer using `release-fast` profile; for quick installer validation |

Set `CARGO_PROFILE_DEV_DEBUG=2` when full breakpoint debug information is
required. The default dev profile keeps line tables while reducing PDB size.

## Target cache GC

`desktop:dev` (on exit), `desktop:preview:debug` (on shutdown), and `desktop:build*` prune stale `target/<profile>` cache generations. Incremental roots keep the latest crate/session. Cargo fingerprint JSON identifies distinct lib, test, bin, and build-script units; GC keeps the latest generation of each unit plus every generation whose Cargo-managed `invoked.timestamp` was refreshed within the last 24 hours, then removes orphaned `deps` files and `build` directories. Busy detection is scoped to Cargo lock files in the selected profile, so an unrelated worktree build does not suppress GC. Manual: `pnpm run target:gc -- --profile debug`. Disable with `OPENBITFUN_TARGET_GC=0`; dry-run with `OPENBITFUN_TARGET_GC_DRY_RUN=1`; adjust the grace window with `OPENBITFUN_TARGET_GC_MIN_AGE_HOURS`.

`release-fast` profile (`Cargo.toml`): inherits `release` but disables LTO, increases `codegen-units` to 16, enables incremental compilation. Significantly faster at the cost of binary size and marginal runtime performance.

All commands that pass `--no-bundle` emit a staged runtime tree rather than a
single-file application. The executable depends on the adjacent `frontend`,
`flashgrep`, `mobile-web`, and `resources` directories. Use
`pnpm run desktop:build:nsis` for a distributable Windows installer.

## DevTools feature (model rule)

The `devtools` Cargo feature exists for debugging UI/UX in the desktop app. When adding or modifying debug-related code:

- Guard all debug-only APIs and commands with `#[cfg(any(debug_assertions, feature = "devtools"))]`
- Provide no-op stubs under `#[cfg(not(any(debug_assertions, feature = "devtools")))]` so commands can always be registered in `invoke_handler`
- The feature is enabled automatically in `dev` builds and `release-fast` profile builds via `--features devtools`
- Never enable in `release` profile builds intended for end users

## Verification

```bash
cargo check -p openbitfun-desktop && cargo test -p openbitfun-desktop
```

For skill discovery response compatibility and timeouts, use
`cargo test -p openbitfun-desktop --lib api::skill_api::tests`.
For content-search routing and remote fallback protection, use
`cargo test --locked -p openbitfun-desktop --lib api::search_api::tests`.
For staged application-update cache and signature behavior, use
`cargo test -p openbitfun-desktop --lib api::update_api::tests`.
For peer system-info response compatibility, run
`cargo test -p openbitfun-desktop --lib system_info_home_contract`.
For window geometry recovery, legacy state compatibility, and snapshot persistence,
run `cargo test -p openbitfun-desktop --lib window_state_support::tests`.
For the matching startup wiring contract, run
`pnpm --dir src/web-ui run test:run src/app/startup/startupPerformanceContract.test.ts`.
For embedded browser preview encoding and target correlation, run
`cargo test -p openbitfun-desktop --lib api::browser_api::tests`.
After browser command registration changes, also run
`cargo test -p openbitfun-desktop --lib remote_workspace_policy`.
After changing updater command registration, also run
`cargo test -p openbitfun-desktop --lib remote_workspace_policy`.

If the change affects startup, WebDriver, browser/computer-use, or packaged behavior, also run:

```bash
cargo build -p openbitfun-desktop
```

To exercise packaged UI customization in an isolated native window without a
development server, build Web UI assets, then use the focused Creation harness:

```bash
pnpm run build:web
cargo build -p openbitfun-desktop --features devtools
node tests/e2e/scripts/run-creation-runtime.mjs
node tests/e2e/scripts/run-creation-runtime.mjs --suspended-paint  # occluded WebKit startup and reload
```

The harness copies a completed build into an independent frontend snapshot so
concurrent builds cannot replace its lazy modules. It uses temporary product storage, a private WebView store, and
`OPENBITFUN_E2E_PACKAGED_FRONTEND=1`. It checks state across document reloads;
the private test store intentionally does not survive process exit.
That debug-only switch takes effect only with the existing E2E storage guard;
release builds always use the packaged protocol.

For alternate dev-server ports and preview startup URL changes, run
`node --test scripts/dev-startup.test.mjs` and
`cargo test -p openbitfun-desktop --no-default-features --lib appearance::development_frontend_tests`.
`OPENBITFUN_DEV_PORT` selects the HTTP port; `OPENBITFUN_DEV_HMR_PORT` defaults
to the previous port. Desktop and Vite must use the same values. Development
launchers reuse the locked Sherpa library/archive cache across Git worktrees,
or download the archive through curl when absent; explicit SHERPA_ONNX overrides win.
