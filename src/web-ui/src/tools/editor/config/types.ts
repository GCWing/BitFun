/**
 * Editor configuration types.
 * Uses camelCase (Tauri convention); auto-converted to snake_case for backend.
 */

export interface MinimapConfig {
  enabled: boolean;
  side: 'left' | 'right';
  size: 'proportional' | 'fill' | 'fit';
}

export interface GuidesConfig {
  indentation: boolean;
  bracketPairs: boolean;
  bracketPairsHorizontal: 'active' | 'true' | 'false';
  highlightActiveBracketPair: boolean;
  highlightActiveIndentation: boolean;
}

export interface ScrollbarConfig {
  vertical: 'auto' | 'visible' | 'hidden';
  horizontal: 'auto' | 'visible' | 'hidden';
  verticalScrollbarSize: number;
  horizontalScrollbarSize: number;
  useShadows: boolean;
}

export interface HoverConfig {
  enabled: boolean;
  delay: number;
  sticky: boolean;
  above: boolean;
}

export interface SuggestConfig {
  showKeywords: boolean;
  showSnippets: boolean;
  preview: boolean;
  showInlineDetails: boolean;
}

export interface QuickSuggestionsConfig {
  other: boolean;
  comments: boolean;
  strings: boolean;
}

export interface InlayHintsConfig {
  enabled: 'on' | 'off' | 'offUnlessPressed' | 'onUnlessPressed';
  fontSize: number;
  fontFamily: string;
  padding: boolean;
}

/**
 * Full editor configuration.
 */
export interface EditorConfig {
  // Appearance
  fontSize: number;
  fontFamily: string;
  fontWeight: 'normal' | 'bold';
  /** Line height multiplier */
  lineHeight: number;
  theme: string;
  cursorStyle: 'line' | 'block' | 'underline' | 'line-thin' | 'block-outline' | 'underline-thin';
  cursorBlinking: 'blink' | 'smooth' | 'phase' | 'expand' | 'solid';
  renderWhitespace: 'none' | 'boundary' | 'selection' | 'trailing' | 'all';
  renderLineHighlight: 'none' | 'gutter' | 'line' | 'all';

  // Behavior
  tabSize: number;
  insertSpaces: boolean;
  wordWrap: 'off' | 'on' | 'wordWrapColumn' | 'bounded';
  autoSave: 'off' | 'afterDelay' | 'onFocusChange' | 'onWindowChange';
  autoSaveDelay: number;
  scrollBeyondLastLine: boolean;
  smoothScrolling: boolean;

  // Features
  lineNumbers: 'on' | 'off' | 'relative' | 'interval';
  minimap: MinimapConfig;
  formatOnSave: boolean;
  formatOnPaste: boolean;
  trimAutoWhitespace: boolean;

  // Advanced
  semanticHighlighting: boolean;
  bracketPairColorization: boolean;
  guides: GuidesConfig;
  scrollbar: ScrollbarConfig;
  hover: HoverConfig;
  suggest: SuggestConfig;
  quickSuggestions: QuickSuggestionsConfig;
  inlayHints: InlayHintsConfig;
}

/** Partial editor config for overrides */
export type EditorConfigPartial = Partial<EditorConfig> & {
  minimap?: Partial<MinimapConfig>;
  guides?: Partial<GuidesConfig>;
  scrollbar?: Partial<ScrollbarConfig>;
  hover?: Partial<HoverConfig>;
  suggest?: Partial<SuggestConfig>;
  quickSuggestions?: Partial<QuickSuggestionsConfig>;
  inlayHints?: Partial<InlayHintsConfig>;
};

export type EditorPresetName = 'readonly' | 'minimal' | 'standard' | 'full' | 'diff';

/** Preset config including runtime properties */
export interface EditorPresetConfig extends EditorConfigPartial {
  readOnly?: boolean;
  enableLsp?: boolean;
  contextmenu?: boolean;
  links?: boolean;
  folding?: boolean;
  codeLens?: boolean;
}

export type DeepPartial<T> = {
  [P in keyof T]?: T[P] extends object ? DeepPartial<T[P]> : T[P];
};

export interface EditorConfigChangeEvent {
  previousConfig: EditorConfig;
  currentConfig: EditorConfig;
  changedKeys: (keyof EditorConfig)[];
}
