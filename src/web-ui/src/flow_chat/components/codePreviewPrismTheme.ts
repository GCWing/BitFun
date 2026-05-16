/**
 * Prism themes for Flow Chat embedded code previews.
 */
import type { CSSProperties } from 'react';

/** Match `.markdown-renderer` code blocks (`Markdown.scss` --markdown-font-mono). */
export const CODE_PREVIEW_FONT_FAMILY =
  'var(--markdown-font-mono, "Fira Code", "JetBrains Mono", Consolas, "Courier New", monospace)';

const PRE_KEY = 'pre[class*="language-"]' as const;
const CODE_KEY = 'code[class*="language-"]' as const;

export function buildCodePreviewPrismStyle(isLight: boolean): Record<string, CSSProperties> {
  const foreground = isLight ? '#24292f' : '#d4d4d4';
  const comment = isLight ? '#6e7781' : '#6a9955';
  const keyword = isLight ? '#cf222e' : '#c586c0';
  const string = isLight ? '#0a3069' : '#ce9178';
  const functionName = isLight ? '#8250df' : '#dcdcaa';
  const number = isLight ? '#0550ae' : '#b5cea8';
  const tag = isLight ? '#116329' : '#569cd6';

  return {
    [PRE_KEY]: {
      color: foreground,
      margin: 0,
      padding: 0,
      background: 'transparent',
      fontSize: '12px',
      lineHeight: '1.6',
      fontFamily: CODE_PREVIEW_FONT_FAMILY,
      fontWeight: 400,
    },
    [CODE_KEY]: {
      color: foreground,
      background: 'transparent',
      fontSize: '12px',
      lineHeight: '1.6',
      fontFamily: CODE_PREVIEW_FONT_FAMILY,
      fontWeight: 400,
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
