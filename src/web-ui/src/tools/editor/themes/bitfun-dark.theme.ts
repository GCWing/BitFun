/**
 * BitFun Dark Theme Definition
 * Custom Monaco Editor Theme
 *
 * Design Philosophy:
 * - Deep background with premium vibrant colors
 * - High saturation, modern color palette
 * - Carefully balanced multi-color scheme
 * - Excellent contrast and distinction between syntax elements
 * - Consistent with BitFun UI style
 * - Inspired by Night Owl, Tokyo Night themes
 */

import type { editor } from 'monaco-editor';

const TRANSPARENT_MONACO_BORDER = '#00000000';

const MONACO_DARK_SURFACE = {
  background: '#121214',
  elevated: '#18181a',
  borderSubtle: '#202024',
  diffDeep: '#121214',
} as const;

const MONACO_EDITOR_TEXT = {
  primary: '#D6DEEB',
  secondary: '#E0E6F0',
  muted: '#6A737D',
} as const;

const MONACO_BRAND_ACCENT = {
  base: '#E1AB80',
  light: '#F6D0A3',
  alpha20: '#E1AB8020',
  alpha30: '#E1AB8030',
  alpha40: '#E1AB8040',
  alpha60: '#E1AB8060',
  alpha70: '#E1AB8070',
  alpha80: '#E1AB8080',
  alphaA0: '#E1AB80A0',
} as const;

const MONACO_STATUS_COLOR = {
  error: '#FF5370',
  warning: '#FFCB6B',
  info: '#82AAFF',
  success: '#ADDB67',
  link: '#7DCFFF',
} as const;

const MONACO_DIFF_COLOR = {
  insertedTextBackground: '#23863625',
  insertedLineBackground: '#23863630',
  insertedGutterBackground: '#23863638',
  removedTextBackground: '#DA363325',
  removedLineBackground: '#DA363330',
  removedGutterBackground: '#DA363338',
  modifiedTextBackground: '#1F6FEB20',
  modifiedLineBackground: '#1F6FEB28',
} as const;

/**
 * BitFun Dark Theme Configuration
 * Follows Monaco Editor official theme format
 * @see https://microsoft.github.io/monaco-editor/api/interfaces/monaco.editor.IStandaloneThemeData.html
 */
