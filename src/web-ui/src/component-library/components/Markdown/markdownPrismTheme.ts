import type { CSSProperties } from 'react';

export function buildMarkdownPrismStyle(isLight: boolean): Record<string, CSSProperties> {
  const foreground = isLight ? '#24292f' : '#d4d4d4';
  const comment = isLight ? '#6e7781' : '#6a9955';
  const keyword = isLight ? '#cf222e' : '#c586c0';
  const string = isLight ? '#0a3069' : '#ce9178';
  const functionName = isLight ? '#8250df' : '#dcdcaa';
  const number = isLight ? '#0550ae' : '#b5cea8';
  const tag = isLight ? '#116329' : '#569cd6';

  return {
    'pre[class*="language-"]': {
      color: foreground,
      background: 'transparent',
      margin: 0,
      fontSize: '0.875rem',
      lineHeight: '1.55',
      fontFamily: 'var(--markdown-font-mono, "Fira Code", "JetBrains Mono", Consolas, "Courier New", monospace)',
    },
    'code[class*="language-"]': {
      color: foreground,
      background: 'transparent',
      fontSize: '0.875rem',
      lineHeight: '1.55',
      fontFamily: 'var(--markdown-font-mono, "Fira Code", "JetBrains Mono", Consolas, "Courier New", monospace)',
    },
    comment: { color: comment, fontStyle: 'italic' },
    prolog: { color: comment },
    doctype: { color: comment },
    cdata: { color: comment },
    punctuation: { color: isLight ? '#57606a' : '#d4d4d4' },
    property: { color: isLight ? '#953800' : '#9cdcfe' },
    tag: { color: tag },
    boolean: { color: number },
    number: { color: number },
    constant: { color: number },
    symbol: { color: number },
    selector: { color: tag },
    attrName: { color: isLight ? '#953800' : '#9cdcfe' },
    string: { color: string },
    char: { color: string },
    builtin: { color: functionName },
    inserted: { color: tag },
    operator: { color: isLight ? '#0550ae' : '#d4d4d4' },
    entity: { color: string },
    url: { color: string },
    atrule: { color: keyword },
    attrValue: { color: string },
    keyword: { color: keyword },
    function: { color: functionName },
    className: { color: functionName },
    regex: { color: string },
    important: { color: keyword, fontWeight: 600 },
    variable: { color: isLight ? '#953800' : '#9cdcfe' },
    deleted: { color: keyword },
  };
}
