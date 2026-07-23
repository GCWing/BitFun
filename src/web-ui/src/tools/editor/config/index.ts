/**
 * Editor configuration module - types, defaults, and presets
 */

export type {
  EditorConfig,
  EditorConfigPartial,
  EditorPresetName,
  EditorPresetConfig,
  MinimapConfig,
  GuidesConfig,
  ScrollbarConfig,
  HoverConfig,
  SuggestConfig,
  QuickSuggestionsConfig,
  InlayHintsConfig,
  DeepPartial,
  EditorConfigChangeEvent,
} from './types';

export {
  DEFAULT_EDITOR_CONFIG,
  DEFAULT_MINIMAP_CONFIG,
  DEFAULT_GUIDES_CONFIG,
  DEFAULT_SCROLLBAR_CONFIG,
  DEFAULT_HOVER_CONFIG,
  DEFAULT_SUGGEST_CONFIG,
  DEFAULT_QUICK_SUGGESTIONS_CONFIG,
  DEFAULT_INLAY_HINTS_CONFIG,
  mergeConfig,
  getFullConfig,
} from './defaults';

export {
  PRESET_READONLY,
  PRESET_MINIMAL,
  PRESET_STANDARD,
  PRESET_FULL,
  PRESET_DIFF,
  EDITOR_PRESETS,
  getPreset,
  getPresetNames,
} from './presets';
