import type { AppearancePalette, AppearancePaletteId } from './AppearancePalette';
import { DEFAULT_DARK_APPEARANCE_ID, DEFAULT_LIGHT_APPEARANCE_ID } from './palettes';

export const APPEARANCE_PROMPT_SNAPSHOT_SCHEMA_VERSION = 1 as const;

export interface AppearancePromptSnapshotEntry {
  id: AppearancePaletteId;
  mode: AppearancePalette['type'];
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

export interface AppearancePromptSnapshotManifest {
  schemaVersion: typeof APPEARANCE_PROMPT_SNAPSHOT_SCHEMA_VERSION;
  defaultLightAppearanceId: AppearancePaletteId;
  defaultDarkAppearanceId: AppearancePaletteId;
  appearances: AppearancePromptSnapshotEntry[];
}

export function createAppearancePromptSnapshotEntry(
  palette: AppearancePalette,
): AppearancePromptSnapshotEntry {
  return {
    id: palette.id,
    mode: palette.type,
    bgPrimary: palette.colors.background.primary,
    bgSecondary: palette.colors.background.secondary,
    bgScene: palette.colors.background.scene,
    textPrimary: palette.colors.text.primary,
    textMuted: palette.colors.text.muted,
    accent500: palette.colors.accent[500],
    accent600: palette.colors.accent[600],
    borderBase: palette.colors.border.base,
    elementBase: palette.colors.element.base,
    radiusBase: palette.effects.radius.base,
    spacing4: palette.effects.spacing[4],
    shadowBase: palette.effects.shadow.base,
    styleNotes: palette.description ?? palette.name,
  };
}

export function createAppearancePromptSnapshotManifest(
  palettes: readonly AppearancePalette[],
): AppearancePromptSnapshotManifest {
  return {
    schemaVersion: APPEARANCE_PROMPT_SNAPSHOT_SCHEMA_VERSION,
    defaultLightAppearanceId: DEFAULT_LIGHT_APPEARANCE_ID,
    defaultDarkAppearanceId: DEFAULT_DARK_APPEARANCE_ID,
    appearances: palettes.map(createAppearancePromptSnapshotEntry),
  };
}
