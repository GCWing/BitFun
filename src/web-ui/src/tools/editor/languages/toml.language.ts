/**
 * TOML Language Definition for Monaco Editor
 * 
 * Supports syntax highlighting for TOML config files (including Cargo.toml).
 * @module languages/toml
 */

import type * as Monaco from 'monaco-editor';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('toml.language');

/** TOML language configuration */
export const tomlLanguageConfig: Monaco.languages.LanguageConfiguration = {
  comments: {
    lineComment: '#',
  },
  brackets: [
    ['{', '}'],
    ['[', ']'],
    ['(', ')'],
  ],
  autoClosingPairs: [
    { open: '{', close: '}' },
    { open: '[', close: ']' },
    { open: '(', close: ')' },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
  ],
  surroundingPairs: [
    { open: '{', close: '}' },
    { open: '[', close: ']' },
    { open: '(', close: ')' },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
  ],
};

/** TOML language Monarch tokenizer definition */
export const tomlLanguageTokens: Monaco.languages.IMonarchLanguage = {
  defaultToken: '',
  keywords: ['true', 'false'],
  operators: ['='],
  brackets: [
    { open: '{', close: '}', token: 'delimiter.curly' },
    { open: '[', close: ']', token: 'delimiter.square' },
    { open: '(', close: ')', token: 'delimiter.parenthesis' },
  ],
  
  tokenizer: {
    root: [
      [/#.*$/, 'comment'],
      [/^\s*\[([^\]]+)\]/, 'type.identifier'],
      [/\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})?)?/, 'number.date'],
      [/[+-]?\d+\.\d+([eE][+-]?\d+)?/, 'number.float'],
      [/[+-]?\d+([eE][+-]?\d+)?/, 'number'],
      [/0x[0-9a-fA-F_]+/, 'number.hex'],
      [/0o[0-7_]+/, 'number.octal'],
      [/0b[01_]+/, 'number.binary'],
      [/\b(true|false)\b/, 'keyword'],
      [/^\s*([a-zA-Z_][a-zA-Z0-9_.-]*)(\s*)(=)/, ['key', '', 'operator']],
      [/([a-zA-Z_][a-zA-Z0-9_.-]*)(\s*)(=)/, ['key', '', 'operator']],
      [/"([^"\\]|\\.)*"(\s*)(=)/, ['key', '', 'operator']],
      [/'([^'\\]|\\.)*'(\s*)(=)/, ['key', '', 'operator']],
      [/"""/, { token: 'string.quote', next: '@multiLineString' }],
      [/'''/, { token: 'string.quote', next: '@multiLineLiteralString' }],
      [/"([^"\\]|\\.)*$/, 'string.invalid'],
      [/"/, { token: 'string.quote', next: '@string' }],
      [/'[^']*$/, 'string.invalid'],
      [/'/, { token: 'string.quote', next: '@literalString' }],
      [/[ \t\r\n]+/, ''],
      [/[{}[\]()]/, '@brackets'],
      [/,/, 'delimiter.comma'],
      [/\./, 'delimiter.dot'],
      [/=/, 'operator'],
    ],
    string: [
      [/[^\\"]+/, 'string'],
      [/\\./, 'string.escape'],
      [/"/, { token: 'string.quote', next: '@pop' }],
    ],
    literalString: [
      [/[^']+/, 'string'],
      [/'/, { token: 'string.quote', next: '@pop' }],
    ],
    multiLineString: [
      [/[^\\"]+/, 'string'],
      [/\\./, 'string.escape'],
      [/"""/, { token: 'string.quote', next: '@pop' }],
      [/./, 'string'],
    ],
    multiLineLiteralString: [
      [/[^']+/, 'string'],
      [/'''/, { token: 'string.quote', next: '@pop' }],
      [/./, 'string'],
    ],
  },
};

/** Register TOML language to Monaco Editor */
export function registerTomlLanguage(monaco: typeof Monaco): void {
  try {
    const languages = monaco.languages.getLanguages();
    const tomlLangExists = languages.some(lang => lang.id === 'toml');
    
    if (!tomlLangExists) {
      monaco.languages.register({
        id: 'toml',
        extensions: ['.toml'],
        aliases: ['TOML', 'toml'],
        mimetypes: ['text/x-toml'],
      });
    }
    
    monaco.languages.setLanguageConfiguration('toml', tomlLanguageConfig);
    monaco.languages.setMonarchTokensProvider('toml', tomlLanguageTokens);
  } catch (error) {
    log.error('Failed to register TOML language', error);
    throw error;
  }
}

