import type { CSSProperties } from 'react';
import { buildSharedPrismStyle } from '@/shared/prism/prismTheme';

export function buildMarkdownPrismStyle(isLight: boolean): Record<string, CSSProperties> {
  return buildSharedPrismStyle(isLight, {
    pre: {
      margin: 0,
      fontSize: 'var(--bf-type-code-md-font-size)',
      lineHeight: 'var(--bf-type-code-md-line-height)',
      fontFamily: 'var(--bf-type-code-md-font-family)',
      fontWeight: 'var(--bf-type-code-md-font-weight)',
    },
    code: {
      fontSize: 'var(--bf-type-code-md-font-size)',
      lineHeight: 'var(--bf-type-code-md-line-height)',
      fontFamily: 'var(--bf-type-code-md-font-family)',
      fontWeight: 'var(--bf-type-code-md-font-weight)',
    },
  });
}
