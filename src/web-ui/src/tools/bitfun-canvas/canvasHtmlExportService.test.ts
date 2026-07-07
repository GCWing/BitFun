import { describe, expect, it } from 'vitest';

import { safeCanvasHtmlFileName } from './canvasHtmlExportService';

describe('Canvas HTML export service', () => {
  it('sanitizes Canvas titles into HTML filenames', () => {
    expect(safeCanvasHtmlFileName('Architecture: Layer / Map?')).toBe('Architecture- Layer - Map-.html');
    expect(safeCanvasHtmlFileName('diagram.htm')).toBe('diagram.html');
    expect(safeCanvasHtmlFileName('')).toBe('BitFun Canvas.html');
  });
});
