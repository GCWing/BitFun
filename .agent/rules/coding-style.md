# Coding Style Rules

These rules summarize repository-wide coding expectations for Coding Agent tasks.

## General

- Read relevant files before editing.
- Prefer the nearest `AGENTS.md` or `AGENTS-CN.md` for module-specific guidance.
- Keep changes limited to the accepted intent and avoid unrelated refactors.
- Reuse existing patterns, helpers, components, and adapters before adding new abstractions.
- Do not introduce new dependencies without explicit approval.

## Logging

- Logs must be English-only and contain no emojis.
- Frontend logging should follow `src/web-ui/LOGGING.md`.
- Backend logging should follow `src/crates/LOGGING.md`.

## Tauri Commands

- Rust command names must use `snake_case`.
- TypeScript wrappers may use `camelCase`, but must invoke Rust commands with a structured `request`.

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

## Verification

- Run the smallest verification command that matches the changed surface.
- Report skipped verification and the reason.
- Prefer adding or updating automated tests when the project already has coverage for the touched behavior.

