import type { ThemeConfig, ThemeId } from '../types';
import { DEFAULT_DARK_THEME_ID, DEFAULT_LIGHT_THEME_ID } from './index';

export const THEME_PROMPT_SNAPSHOT_VERSION = 1;

export interface ThemePromptSnapshotEntry {
  id: ThemeId;
  themeType: ThemeConfig['type'];
  bgPrimary: string;
  bgSecondary: string;
  bgScene: string;
  textPrimary: string;
  textMuted: string;
  accent500: string;
  accent600: string;
  borderBase: string;
  elementBase: string;
  radiusBase: string;
  spacing4: string;
  shadowBase: string;
  styleNotes: string;
}

export interface ThemePromptSnapshotManifest {
  version: typeof THEME_PROMPT_SNAPSHOT_VERSION;
  defaultLightThemeId: ThemeId;
  defaultDarkThemeId: ThemeId;
  themes: ThemePromptSnapshotEntry[];
}

export function createThemePromptSnapshotEntry(theme: ThemeConfig): ThemePromptSnapshotEntry {
  return {
    id: theme.id,
    themeType: theme.type,
    bgPrimary: theme.colors.background.primary,
    bgSecondary: theme.colors.background.secondary,
    bgScene: theme.colors.background.scene,
    textPrimary: theme.colors.text.primary,
    textMuted: theme.colors.text.muted,
    accent500: theme.colors.accent[500],
    accent600: theme.colors.accent[600],
    borderBase: theme.colors.border.base,
    elementBase: theme.colors.element.base,
    radiusBase: theme.effects.radius.base,
    spacing4: theme.effects.spacing[4],
    shadowBase: theme.effects.shadow.base,
    styleNotes: theme.description ?? theme.name,
  };
}

export function createThemePromptSnapshotManifest(
  themes: readonly ThemeConfig[],
): ThemePromptSnapshotManifest {
  return {
    version: THEME_PROMPT_SNAPSHOT_VERSION,
    defaultLightThemeId: DEFAULT_LIGHT_THEME_ID,
    defaultDarkThemeId: DEFAULT_DARK_THEME_ID,
    themes: themes.map(createThemePromptSnapshotEntry),
  };
}
