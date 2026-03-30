/**
 * Mermaid syntax highlighting configuration.
 */

import Prism from 'prismjs';

// Mermaid grammar definition.
Prism.languages.mermaid = {
  'comment': /%%[\s\S]*?%%|%%.*/,
  'chart-type': {
    pattern: /^\s*(graph|flowchart|sequenceDiagram|classDiagram|stateDiagram|erDiagram|journey|gantt|pie|gitgraph|mindmap|timeline|quadrantChart)\b/m,
    alias: 'keyword'
  },
  'direction': {
    pattern: /\b(TD|TB|BT|RL|LR)\b/,
    alias: 'keyword'
  },
  'connector': {
    pattern: /-->|->|---|--\|[^|]*\||==>/,
    alias: 'operator'
  },
  'node-shape': {
    pattern: /[[\](){}><]/,
    alias: 'punctuation'
  },
  'string': {
    pattern: /"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/,
    greedy: true
  },
  'identifier': {
    pattern: /\b[A-Za-z]\w*\b/,
    alias: 'variable'
  },
  'keyword': /\b(participant|actor|note|loop|alt|else|opt|par|and|rect|activate|deactivate|state|class|relationship|title|actor|participant)\b/,
  'number': /\b\d+(?:\.\d+)?\b/,
  'operator': /[+\-*/%=<>!&|]/,
  'punctuation': /[{}[\];(),.:]/
};

// Style configuration.
export const mermaidTheme = {
  comment: { color: '#6a9955', fontStyle: 'italic' },
  'chart-type': { color: '#569cd6', fontWeight: 'bold' },
  direction: { color: '#569cd6' },
  connector: { color: '#d4d4d4', fontWeight: 'bold' },
  'node-shape': { color: '#ffd700' },
  string: { color: '#ce9178' },
  identifier: { color: '#9cdcfe' },
  keyword: { color: '#c586c0' },
  number: { color: '#b5cea8' },
  operator: { color: '#d4d4d4' },
  punctuation: { color: '#d4d4d4' }
};

export default Prism;
