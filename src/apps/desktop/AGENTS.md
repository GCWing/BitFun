[中文](AGENTS-CN.md) | **English**

# AGENTS.md

## Scope

This file applies to `src/apps/desktop`. Use the top-level `AGENTS.md` for repository-wide rules.

## What matters here

`src/apps/desktop` is the Tauri host / integration layer.

Main areas:

- `src/api/`: Tauri commands
- `src/api/peer_host_invoke.rs`: Peer Device Mode host-invoke bridge + control attach
- `src/lib.rs`, `src/main.rs`: app setup and wiring
- `src/computer_use/`: OS-specific automation support

Peer Device Mode ownership and boundaries:
`docs/architecture/peer-device-mode.md`.
Frontend regression guards:
`src/web-ui/src/infrastructure/peer-device/README.md`.

Account login (pending sync choice / finalize) lives in
`src/api/remote_connect_api.rs` (`PENDING_SYNC_CHOICE`, `account_login`,
`account_finalize_login`). Do not persist a session before the user chooses
cloud vs local settings.

One-click relay deploy: Tauri surface `src/api/relay_deploy_api.rs`, orchestration
in `bitfun-services-integrations` `remote_ssh/relay_deploy.rs`. Feature invariants:
`src/web-ui/src/features/relay-deploy/README.md`.

If a change affects behavior shared by multiple runtimes, place stable contracts,
execution policy, and services in their owning lower-layer crates. Keep only
product wiring and compatibility bridges in `src/crates/assembly/core`.

## Local rules

- Keep desktop-only integrations here; do not move them into shared core
- Window lifecycle behavior, including close/minimize-to-tray defaults, is a
  desktop surface concern. Preserve saved user preferences when changing it.

## Commands

Use these for the desktop development loop. Verification commands are kept in
the Verification section below.

```bash
pnpm run desktop:dev
pnpm run desktop:preview:debug
pnpm run prepare:dsh-profile   # optional: local DeepSeek Harness sessions
```

## Fast builds

| Command | When to use |
|---|---|
| `pnpm run desktop:build:fast` | Debug build without bundling; for compile verification. Its binary breaks IPC against the dev server — see the two-semantics note below |
| `pnpm run desktop:build:release-fast` | Release-like build with reduced LTO; use when you need release behavior but can't wait for full LTO |
| `pnpm run desktop:build:nsis:fast` | Windows installer using `release-fast` profile; for quick installer validation |

Set `CARGO_PROFILE_DEV_DEBUG=2` when full breakpoint debug information is
required. The default dev profile keeps line tables while reducing PDB size.

### Debug binaries have two semantics; a `desktop:build:fast` binary breaks IPC against the dev server

`target/debug/bitfun-desktop.exe` can be built with two different tauri semantics:

- `cargo build -p bitfun-desktop` (also what `desktop:preview:debug` builds internally): tauri dev semantics (`DEP_TAURI_DEV=true`). The dev server origin `http://localhost:1422` is trusted; IPC works.
- `desktop:build:fast` runs `tauri build`, which enables `custom-protocol`: tauri production semantics. The same origin is treated as a remote URL and the ACL denies every app command and `plugin-log`.

Debug builds always navigate to `devUrl` (startup log `url_kind=external`), so running a `desktop:build:fast` binary against the dev server renders a fully working UI where every invoke is rejected: `... not allowed. Plugin not found` error toasts, session list failures, an empty miniapp catalog (the load error is swallowed into an empty list), and a 0-byte `webview.log` in the session log dir. Launching such a binary without the dev server shows `ERR_CONNECTION_REFUSED` instead.

`desktop:preview:debug` reuses the existing binary whenever its mtime is newer than the tracked inputs — including a leftover `desktop:build:fast` binary. After running `desktop:build:fast`, run `cargo build -p bitfun-desktop` (or `pnpm run desktop:preview:debug -- --force-rebuild`) before the preview, or the broken binary is reused.

Diagnosis shortcut: rendered UI + 0-byte `webview.log` under `config/logs/<session>/` means IPC was denied by the ACL — a build-semantics problem, not a data problem. Data under `BITFUN_USER_ROOT` is unaffected.

Also note: builtin miniapp assets (for example the `bitfun-loopx` `ui.js`/`worker.js`) are embedded via `include_str!` into `bitfun-product-domains`, so asset edits recompile the product-domains → assembly-core → desktop chain; several minutes for an incremental build is normal. `os error 5` on the exe itself means an instance is still running and locks it; see the GC-race section below.

## Target cache GC

`desktop:dev` (on exit), `desktop:preview:debug` (on shutdown), and `desktop:build*` prune stale `target/<profile>` cache generations. Incremental roots keep the latest crate/session. Cargo fingerprint JSON identifies distinct lib, test, bin, and build-script units; GC keeps the latest generation of each unit plus every generation whose Cargo-managed `invoked.timestamp` was refreshed within the last 24 hours, then removes orphaned `deps` files and `build` directories. Busy detection is scoped to Cargo lock files in the selected profile, so an unrelated worktree build does not suppress GC. Manual: `pnpm run target:gc -- --profile debug`. Disable with `BITFUN_TARGET_GC=0`; dry-run with `BITFUN_TARGET_GC_DRY_RUN=1`; adjust the grace window with `BITFUN_TARGET_GC_MIN_AGE_HOURS`.

`release-fast` profile (`Cargo.toml`): inherits `release` but disables LTO, increases `codegen-units` to 16, enables incremental compilation. Significantly faster at the cost of binary size and marginal runtime performance.

### Concurrent manual builds race the shutdown GC

Killing `bitfun-desktop.exe` ends the `desktop:dev` / `desktop:preview:debug` session, and that shutdown runs target GC. A manual `cargo build -p bitfun-desktop` started too early can then fail mid-compile with `os error 3` (系统找不到指定的路径) writing into `target/debug/build` or `target/debug/incremental` because GC deleted those directories while the build was writing. `os error 5` (拒绝访问) on `bitfun-desktop.exe` itself means the app is still running and locks the exe. Both are transient: make sure the preview session (node `dev.cjs` + vite + the exe) has fully exited — wait a few seconds after killing the exe — and simply re-run the build. No `cargo clean` is needed.

## DevTools feature (model rule)

The `devtools` Cargo feature exists for debugging UI/UX in the desktop app. When adding or modifying debug-related code:

- Guard all debug-only APIs and commands with `#[cfg(any(debug_assertions, feature = "devtools"))]`
- Provide no-op stubs under `#[cfg(not(any(debug_assertions, feature = "devtools")))]` so commands can always be registered in `invoke_handler`
- The feature is enabled automatically in `dev` builds and `release-fast` profile builds via `--features devtools`
- Never enable in `release` profile builds intended for end users

## Verification

```bash
cargo check -p bitfun-desktop && cargo test -p bitfun-desktop
```

If the change affects startup, WebDriver, browser/computer-use, or packaged behavior, also run:

```bash
cargo build -p bitfun-desktop
```
