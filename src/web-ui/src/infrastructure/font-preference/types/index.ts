
export type FontSizeLevel = 'compact' | 'small' | 'default' | 'medium' | 'large' | 'custom';

export interface UiFontSizePreference {
  level: FontSizeLevel;
  /** Only used when level === 'custom'. Range: 12–20. */
  customPx?: number;
}

export type FlowChatFontMode = 'sync' | 'lift' | 'independent';

/**
 * Flow chat typographic scale vs global UI:
 * - `sync`: same tokens as UI
 * - `lift`: UI baseline + 1px (cap 20), default
 * - `independent`: custom baseline px
 */
export interface FlowChatFontSizePreference {
  mode: FlowChatFontMode;
  /** When `mode === 'independent'`, baseline px (12–20) for the flow-chat token ladder. */
  basePx?: number;
}

export interface FontPreference {
  uiSize: UiFontSizePreference;
  /** Conversation / flow chat panel font scale (separate from global UI). */
  flowChat: FlowChatFontSizePreference;
}

export interface FontSizeTokens {
  xs: string;
  sm: string;
  base: string;
  lg: string;
  xl: string;
  '2xl': string;
  '3xl': string;
  '4xl': string;
  '5xl': string;
}

export type FontSizeLevelPresets = Record<Exclude<FontSizeLevel, 'custom'>, FontSizeTokens>;

/** UI baseline (px) for each preset level. Default = 14. */
export const PRESET_UI_BASE_PX: Record<Exclude<FontSizeLevel, 'custom'>, number> = {
  compact: 12,
  small: 13,
  default: 14,
  medium: 15,
  large: 16,
};

/**
 * Derive font size tokens from a custom base px value.
 * Other steps are calculated relative to base with fixed offsets.
 */
export function deriveFontSizeTokens(basePx: number): FontSizeTokens {
  const b = Math.max(12, Math.min(20, basePx));
  return {
    xs:   `${b - 2}px`,
    sm:   `${b - 1}px`,
    base: `${b}px`,
    lg:   `${b + 1}px`,
    xl:   `${b + 2}px`,
    '2xl': `${b + 4}px`,
    '3xl': `${b + 8}px`,
    '4xl': `${b + 12}px`,
    '5xl': `${b + 18}px`,
  };
}

/** Preset levels share the same formula as custom UI size (deriveFontSizeTokens). */
export const UI_FONT_SIZE_PRESETS: FontSizeLevelPresets = {
  compact: deriveFontSizeTokens(PRESET_UI_BASE_PX.compact),
  small: deriveFontSizeTokens(PRESET_UI_BASE_PX.small),
  default: deriveFontSizeTokens(PRESET_UI_BASE_PX.default),
  medium: deriveFontSizeTokens(PRESET_UI_BASE_PX.medium),
  large: deriveFontSizeTokens(PRESET_UI_BASE_PX.large),
};

export function resolveFontSizeTokens(uiSize: UiFontSizePreference): FontSizeTokens {
  if (uiSize.level === 'custom') {
    return deriveFontSizeTokens(uiSize.customPx ?? 14);
  }
  return UI_FONT_SIZE_PRESETS[uiSize.level];
}

export function resolveFlowChatFontSizeTokens(pref: FontPreference): FontSizeTokens {
  if (pref.flowChat.mode === 'independent') {
    return deriveFontSizeTokens(pref.flowChat.basePx ?? 14);
  }
  if (pref.flowChat.mode === 'lift') {
    /*
     * "lift" used to add +1 to the UI baseline, which made every flow-chat font level
     * one step bigger than the surrounding UI. The user prefers the chat one notch
     * smaller, so we now leave the baseline as-is (effectively the same scale as `sync`,
     * but kept as a separate mode for compatibility with stored preferences).
     */
    const ui = resolveFontSizeTokens(pref.uiSize);
    const uiBase = parseInt(ui.base, 10);
    const bumped = Number.isNaN(uiBase) ? 14 : Math.min(20, uiBase);
    return deriveFontSizeTokens(bumped);
  }
  return resolveFontSizeTokens(pref.uiSize);
}

export const DEFAULT_FONT_PREFERENCE: FontPreference = {
  uiSize: { level: 'default' },
  flowChat: { mode: 'lift' },
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