export const BitFunDarkTheme: editor.IStandaloneThemeData = {
  base: 'vs-dark',
  inherit: true,

  rules: [
    // Comments
    { token: 'comment', foreground: '6A737D', fontStyle: 'italic' },
    { token: 'comment.line', foreground: '6A737D', fontStyle: 'italic' },
    { token: 'comment.block', foreground: '6A737D', fontStyle: 'italic' },
    { token: 'comment.doc', foreground: '6A737D', fontStyle: 'italic' },

    // Keywords
    { token: 'keyword', foreground: 'C792EA' },
    { token: 'keyword.control', foreground: 'C792EA' },
    { token: 'keyword.control.import', foreground: 'C792EA' },
    { token: 'keyword.control.export', foreground: 'C792EA' },
    { token: 'keyword.control.from', foreground: 'C792EA' },
    { token: 'keyword.operator', foreground: 'C792EA' },
    { token: 'keyword.operator.new', foreground: 'C792EA' },
    { token: 'keyword.other', foreground: 'C792EA' },

    // Strings
    { token: 'string', foreground: 'A5E844' },
    { token: 'string.quoted', foreground: 'A5E844' },
    { token: 'string.template', foreground: 'A5E844' },
    { token: 'string.regexp', foreground: 'A5E844' },

    // Numbers
    { token: 'number', foreground: 'F78C6C' },
    { token: 'number.hex', foreground: 'F78C6C' },
    { token: 'number.binary', foreground: 'F78C6C' },
    { token: 'number.octal', foreground: 'F78C6C' },
    { token: 'number.float', foreground: 'F78C6C' },

    // Functions and Methods
    { token: 'function', foreground: '7DCFFF' },
    { token: 'function.call', foreground: '7DCFFF' },
    { token: 'method', foreground: '7DCFFF' },
    { token: 'method.call', foreground: '7DCFFF' },
    { token: 'entity.name.function', foreground: '7DCFFF' },
    { token: 'support.function', foreground: '7DCFFF' },

    // Classes and Types
    { token: 'class', foreground: '4ECDC4' },
    { token: 'class.name', foreground: '4ECDC4' },
    { token: 'entity.name.class', foreground: '4ECDC4' },
    { token: 'entity.name.type.class', foreground: '4ECDC4' },
    { token: 'type', foreground: 'FFC777' },
    { token: 'type.identifier', foreground: 'FFC777' },
    { token: 'entity.name.type', foreground: 'FFC777' },
    { token: 'entity.other.inherited-class', foreground: '4ECDC4', fontStyle: 'italic' },
    { token: 'interface', foreground: '4ECDC4' },
    { token: 'entity.name.interface', foreground: '4ECDC4' },
    { token: 'enum', foreground: '73DACA' },
    { token: 'entity.name.enum', foreground: '73DACA' },
    { token: 'struct', foreground: '4ECDC4' },
    { token: 'entity.name.struct', foreground: '4ECDC4' },

    // Packages and Namespaces
    { token: 'namespace', foreground: '7AA2F7' },
    { token: 'entity.name.namespace', foreground: '7AA2F7' },
    { token: 'entity.name.package', foreground: '7AA2F7' },
    { token: 'entity.name.module', foreground: '7AA2F7' },
    { token: 'support.type.package', foreground: '7AA2F7' },

    // Variables
    { token: 'variable', foreground: '80D4FF' },
    { token: 'variable.name', foreground: '80D4FF' },
    { token: 'variable.parameter', foreground: 'E0E6F0' },
    { token: 'variable.other', foreground: '80D4FF' },
    { token: 'variable.language', foreground: 'C792EA', fontStyle: 'italic' },
    { token: 'variable.other.readwrite', foreground: '80D4FF' },
    { token: 'variable.other.property', foreground: '80D4FF' },
    { token: 'variable.other.constant', foreground: 'BB9AF7' },

    // Constants
    { token: 'constant', foreground: 'BB9AF7' },
    { token: 'constant.language', foreground: 'C792EA' },
    { token: 'constant.numeric', foreground: 'F78C6C' },
    { token: 'constant.character', foreground: 'A5E844' },

    // Operators and Punctuation
    { token: 'operator', foreground: 'C792EA' },
    { token: 'delimiter', foreground: 'E0E6F0' },
    { token: 'delimiter.bracket', foreground: '89DDFF' },
    { token: 'delimiter.parenthesis', foreground: '89DDFF' },
    { token: 'delimiter.square', foreground: '89DDFF' },

    // Tags (HTML/XML)
    { token: 'tag', foreground: '4ECDC4' },
    { token: 'tag.name', foreground: '4ECDC4' },
    { token: 'tag.attribute', foreground: 'C792EA', fontStyle: 'italic' },
    { token: 'tag.delimiter', foreground: '565F89' },

    // Special Tokens
    { token: 'annotation', foreground: 'FFC777' },
    { token: 'decorator', foreground: 'FFC777' },
    { token: 'attribute', foreground: 'C792EA', fontStyle: 'italic' },
    { token: 'meta', foreground: '7DCFFF' },
    { token: 'regexp', foreground: 'A5E844' },

    // Language-Specific: TypeScript/JavaScript
    { token: 'support.type.primitive', foreground: 'FFC777' },
    { token: 'support.type.builtin', foreground: 'FFC777' },
    { token: 'support.class', foreground: '4ECDC4' },
    { token: 'support.type.object', foreground: '4ECDC4' },
    { token: 'meta.import', foreground: 'C792EA' },
    { token: 'meta.export', foreground: 'C792EA' },

    // Language-Specific: Python
    { token: 'support.type.python', foreground: 'FFC777' },
    { token: 'meta.function.decorator.python', foreground: 'FFC777' },

    // Language-Specific: Java/C#
    { token: 'storage.modifier', foreground: 'C792EA', fontStyle: 'italic' },
    { token: 'storage.type', foreground: 'FFC777' },
    { token: 'meta.import.java', foreground: 'C792EA' },
    { token: 'storage.modifier.package.java', foreground: '7AA2F7' },
    { token: 'storage.modifier.import.java', foreground: 'C792EA' },

    // Language-Specific: C/C++
    { token: 'storage.type.built-in', foreground: 'FFC777' },
    { token: 'entity.name.type.typedef', foreground: 'FFC777' },
    { token: 'meta.preprocessor', foreground: 'C792EA', fontStyle: 'italic' },
    { token: 'keyword.control.directive', foreground: 'C792EA' },

    // Language-Specific: Rust
    { token: 'entity.name.type.rust', foreground: '4ECDC4' },
    { token: 'storage.type.rust', foreground: 'FFC777' },
    { token: 'support.type.primitive.rust', foreground: 'FFC777' },
    { token: 'entity.name.type.trait.rust', foreground: '4ECDC4' },

    // Language-Specific: Go
    { token: 'entity.name.package.go', foreground: '7AA2F7' },
    { token: 'storage.type.go', foreground: 'FFC777' },

    // Language-Specific: CSS
    { token: 'support.type.property-name', foreground: '80D4FF' },
    { token: 'entity.other.attribute-name', foreground: 'C792EA', fontStyle: 'italic' },

    // Language-Specific: Markdown
    { token: 'markup.heading', foreground: '7DCFFF' },
    { token: 'markup.bold', foreground: 'FFC777', fontStyle: 'bold' },
    { token: 'markup.italic', foreground: 'A5E844', fontStyle: 'italic' },
    { token: 'markup.underline', foreground: '80D4FF', fontStyle: 'underline' },
    { token: 'markup.quote', foreground: '6A737D', fontStyle: 'italic' },
    { token: 'markup.inline.raw', foreground: 'A5E844' },
    { token: 'markup.list', foreground: 'C792EA' },
    { token: 'markup.link', foreground: '7DCFFF', fontStyle: 'underline' },

    // Language-Specific: JSON
    { token: 'support.type.property-name.json', foreground: '80D4FF' },
    { token: 'string.key.json', foreground: '80D4FF' },
    { token: 'string.value.json', foreground: 'A5E844' },

    // Language-Specific: TOML
    { token: 'type.identifier.toml', foreground: 'FFC777' },
    { token: 'key.toml', foreground: '80D4FF' },
    { token: 'operator.toml', foreground: 'C792EA' },
    { token: 'string.toml', foreground: 'A5E844' },
    { token: 'string.quote.toml', foreground: 'A5E844' },
    { token: 'string.escape.toml', foreground: 'C792EA' },
    { token: 'string.invalid.toml', foreground: 'FF5370' },
    { token: 'number.toml', foreground: 'F78C6C' },
    { token: 'number.date.toml', foreground: 'F78C6C' },
    { token: 'number.float.toml', foreground: 'F78C6C' },
    { token: 'number.hex.toml', foreground: 'F78C6C' },
    { token: 'number.octal.toml', foreground: 'F78C6C' },
    { token: 'number.binary.toml', foreground: 'F78C6C' },
    { token: 'keyword.toml', foreground: 'C792EA' },
    { token: 'comment.toml', foreground: '6A737D', fontStyle: 'italic' },
    { token: 'delimiter.curly.toml', foreground: '89DDFF' },
    { token: 'delimiter.square.toml', foreground: '89DDFF' },
    { token: 'delimiter.bracket.toml', foreground: '89DDFF' },
    { token: 'delimiter.parenthesis.toml', foreground: '89DDFF' },
    { token: 'delimiter.comma.toml', foreground: 'E0E6F0' },
    { token: 'delimiter.dot.toml', foreground: 'E0E6F0' },

    // Semantic Tokens (LSP)
    { token: 'namespace', foreground: '7AA2F7' },
    { token: 'class', foreground: '4ECDC4' },
    { token: 'enum', foreground: '73DACA' },
    { token: 'interface', foreground: '4ECDC4' },
    { token: 'struct', foreground: '4ECDC4' },
    { token: 'typeParameter', foreground: 'FFC777' },
    { token: 'type', foreground: 'FFC777' },
    { token: 'parameter', foreground: 'E0E6F0' },
    { token: 'variable', foreground: '80D4FF' },
    { token: 'property', foreground: '80D4FF' },
    { token: 'enumMember', foreground: 'BB9AF7' },
    { token: 'event', foreground: 'FFC777' },
    { token: 'function', foreground: '7DCFFF' },
    { token: 'method', foreground: '7DCFFF' },
    { token: 'macro', foreground: '73DACA' },
    { token: 'keyword', foreground: 'C792EA' },
    { token: 'modifier', foreground: 'C792EA' },
    { token: 'comment', foreground: '6A737D' },
    { token: 'string', foreground: 'A5E844' },
    { token: 'number', foreground: 'F78C6C' },
    { token: 'regexp', foreground: 'A5E844' },
    { token: 'operator', foreground: 'C792EA' },
    { token: 'decorator', foreground: 'FFC777' },
    { token: 'label', foreground: 'C792EA' },
  ],

  colors: {
    // Global Border
    'focusBorder': TRANSPARENT_MONACO_BORDER,
    'contrastBorder': TRANSPARENT_MONACO_BORDER,

    // Editor Body
    'editor.background': MONACO_DARK_SURFACE.background,
    'editor.foreground': MONACO_EDITOR_TEXT.primary,

    // Line Numbers
    'editorLineNumber.foreground': '#707070',
    'editorLineNumber.activeForeground': MONACO_BRAND_ACCENT.base,
    'editorLineNumber.dimmedForeground': '#454545',

    // Cursor and Selection
    'editorCursor.foreground': MONACO_BRAND_ACCENT.base,
    'editorCursor.background': MONACO_DARK_SURFACE.background,
    'editor.selectionBackground': MONACO_BRAND_ACCENT.alpha40,
    'editor.selectionForeground': '#ffffff',
    'editor.inactiveSelectionBackground': MONACO_BRAND_ACCENT.alpha20,
    'editor.selectionHighlightBackground': MONACO_BRAND_ACCENT.alpha30,
    'editor.selectionHighlightBorder': MONACO_BRAND_ACCENT.base,

    // Current Line Highlight
    'editor.lineHighlightBackground': MONACO_DARK_SURFACE.elevated,
    'editor.lineHighlightBorder': MONACO_DARK_SURFACE.borderSubtle,

    // Find and Match
    'editor.findMatchBackground': MONACO_BRAND_ACCENT.base,
    'editor.findMatchHighlightBackground': MONACO_BRAND_ACCENT.alpha40,
    'editor.findRangeHighlightBackground': MONACO_BRAND_ACCENT.alpha20,
    'editor.findMatchBorder': MONACO_BRAND_ACCENT.light,
    'editor.findMatchHighlightBorder': MONACO_BRAND_ACCENT.alpha80,

    // Word Highlight
    'editor.wordHighlightBackground': MONACO_BRAND_ACCENT.alpha20,
    'editor.wordHighlightStrongBackground': MONACO_BRAND_ACCENT.alpha40,
    'editor.wordHighlightBorder': MONACO_BRAND_ACCENT.alpha60,
    'editor.wordHighlightStrongBorder': MONACO_BRAND_ACCENT.base,

    // Code Highlight and Decorations
    'editor.hoverHighlightBackground': MONACO_BRAND_ACCENT.alpha20,
    'editor.symbolHighlightBackground': MONACO_BRAND_ACCENT.alpha20,
    'editor.symbolHighlightBorder': MONACO_BRAND_ACCENT.alpha60,

    // Indent Guides and Rulers
    'editorIndentGuide.background': MONACO_DARK_SURFACE.borderSubtle,
    'editorIndentGuide.activeBackground': MONACO_BRAND_ACCENT.alpha60,
    'editorRuler.foreground': MONACO_DARK_SURFACE.borderSubtle,

    // Bracket Matching
    'editorBracketMatch.background': MONACO_BRAND_ACCENT.alpha30,
    'editorBracketMatch.border': MONACO_BRAND_ACCENT.base,
    'editorBracketHighlight.foreground1': '#ffd700',
    'editorBracketHighlight.foreground2': MONACO_BRAND_ACCENT.base,
    'editorBracketHighlight.foreground3': '#C792EA',
    'editorBracketHighlight.foreground4': '#4ECDC4',
    'editorBracketHighlight.foreground5': '#F78C6C',
    'editorBracketHighlight.foreground6': '#A5E844',

    // Suggest Widget
    'editorSuggestWidget.background': MONACO_DARK_SURFACE.elevated,
    'editorSuggestWidget.border': MONACO_BRAND_ACCENT.base,
    'editorSuggestWidget.foreground': MONACO_EDITOR_TEXT.secondary,
    'editorSuggestWidget.highlightForeground': MONACO_BRAND_ACCENT.base,
    'editorSuggestWidget.selectedBackground': MONACO_BRAND_ACCENT.alpha30,
    'editorSuggestWidget.focusHighlightForeground': '#A5E844',

    // Hover Widget
    'editorHoverWidget.background': MONACO_DARK_SURFACE.elevated,
    'editorHoverWidget.border': MONACO_BRAND_ACCENT.base,
    'editorHoverWidget.foreground': MONACO_EDITOR_TEXT.secondary,
    'editorHoverWidget.statusBarBackground': MONACO_DARK_SURFACE.borderSubtle,

    // Inlay Hints
    'editorInlayHint.background': TRANSPARENT_MONACO_BORDER,
    'editorInlayHint.foreground': MONACO_EDITOR_TEXT.muted,
    'editorInlayHint.typeForeground': MONACO_EDITOR_TEXT.muted,
    'editorInlayHint.parameterForeground': MONACO_EDITOR_TEXT.muted,

    // Errors and Warnings
    'editorError.foreground': MONACO_STATUS_COLOR.error,
    'editorWarning.foreground': MONACO_STATUS_COLOR.warning,
    'editorInfo.foreground': MONACO_STATUS_COLOR.info,
    'editorHint.foreground': MONACO_EDITOR_TEXT.muted,

    // Scrollbar
    'scrollbar.shadow': MONACO_DARK_SURFACE.background,
    'scrollbarSlider.background': MONACO_BRAND_ACCENT.alpha40,
    'scrollbarSlider.hoverBackground': MONACO_BRAND_ACCENT.alpha70,
    'scrollbarSlider.activeBackground': MONACO_BRAND_ACCENT.alphaA0,

    // Minimap
    'minimap.background': MONACO_DARK_SURFACE.background,
    'minimap.selectionHighlight': MONACO_BRAND_ACCENT.alpha40,
    'minimap.findMatchHighlight': MONACO_BRAND_ACCENT.base,
    'minimap.errorHighlight': MONACO_STATUS_COLOR.error,
    'minimap.warningHighlight': MONACO_STATUS_COLOR.warning,
    'minimapSlider.background': MONACO_BRAND_ACCENT.alpha40,
    'minimapSlider.hoverBackground': MONACO_BRAND_ACCENT.alpha70,
    'minimapSlider.activeBackground': MONACO_BRAND_ACCENT.alphaA0,

    // Widget Borders
    'editorWidget.background': MONACO_DARK_SURFACE.elevated,
    'editorWidget.border': MONACO_BRAND_ACCENT.alpha40,
    'editorWidget.foreground': MONACO_EDITOR_TEXT.primary,
    'editorWidget.resizeBorder': MONACO_BRAND_ACCENT.alpha60,

    // Code Lens
    'editorCodeLens.foreground': MONACO_EDITOR_TEXT.muted,

    // Links
    'editorLink.activeForeground': MONACO_STATUS_COLOR.link,

    // Whitespace
    'editorWhitespace.foreground': '#3A4A5A',

    // Overview Ruler
    'editorOverviewRuler.border': MONACO_DARK_SURFACE.elevated,
    'editorOverviewRuler.background': MONACO_DARK_SURFACE.background,
    'editorOverviewRuler.currentContentForeground': MONACO_BRAND_ACCENT.alpha80,
    'editorOverviewRuler.incomingContentForeground': '#7FDBCA80',
    'editorOverviewRuler.findMatchForeground': '#FFCB6B80',
    'editorOverviewRuler.rangeHighlightForeground': MONACO_BRAND_ACCENT.alpha40,
    'editorOverviewRuler.selectionHighlightForeground': MONACO_BRAND_ACCENT.alpha60,
    'editorOverviewRuler.wordHighlightForeground': '#C792EA60',
    'editorOverviewRuler.modifiedForeground': MONACO_STATUS_COLOR.warning,
    'editorOverviewRuler.addedForeground': MONACO_STATUS_COLOR.success,
    'editorOverviewRuler.deletedForeground': MONACO_STATUS_COLOR.error,
    'editorOverviewRuler.errorForeground': MONACO_STATUS_COLOR.error,
    'editorOverviewRuler.warningForeground': MONACO_STATUS_COLOR.warning,
    'editorOverviewRuler.infoForeground': MONACO_BRAND_ACCENT.base,

    // Diff Editor (GitHub Dark style)
    'diffEditor.insertedTextBackground': MONACO_DIFF_COLOR.insertedTextBackground,
    'diffEditor.insertedLineBackground': MONACO_DIFF_COLOR.insertedLineBackground,
    'diffEditor.insertedTextBorder': TRANSPARENT_MONACO_BORDER,
    'diffEditorGutter.insertedLineBackground': MONACO_DIFF_COLOR.insertedGutterBackground,

    'diffEditor.removedTextBackground': MONACO_DIFF_COLOR.removedTextBackground,
    'diffEditor.removedLineBackground': MONACO_DIFF_COLOR.removedLineBackground,
    'diffEditor.removedTextBorder': TRANSPARENT_MONACO_BORDER,
    'diffEditorGutter.removedLineBackground': MONACO_DIFF_COLOR.removedGutterBackground,

    'diffEditor.modifiedTextBackground': MONACO_DIFF_COLOR.modifiedTextBackground,
    'diffEditor.modifiedLineBackground': MONACO_DIFF_COLOR.modifiedLineBackground,

    'diffEditor.border': '#2A2D35',
    'diffEditor.diagonalFill': MONACO_DARK_SURFACE.elevated,
    'diffEditor.unchangedRegionBackground': MONACO_DARK_SURFACE.diffDeep,
    'diffEditor.unchangedCodeBackground': MONACO_DARK_SURFACE.diffDeep,

    'diffEditorOverview.insertedForeground': '#3FB950',
    'diffEditorOverview.removedForeground': '#F85149',
  }
};

export const BitFunDarkThemeMetadata = {
  id: 'bitfun-dark',
  label: 'Dark',
  description: 'Premium vibrant dark theme with modern multi-color palette',
  author: 'BitFun Team',
  version: '2.0.0',
};
