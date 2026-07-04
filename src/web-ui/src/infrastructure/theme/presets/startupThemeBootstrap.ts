import type { ThemeConfig, ThemeId } from '../types';
import { DEFAULT_DARK_THEME_ID, DEFAULT_LIGHT_THEME_ID } from './index';

export const STARTUP_THEME_BOOTSTRAP_VERSION = 1;

export interface StartupThemeBootstrapEntry {
  id: ThemeId;
  bgPrimary: string;
  bgSecondary: string;
  bgScene: string;
  isLight: boolean;
  textPrimary: string;
  textMuted: string;
  accentColor: string;
}

export interface StartupThemeBootstrapManifest {
  version: typeof STARTUP_THEME_BOOTSTRAP_VERSION;
  defaultLightThemeId: ThemeId;
  defaultDarkThemeId: ThemeId;
  themes: StartupThemeBootstrapEntry[];
}

export function createStartupThemeBootstrapEntry(theme: ThemeConfig): StartupThemeBootstrapEntry {
  return {
    id: theme.id,
    bgPrimary: theme.colors.background.primary,
    bgSecondary: theme.colors.background.secondary,
    bgScene: theme.colors.background.scene,
    isLight: theme.type === 'light',
    textPrimary: theme.colors.text.primary,
    textMuted: theme.colors.text.muted,
    accentColor: theme.colors.accent[500],
  };
}

export function createStartupThemeBootstrapManifest(
  themes: readonly ThemeConfig[],
): StartupThemeBootstrapManifest {
  return {
    version: STARTUP_THEME_BOOTSTRAP_VERSION,
    defaultLightThemeId: DEFAULT_LIGHT_THEME_ID,
    defaultDarkThemeId: DEFAULT_DARK_THEME_ID,
    themes: themes.map(createStartupThemeBootstrapEntry),
  };
}
