import type { CSSProperties } from 'react';
import { buildSharedPrismStyle } from '@/shared/prism/prismTheme';

export function buildMarkdownPrismStyle(isLight: boolean): Record<string, CSSProperties> {
  return buildSharedPrismStyle(isLight, {
    pre: {
      margin: 0,
      fontSize: '0.875rem',
      lineHeight: '1.55',
      fontFamily: 'var(--font-family-mono)',
    },
    code: {
      fontSize: '0.875rem',
      lineHeight: '1.55',
      fontFamily: 'var(--font-family-mono)',
    },
  });
}
