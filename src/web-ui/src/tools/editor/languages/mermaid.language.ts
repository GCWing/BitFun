/**
 * Mermaid language definition for Monaco Editor.
 */

import type * as Monaco from 'monaco-editor';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('mermaid.language');

export const mermaidLanguageConfig: Monaco.languages.LanguageConfiguration = {
  comments: {
    lineComment: '%%',
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

export const mermaidLanguageTokens: Monaco.languages.IMonarchLanguage = {
  defaultToken: '',
  tokenizer: {
    root: [
      [/%%.*$/, 'comment'],
      [/"([^"\\]|\\.)*$/, 'string.invalid'],
      [/'([^'\\]|\\.)*$/, 'string.invalid'],
      [/"/, { token: 'string.quote', next: '@stringDouble' }],
      [/'/, { token: 'string.quote', next: '@stringSingle' }],
      [/\b(graph|flowchart|sequenceDiagram|classDiagram|stateDiagram|erDiagram|journey|gantt|pie|gitgraph|mindmap|timeline|quadrantChart)\b/, 'keyword'],
      [/\b(TD|TB|BT|RL|LR)\b/, 'keyword'],
      [/\b(participant|actor|note|loop|alt|else|opt|par|and|rect|activate|deactivate|state|class|relationship|title)\b/, 'keyword'],
      [/(-->|->|---|--\|[^|]*\||==>)/, 'operator'],
      [/\d+(\.\d+)?/, 'number'],
      [/[A-Za-z_\u4e00-\u9fff][A-Za-z0-9_\u4e00-\u9fff]*/, 'variable'],
      [/[{}[\]()]/, '@brackets'],
      [/[<>]/, 'delimiter'],
      [/[+\-*/%=!&|]/, 'operator'],
      [/[;:,.]/, 'delimiter'],
    ],
    stringDouble: [
      [/[^\\"]+/, 'string'],
      [/\\./, 'string.escape'],
      [/"/, { token: 'string.quote', next: '@pop' }],
    ],
    stringSingle: [
      [/[^\\']+/, 'string'],
      [/\\./, 'string.escape'],
      [/'/, { token: 'string.quote', next: '@pop' }],
    ],
  },
};

export function registerMermaidLanguage(monaco: typeof Monaco): void {
  try {
    const languages = monaco.languages.getLanguages();
    const mermaidExists = languages.some((lang) => lang.id === 'mermaid');

    if (!mermaidExists) {
      monaco.languages.register({
        id: 'mermaid',
        extensions: ['.mmd', '.mermaid'],
        aliases: ['Mermaid', 'mermaid'],
        mimetypes: ['text/x-mermaid'],
      });
    }

    monaco.languages.setLanguageConfiguration('mermaid', mermaidLanguageConfig);
    monaco.languages.setMonarchTokensProvider('mermaid', mermaidLanguageTokens);
  } catch (error) {
    log.error('Failed to register Mermaid language', error);
    throw error;
  }
}
