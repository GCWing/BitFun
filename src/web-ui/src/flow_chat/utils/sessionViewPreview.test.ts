import { describe, expect, it } from 'vitest';

import {
  formatSessionViewPreviewText,
  isOnlySessionViewPreviewText,
  isSessionViewPreviewText,
} from './sessionViewPreview';

describe('sessionViewPreview', () => {
  it('replaces legacy internal markers with user-facing preview text', () => {
    expect(formatSessionViewPreviewText('abc\n...[truncated for session view]'))
      .toBe('abc\n... Output truncated for session preview');
    expect(formatSessionViewPreviewText('[truncated for session view]'))
      .toBe('Output omitted from session preview');
  });

  it('detects marker-only preview output', () => {
    expect(isOnlySessionViewPreviewText('[truncated for session view]')).toBe(true);
    expect(isOnlySessionViewPreviewText('...[truncated for session view]')).toBe(true);
    expect(isOnlySessionViewPreviewText('real output [truncated for session view]')).toBe(false);
    expect(isSessionViewPreviewText('real output [truncated for session view]')).toBe(true);
  });
});
