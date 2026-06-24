import type { CSSProperties } from 'react';
import { SHARED_PRISM_COLOR_SCHEME } from '@/shared/theme/syntaxHighlightAccents';

type PrismBlockStyles = {
  pre: CSSProperties;
  code: CSSProperties;
};

const PRE_KEY = 'pre[class*="language-"]' as const;
const CODE_KEY = 'code[class*="language-"]' as const;

export function buildSharedPrismStyle(
  isLight: boolean,
  blockStyles: PrismBlockStyles,
): Record<string, CSSProperties> {
  const colors = isLight
    ? SHARED_PRISM_COLOR_SCHEME.light
    : SHARED_PRISM_COLOR_SCHEME.dark;

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
