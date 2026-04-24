 

export { bitfunDarkTheme } from './dark-theme';
export { bitfunLightTheme } from './light-theme';
export { bitfunMidnightTheme } from './midnight-theme';
export { bitfunChinaStyleTheme } from './china-style-theme';
export { bitfunChinaNightTheme } from './china-night-theme';
export { bitfunCyberTheme } from './cyber-theme';
export { bitfunSlateTheme } from './slate-theme';
export { sparoLightTheme } from './sparo-light-theme';

import { bitfunDarkTheme } from './dark-theme';
import { bitfunLightTheme } from './light-theme';
import { bitfunMidnightTheme } from './midnight-theme';
import { bitfunChinaStyleTheme } from './china-style-theme';
import { bitfunChinaNightTheme } from './china-night-theme';
import { bitfunCyberTheme } from './cyber-theme';
import { bitfunSlateTheme } from './slate-theme';
import { sparoLightTheme } from './sparo-light-theme';
import { ThemeConfig, ThemeId } from '../types';

/** Default light / dark builtin themes used when following system appearance. */
export const DEFAULT_LIGHT_THEME_ID: ThemeId = 'bitfun-light';
export const DEFAULT_DARK_THEME_ID: ThemeId = 'bitfun-dark';

/**
 * Picks bitfun-dark vs bitfun-light from `prefers-color-scheme`.
 * Used when the user has no saved theme preference.
 */
export function getSystemPreferredDefaultThemeId(): ThemeId {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return DEFAULT_LIGHT_THEME_ID;
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches
    ? DEFAULT_DARK_THEME_ID
    : DEFAULT_LIGHT_THEME_ID;
}

/** Static fallback when system preference is unavailable (e.g. SSR). */
export const DEFAULT_THEME_ID: ThemeId = DEFAULT_LIGHT_THEME_ID;

 
export const builtinThemes: ThemeConfig[] = [
  sparoLightTheme,
  bitfunLightTheme,
  bitfunSlateTheme,
  bitfunDarkTheme,
  bitfunMidnightTheme,
  bitfunChinaStyleTheme,
  bitfunChinaNightTheme,
  bitfunCyberTheme,
];

 



