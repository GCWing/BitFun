import { describe, expect, it } from 'vitest';

import { getEditorType } from './helpers';

describe('getEditorType', () => {
  it('routes PDF files to the PDF viewer case-insensitively', () => {
    expect(getEditorType('report.PDF')).toBe('pdf-viewer');
  });
});
