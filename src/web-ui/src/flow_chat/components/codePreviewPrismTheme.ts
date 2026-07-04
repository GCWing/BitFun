/**
 * Prism themes for Flow Chat embedded code previews.
 */
import type { CSSProperties } from 'react';
import { buildSharedPrismStyle } from '@/shared/prism/prismTheme';

/** Match `.markdown-renderer` code blocks. */
export const CODE_PREVIEW_FONT_FAMILY =
  'var(--font-family-mono)';

export function buildCodePreviewPrismStyle(isLight: boolean): Record<string, CSSProperties> {
  return buildSharedPrismStyle(isLight, {
    pre: {
      margin: 0,
      padding: 0,
      fontSize: '12px',
      lineHeight: '1.6',
      fontFamily: CODE_PREVIEW_FONT_FAMILY,
      fontWeight: 400,
    },
    code: {
      fontSize: '12px',
      lineHeight: '1.6',
      fontFamily: CODE_PREVIEW_FONT_FAMILY,
      fontWeight: 400,
    },
  });
}
