/**
 * Editor preset configurations for different use cases.
 */

import type { EditorPresetConfig, EditorPresetName } from './types';

/** Readonly preset: chat code blocks, doc preview, code display */
export const PRESET_READONLY: EditorPresetConfig = {
  readOnly: true,
  contextmenu: false,
  links: true,
  folding: true,
  codeLens: false,
  
  minimap: {
    enabled: false,
    side: 'right',
    size: 'proportional',
  },
  lineNumbers: 'on',
  renderLineHighlight: 'none',
  
  formatOnSave: false,
  formatOnPaste: false,
  
  hover: {
    enabled: true,
    delay: 300,
    sticky: false,
    above: false,
  },
  inlayHints: {
    enabled: 'off',
    fontSize: 12,
    fontFamily: "'Fira Code', Consolas, 'Courier New', monospace",
    padding: false,
  },
};

/** Minimal preset: config editing, code snippets, small editors */
export const PRESET_MINIMAL: EditorPresetConfig = {
  readOnly: false,
  contextmenu: true,
  links: true,
  folding: false,
  codeLens: false,
  
  minimap: {
    enabled: false,
    side: 'right',
    size: 'proportional',
  },
  lineNumbers: 'on',
  
  semanticHighlighting: false,
  inlayHints: {
    enabled: 'off',
    fontSize: 12,
    fontFamily: "'Fira Code', Consolas, 'Courier New', monospace",
    padding: false,
  },
};

/** Standard preset: general code editing */
export const PRESET_STANDARD: EditorPresetConfig = {
  readOnly: false,
  contextmenu: true,
  links: true,
  folding: true,
  codeLens: true,
  
  minimap: {
    enabled: true,
    side: 'right',
    size: 'proportional',
  },
  lineNumbers: 'on',
};

/** Full preset: main file editor with all features */
export const PRESET_FULL: EditorPresetConfig = {
  readOnly: false,
  contextmenu: true,
  links: true,
  folding: true,
  codeLens: true,
  
  minimap: {
    enabled: true,
    side: 'right',
    size: 'proportional',
  },
  lineNumbers: 'on',
  semanticHighlighting: true,
  bracketPairColorization: true,
  
  hover: {
    enabled: true,
    delay: 100,
    sticky: true,
    above: false,
  },
  inlayHints: {
    enabled: 'on',
    fontSize: 12,
    fontFamily: "'Fira Code', Consolas, 'Courier New', monospace",
    padding: false,
  },
  guides: {
    indentation: true,
    bracketPairs: true,
    bracketPairsHorizontal: 'active',
    highlightActiveBracketPair: true,
    highlightActiveIndentation: true,
  },
};

/** Diff preset: code diff comparison */
export const PRESET_DIFF: EditorPresetConfig = {
  readOnly: false,
  contextmenu: false,
  links: true,
  folding: false,
  codeLens: false,
  
  minimap: {
    enabled: false,
    side: 'right',
    size: 'proportional',
  },
  lineNumbers: 'on',
  renderWhitespace: 'selection',
  scrollBeyondLastLine: false,
};

export const EDITOR_PRESETS: Record<EditorPresetName, EditorPresetConfig> = {
  readonly: PRESET_READONLY,
  minimal: PRESET_MINIMAL,
  standard: PRESET_STANDARD,
  full: PRESET_FULL,
  diff: PRESET_DIFF,
};

export function getPreset(presetName: EditorPresetName): EditorPresetConfig {
  return EDITOR_PRESETS[presetName];
}

export function getPresetNames(): EditorPresetName[] {
  return Object.keys(EDITOR_PRESETS) as EditorPresetName[];
}
