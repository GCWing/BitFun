import type { AppearancePalette, AppearancePaletteId } from './AppearancePalette';
import { DEFAULT_DARK_APPEARANCE_ID, DEFAULT_LIGHT_APPEARANCE_ID } from './palettes';

export const STARTUP_APPEARANCE_BOOTSTRAP_SCHEMA_VERSION = 1 as const;

export interface StartupAppearanceBootstrapEntry {
  id: AppearancePaletteId;
  bgPrimary: string;
  bgSecondary: string;
  bgScene: string;
  isLight: boolean;
  textPrimary: string;
  textMuted: string;
  accentColor: string;
}

export interface StartupAppearanceBootstrapManifest {
  schemaVersion: typeof STARTUP_APPEARANCE_BOOTSTRAP_SCHEMA_VERSION;
  defaultLightAppearanceId: AppearancePaletteId;
  defaultDarkAppearanceId: AppearancePaletteId;
  appearances: StartupAppearanceBootstrapEntry[];
}

export function createStartupAppearanceBootstrapEntry(
  palette: AppearancePalette,
): StartupAppearanceBootstrapEntry {
  const chrome = palette.colors.chrome;
  return {
    id: palette.id,
    bgPrimary: chrome?.background.chrome
      ?? chrome?.background.primary
      ?? palette.colors.background.chrome
      ?? palette.colors.background.primary,
    bgSecondary: chrome?.background.secondary ?? palette.colors.background.secondary,
    bgScene: palette.colors.background.scene,
    isLight: palette.type === 'light',
    textPrimary: chrome?.text.primary ?? palette.colors.text.primary,
    textMuted: chrome?.text.muted ?? palette.colors.text.muted,
    accentColor: chrome?.accent[500] ?? palette.colors.accent[500],
  };
}

export function createStartupAppearanceBootstrapManifest(
  palettes: readonly AppearancePalette[],
): StartupAppearanceBootstrapManifest {
  return {
    schemaVersion: STARTUP_APPEARANCE_BOOTSTRAP_SCHEMA_VERSION,
    defaultLightAppearanceId: DEFAULT_LIGHT_APPEARANCE_ID,
    defaultDarkAppearanceId: DEFAULT_DARK_APPEARANCE_ID,
    appearances: palettes.map(createStartupAppearanceBootstrapEntry),
  };
}
