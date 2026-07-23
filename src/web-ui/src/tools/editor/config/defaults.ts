/**
 * Editor default configuration (single source of truth).
 */

import type { EditorConfig, MinimapConfig, GuidesConfig, ScrollbarConfig, HoverConfig, SuggestConfig, QuickSuggestionsConfig, InlayHintsConfig } from './types';

export const DEFAULT_MINIMAP_CONFIG: MinimapConfig = {
  enabled: true,
  side: 'right',
  size: 'proportional',
};

export const DEFAULT_GUIDES_CONFIG: GuidesConfig = {
  indentation: true,
  bracketPairs: true,
  bracketPairsHorizontal: 'active',
  highlightActiveBracketPair: true,
  highlightActiveIndentation: true,
};

export const DEFAULT_SCROLLBAR_CONFIG: ScrollbarConfig = {
  vertical: 'auto',
  horizontal: 'visible',
  verticalScrollbarSize: 10,
  horizontalScrollbarSize: 12,
  useShadows: false,
};

export const DEFAULT_HOVER_CONFIG: HoverConfig = {
  enabled: true,
  delay: 100,
  sticky: true,
  above: false,
};

export const DEFAULT_SUGGEST_CONFIG: SuggestConfig = {
  showKeywords: true,
  showSnippets: true,
  preview: true,
  showInlineDetails: true,
};

export const DEFAULT_QUICK_SUGGESTIONS_CONFIG: QuickSuggestionsConfig = {
  other: true,
  comments: false,
  strings: false,
};

export const DEFAULT_INLAY_HINTS_CONFIG: InlayHintsConfig = {
  enabled: 'on',
  fontSize: 12,
  fontFamily: "'Fira Code', Consolas, 'Courier New', monospace",
  padding: false,
};

export const DEFAULT_EDITOR_CONFIG: EditorConfig = {
  // Appearance
  fontSize: 14,
  fontFamily: "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace",
  fontWeight: 'normal',
  lineHeight: 1.5,
  theme: 'bitfun-dark',
  cursorStyle: 'line',
  cursorBlinking: 'smooth',
  renderWhitespace: 'selection',
  renderLineHighlight: 'line',

  // Behavior
  tabSize: 2,
  insertSpaces: true,
  wordWrap: 'off',
  autoSave: 'afterDelay',
  autoSaveDelay: 1000,
  scrollBeyondLastLine: false,
  smoothScrolling: true,

  // Features
  lineNumbers: 'on',
  minimap: { ...DEFAULT_MINIMAP_CONFIG },
  formatOnSave: false,
  formatOnPaste: false,
  trimAutoWhitespace: true,

  // Advanced
  semanticHighlighting: true,
  bracketPairColorization: true,
  guides: { ...DEFAULT_GUIDES_CONFIG },
  scrollbar: { ...DEFAULT_SCROLLBAR_CONFIG },
  hover: { ...DEFAULT_HOVER_CONFIG },
  suggest: { ...DEFAULT_SUGGEST_CONFIG },
  quickSuggestions: { ...DEFAULT_QUICK_SUGGESTIONS_CONFIG },
  inlayHints: { ...DEFAULT_INLAY_HINTS_CONFIG },
};

/** Deep merge configuration (source overrides target) */
export function mergeConfig<T extends Record<string, any>>(
  target: T,
  source: Partial<T> | undefined
): T {
  if (!source) {
    return target;
  }

  const result = { ...target };

  for (const key in source) {
    if (Object.prototype.hasOwnProperty.call(source, key)) {
      const sourceValue = source[key];
      const targetValue = target[key];

      if (
        sourceValue !== undefined &&
        typeof sourceValue === 'object' &&
        sourceValue !== null &&
        !Array.isArray(sourceValue) &&
        typeof targetValue === 'object' &&
        targetValue !== null &&
        !Array.isArray(targetValue)
      ) {
        (result as any)[key] = mergeConfig(targetValue, sourceValue);
      } else if (sourceValue !== undefined) {
        (result as any)[key] = sourceValue;
      }
    }
  }

  return result;
}

export function getFullConfig(partial?: Partial<EditorConfig>): EditorConfig {
  return mergeConfig(DEFAULT_EDITOR_CONFIG, partial);
}
