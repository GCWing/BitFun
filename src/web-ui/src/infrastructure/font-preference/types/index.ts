
import {
  createTypographySizeScale,
  type TypographySizeScale,
} from '@openbitfun/design-tokens/typography-runtime';

export type FontSizeLevel = 'compact' | 'small' | 'default' | 'medium' | 'large' | 'custom';

export interface UiFontSizePreference {
  level: FontSizeLevel;
  /** Only used when level === 'custom'. Range: 12–20. */
  customPx?: number;
}

export interface FontPreference {
  uiSize: UiFontSizePreference;
}

export type FontSizeTokens = TypographySizeScale;

export type FontSizeLevelPresets = Record<Exclude<FontSizeLevel, 'custom'>, FontSizeTokens>;

/** UI baseline (px) for each preset level. Default = 14. */
export const PRESET_UI_BASE_PX: Record<Exclude<FontSizeLevel, 'custom'>, number> = {
  compact: 12,
  small: 13,
  default: 14,
  medium: 15,
  large: 16,
};

/** Preset levels share the canonical design-token runtime scale. */
export const UI_FONT_SIZE_PRESETS: FontSizeLevelPresets = {
  compact: createTypographySizeScale(PRESET_UI_BASE_PX.compact),
  small: createTypographySizeScale(PRESET_UI_BASE_PX.small),
  default: createTypographySizeScale(PRESET_UI_BASE_PX.default),
  medium: createTypographySizeScale(PRESET_UI_BASE_PX.medium),
  large: createTypographySizeScale(PRESET_UI_BASE_PX.large),
};

export function resolveFontSizeTokens(uiSize: UiFontSizePreference): FontSizeTokens {
  if (uiSize.level === 'custom') {
    return createTypographySizeScale(uiSize.customPx ?? 14);
  }
  return UI_FONT_SIZE_PRESETS[uiSize.level];
}

export const DEFAULT_FONT_PREFERENCE: FontPreference = {
  uiSize: { level: 'default' },
};

// ---- Events ----

export type FontPreferenceEventType = 'font:before-change' | 'font:after-change';

export interface FontPreferenceEvent {
  type: FontPreferenceEventType;
  preference: FontPreference;
  previousPreference?: FontPreference;
  timestamp: number;
}

export type FontPreferenceEventListener = (event: FontPreferenceEvent) => void | Promise<void>;
