 

export { bitfunDarkTheme } from './dark-theme';
export { bitfunLightTheme } from './light-theme';
export { bitfunMidnightTheme } from './midnight-theme';
export { bitfunChinaStyleTheme } from './china-style-theme';
export { bitfunChinaNightTheme } from './china-night-theme';
export { bitfunCyberTheme } from './cyber-theme';
export { bitfunSlateTheme } from './slate-theme';
export { bitfunTokyoNightTheme } from './tokyo-night-theme';

import { bitfunDarkTheme } from './dark-theme';
import { bitfunLightTheme } from './light-theme';
import { bitfunMidnightTheme } from './midnight-theme';
import { bitfunChinaStyleTheme } from './china-style-theme';
import { bitfunChinaNightTheme } from './china-night-theme';
import { bitfunCyberTheme } from './cyber-theme';
import { bitfunSlateTheme } from './slate-theme';
import { bitfunTokyoNightTheme } from './tokyo-night-theme';
import { ThemeConfig, ThemeId } from '../types';

/** Default light / dark builtin themes used when following system appearance. */
export const DEFAULT_LIGHT_THEME_ID: ThemeId = 'bitfun-light';
export const DEFAULT_DARK_THEME_ID: ThemeId = 'bitfun-dark';

/**
 * Picks the configured dark vs light theme from `prefers-color-scheme`.
 * When `lightId`/`darkId` are provided they override the built-in defaults,
 * allowing the user to pick independent light/dark themes for system mode.
 */
export function getSystemPreferredDefaultThemeId(
  lightId: ThemeId = DEFAULT_LIGHT_THEME_ID,
  darkId: ThemeId = DEFAULT_DARK_THEME_ID,
): ThemeId {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return lightId;
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches
    ? darkId
    : lightId;
}

/** Static fallback when system preference is unavailable (e.g. SSR). */
export const DEFAULT_THEME_ID: ThemeId = DEFAULT_LIGHT_THEME_ID;

 
export const builtinThemes: ThemeConfig[] = [
  bitfunLightTheme,
  bitfunSlateTheme,
  bitfunDarkTheme,
  bitfunMidnightTheme,
  bitfunChinaStyleTheme,
  bitfunChinaNightTheme,
  bitfunCyberTheme,
  bitfunTokyoNightTheme,
];

 



