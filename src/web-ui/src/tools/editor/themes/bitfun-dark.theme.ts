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
    'editor.background': '#121214',
    'editor.foreground': '#D6DEEB',

    // Line Numbers
    'editorLineNumber.foreground': '#707070',
    'editorLineNumber.activeForeground': '#E1AB80',
    'editorLineNumber.dimmedForeground': '#454545',

    // Cursor and Selection
    'editorCursor.foreground': '#E1AB80',
    'editorCursor.background': '#121214',
    'editor.selectionBackground': '#E1AB8040',
    'editor.selectionForeground': '#ffffff',
    'editor.inactiveSelectionBackground': '#E1AB8020',
    'editor.selectionHighlightBackground': '#E1AB8030',
    'editor.selectionHighlightBorder': '#E1AB80',

    // Current Line Highlight
    'editor.lineHighlightBackground': '#18181a',
    'editor.lineHighlightBorder': '#202024',

    // Find and Match
    'editor.findMatchBackground': '#E1AB80',
    'editor.findMatchHighlightBackground': '#E1AB8040',
    'editor.findRangeHighlightBackground': '#E1AB8020',
    'editor.findMatchBorder': '#F6D0A3',
    'editor.findMatchHighlightBorder': '#E1AB8080',

    // Word Highlight
    'editor.wordHighlightBackground': '#E1AB8020',
    'editor.wordHighlightStrongBackground': '#E1AB8040',
    'editor.wordHighlightBorder': '#E1AB8060',
    'editor.wordHighlightStrongBorder': '#E1AB80',

    // Code Highlight and Decorations
    'editor.hoverHighlightBackground': '#E1AB8020',
    'editor.symbolHighlightBackground': '#E1AB8020',
    'editor.symbolHighlightBorder': '#E1AB8060',

    // Indent Guides and Rulers
    'editorIndentGuide.background': '#202024',
    'editorIndentGuide.activeBackground': '#E1AB8060',
    'editorRuler.foreground': '#202024',

    // Bracket Matching
    'editorBracketMatch.background': '#E1AB8030',
    'editorBracketMatch.border': '#E1AB80',
    'editorBracketHighlight.foreground1': '#ffd700',
    'editorBracketHighlight.foreground2': '#E1AB80',
    'editorBracketHighlight.foreground3': '#C792EA',
    'editorBracketHighlight.foreground4': '#4ECDC4',
    'editorBracketHighlight.foreground5': '#F78C6C',
    'editorBracketHighlight.foreground6': '#A5E844',

    // Suggest Widget
    'editorSuggestWidget.background': '#18181a',
    'editorSuggestWidget.border': '#E1AB80',
    'editorSuggestWidget.foreground': '#E0E6F0',
    'editorSuggestWidget.highlightForeground': '#E1AB80',
    'editorSuggestWidget.selectedBackground': '#E1AB8030',
    'editorSuggestWidget.focusHighlightForeground': '#A5E844',

    // Hover Widget
    'editorHoverWidget.background': '#18181a',
    'editorHoverWidget.border': '#E1AB80',
    'editorHoverWidget.foreground': '#E0E6F0',
    'editorHoverWidget.statusBarBackground': '#202024',

    // Inlay Hints
    'editorInlayHint.background': TRANSPARENT_MONACO_BORDER,
    'editorInlayHint.foreground': '#6A737D',
    'editorInlayHint.typeForeground': '#6A737D',
    'editorInlayHint.parameterForeground': '#6A737D',

    // Errors and Warnings
    'editorError.foreground': '#FF5370',
    'editorWarning.foreground': '#FFCB6B',
    'editorInfo.foreground': '#82AAFF',
    'editorHint.foreground': '#6A737D',

    // Scrollbar
    'scrollbar.shadow': '#121214',
    'scrollbarSlider.background': '#E1AB8040',
    'scrollbarSlider.hoverBackground': '#E1AB8070',
    'scrollbarSlider.activeBackground': '#E1AB80A0',

    // Minimap
    'minimap.background': '#121214',
    'minimap.selectionHighlight': '#E1AB8040',
    'minimap.findMatchHighlight': '#E1AB80',
    'minimap.errorHighlight': '#FF5370',
    'minimap.warningHighlight': '#FFCB6B',
    'minimapSlider.background': '#E1AB8040',
    'minimapSlider.hoverBackground': '#E1AB8070',
    'minimapSlider.activeBackground': '#E1AB80A0',

    // Widget Borders
    'editorWidget.background': '#18181a',
    'editorWidget.border': '#E1AB8040',
    'editorWidget.foreground': '#D6DEEB',
    'editorWidget.resizeBorder': '#E1AB8060',

    // Code Lens
    'editorCodeLens.foreground': '#6A737D',

    // Links
    'editorLink.activeForeground': '#7DCFFF',

    // Whitespace
    'editorWhitespace.foreground': '#3A4A5A',

    // Overview Ruler
    'editorOverviewRuler.border': '#18181a',
    'editorOverviewRuler.background': '#121214',
    'editorOverviewRuler.currentContentForeground': '#E1AB8080',
    'editorOverviewRuler.incomingContentForeground': '#7FDBCA80',
    'editorOverviewRuler.findMatchForeground': '#FFCB6B80',
    'editorOverviewRuler.rangeHighlightForeground': '#E1AB8040',
    'editorOverviewRuler.selectionHighlightForeground': '#E1AB8060',
    'editorOverviewRuler.wordHighlightForeground': '#C792EA60',
    'editorOverviewRuler.modifiedForeground': '#FFCB6B',
    'editorOverviewRuler.addedForeground': '#ADDB67',
    'editorOverviewRuler.deletedForeground': '#FF5370',
    'editorOverviewRuler.errorForeground': '#FF5370',
    'editorOverviewRuler.warningForeground': '#FFCB6B',
    'editorOverviewRuler.infoForeground': '#E1AB80',

    // Diff Editor (GitHub Dark style)
    'diffEditor.insertedTextBackground': '#23863625',
    'diffEditor.insertedLineBackground': '#23863630',
    'diffEditor.insertedTextBorder': TRANSPARENT_MONACO_BORDER,
    'diffEditorGutter.insertedLineBackground': '#23863638',

    'diffEditor.removedTextBackground': '#DA363325',
    'diffEditor.removedLineBackground': '#DA363330',
    'diffEditor.removedTextBorder': TRANSPARENT_MONACO_BORDER,
    'diffEditorGutter.removedLineBackground': '#DA363338',

    'diffEditor.modifiedTextBackground': '#1F6FEB20',
    'diffEditor.modifiedLineBackground': '#1F6FEB28',

    'diffEditor.border': '#2A2D35',
    'diffEditor.diagonalFill': '#16181D',
    'diffEditor.unchangedRegionBackground': '#0D0D0F',
    'diffEditor.unchangedCodeBackground': '#0D0D0F',

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
