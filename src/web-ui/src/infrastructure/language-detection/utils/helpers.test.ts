import { describe, expect, it } from 'vitest';

import { getEditorType } from './helpers';

describe('getEditorType', () => {
  it('routes PDF files to the PDF viewer case-insensitively', () => {
    expect(getEditorType('report.PDF')).toBe('pdf-viewer');
  });

  it('routes HTML files to the embedded preview case-insensitively', () => {
    expect(getEditorType('index.HTML')).toBe('html-preview');
    expect(getEditorType('legacy.HTM')).toBe('html-preview');
  });
});
