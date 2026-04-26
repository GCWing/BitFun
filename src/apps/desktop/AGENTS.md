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

- Keep Tauri commands consistent: `snake_case` names, structured `request`
- Keep desktop-only integrations here; do not move them into shared core
- For local temporary debugging, prefer `pnpm run desktop:preview:debug` for both frontend-only shared-UI changes and Rust / Tauri changes. It reuses the existing debug binary when it is still current and auto-rebuilds before preview when desktop-side inputs are newer or the binary is missing. Use `pnpm run desktop:dev` only when you need the full Tauri dev watcher or are debugging startup/build integration itself
- When the wording mixes "build/debug version" with "quickly inspect the effect", treat the higher-level intent as preview and use the preview commands instead of `pnpm run desktop:build:fast`

Preferred command shape:

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>,
    request: YourRequest,
) -> Result<YourResponse, String>
```

```ts
await api.invoke('your_command', { request: { ... } });
```

## Commands

```bash
pnpm run desktop:dev
pnpm run desktop:preview:debug
cargo check -p bitfun-desktop
cargo test -p bitfun-desktop
cargo build -p bitfun-desktop
pnpm run desktop:build:fast
```

## Verification

```bash
cargo check -p bitfun-desktop && cargo test -p bitfun-desktop
```

If the change affects startup, WebDriver, browser/computer-use, or packaged behavior, also run:

```bash
cargo build -p bitfun-desktop
```

The preview commands above are iteration shortcuts only; keep using the minimum Rust checks and any required build / E2E verification before finishing.

Use `pnpm run desktop:preview:debug -- --force-rebuild` only when you explicitly want to rebuild before preview even if the timestamp check says the binary is current.

Use `pnpm run desktop:build:fast` only when the user explicitly wants a debug build artifact without launching the app.

For packaging or release asks:

- Confirm the package form when the user did not specify whether they want a local fast artifact, a standalone executable, or an installer.
- Do not substitute preview/debug outputs for a real release deliverable.
- On Windows, prefer `pnpm run desktop:build:nsis` for installer-style delivery and `pnpm run desktop:build:exe` only when the user explicitly wants a standalone executable.
