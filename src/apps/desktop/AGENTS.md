[中文](AGENTS-CN.md) | **English**

# AGENTS.md

## Scope

This file applies to `src/apps/desktop`. Use the top-level `AGENTS.md` for repository-wide rules.

## What matters here

`src/apps/desktop` is the Tauri host / integration layer.

Main areas:

- `src/api/`: Tauri commands
- `src/lib.rs`, `src/main.rs`: app setup and wiring
- `src/computer_use/`: OS-specific automation support

If a change affects shared product behavior across runtimes, the implementation likely belongs in `src/crates/core`.

## Local rules

- Keep desktop-only integrations here; do not move them into shared core
- For packaging or release asks, see the top-level `AGENTS.md`

## Commands

```bash
pnpm run desktop:dev
pnpm run desktop:preview:debug
cargo check -p bitfun-desktop
cargo test -p bitfun-desktop
cargo build -p bitfun-desktop
pnpm run desktop:build:fast
```

## Fast builds

| Command | When to use |
|---|---|
| `pnpm run desktop:build:fast` | Debug build without bundling; fastest compile for manual testing |
| `pnpm run desktop:build:release-fast` | Release-like build with reduced LTO; use when you need release behavior but can't wait for full LTO |
| `pnpm run desktop:build:nsis:fast` | Windows installer using `release-fast` profile; for quick installer validation |

`release-fast` profile (`Cargo.toml`): inherits `release` but disables LTO, increases `codegen-units` to 16, enables incremental compilation. Significantly faster at the cost of binary size and marginal runtime performance.

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
