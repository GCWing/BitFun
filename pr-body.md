## Summary

When "Match system" is selected as the theme, users can now independently choose which light theme and which dark theme the system mode uses. Previously, system mode always fell back to the hardcoded defaults (`bitfun-light` / `bitfun-dark`).

Closes #1080

## Changes

- **`presets/index.ts`**: `getSystemPreferredDefaultThemeId()` now accepts optional `lightId`/`darkId` override parameters, falling back to the built-in defaults when not provided.
- **`ThemeService.ts`**: Added `systemLightId`/`systemDarkId` fields with lazy loading from `themes.systemLightId`/`themes.systemDarkId` config paths. Added `setSystemThemeOverride()` method that persists overrides and re-resolves the active theme if system mode is active. All three call sites of `getSystemPreferredDefaultThemeId()` now pass the user-configured overrides.
- **`themeStore.ts`**: Added `systemLightId`/`systemDarkId` state and `setSystemThemeOverride` action that delegates to `themeService`.
- **`useTheme.ts`**: Exposed `systemLightId`, `systemDarkId`, and `setSystemThemeOverride` from the hook.
- **`AppearanceConfig.tsx`**: Added two conditional `Select` dropdowns (light themes / dark themes) that appear only when "Match system" is selected, letting users pick independent light/dark themes.
- **i18n**: Added 4 new keys to `en-US`, `zh-CN`, and `zh-TW` locale files.

## Design decisions

- System theme overrides are loaded **lazily** — only when the user actually selects "Match system" mode. This avoids unnecessary config reads during initialization and preserves existing test expectations.
- Overrides are validated against the registered themes map; invalid IDs fall back to defaults silently.
- Config paths follow the existing `themes.*` namespace convention (`themes.systemLightId`, `themes.systemDarkId`).

## Validation

- `tsc --noEmit` — 0 errors
- `vitest run src/infrastructure/theme` — 37/37 tests passed
- `pnpm run i18n:audit` — 0 warnings
