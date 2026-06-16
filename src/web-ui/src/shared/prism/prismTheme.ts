import type { CSSProperties } from 'react';

type PrismBlockStyles = {
  pre: CSSProperties;
  code: CSSProperties;
};

const PRISM_COLOR_SCHEME = {
  light: {
    foreground: '#24292f',
    comment: '#6e7781',
    keyword: '#cf222e',
    string: '#0a3069',
    functionName: '#8250df',
    number: '#0550ae',
    tag: '#116329',
    punctuation: '#57606a',
    property: '#953800',
  },
  dark: {
    foreground: '#d4d4d4',
    comment: '#6a9955',
    keyword: '#c586c0',
    string: '#ce9178',
    functionName: '#dcdcaa',
    number: '#b5cea8',
    tag: '#569cd6',
    punctuation: '#d4d4d4',
    property: '#9cdcfe',
  },
} as const;

const PRE_KEY = 'pre[class*="language-"]' as const;
const CODE_KEY = 'code[class*="language-"]' as const;

export function buildSharedPrismStyle(
  isLight: boolean,
  blockStyles: PrismBlockStyles,
): Record<string, CSSProperties> {
  const colors = isLight ? PRISM_COLOR_SCHEME.light : PRISM_COLOR_SCHEME.dark;

  return {
    [PRE_KEY]: {
      ...blockStyles.pre,
      color: colors.foreground,
      background: 'transparent',
    },
    [CODE_KEY]: {
      ...blockStyles.code,
      color: colors.foreground,
      background: 'transparent',
    },
    comment: { color: colors.comment, fontStyle: 'italic' },
    prolog: { color: colors.comment },
    doctype: { color: colors.comment },
    cdata: { color: colors.comment },
    punctuation: { color: colors.punctuation },
    property: { color: colors.property },
    tag: { color: colors.tag },
    boolean: { color: colors.number },
    number: { color: colors.number },
    constant: { color: colors.number },
    symbol: { color: colors.number },
    selector: { color: colors.tag },
    attrName: { color: colors.property },
    string: { color: colors.string },
    char: { color: colors.string },
    builtin: { color: colors.functionName },
    inserted: { color: colors.tag },
    operator: { color: isLight ? colors.number : colors.foreground },
    entity: { color: colors.string },
    url: { color: colors.string },
    atrule: { color: colors.keyword },
    attrValue: { color: colors.string },
    keyword: { color: colors.keyword },
    function: { color: colors.functionName },
    className: { color: colors.functionName },
    regex: { color: colors.string },
    important: { color: colors.keyword, fontWeight: 600 },
    variable: { color: colors.property },
    deleted: { color: colors.keyword },
  };
}
