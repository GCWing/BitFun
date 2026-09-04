import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const stylesheet = readFileSync(new URL('./PdfViewer.scss', import.meta.url), 'utf8');

describe('PdfViewer presentation', () => {
  it('keeps text-layer glyphs transparent while showing the selection background', () => {
    expect(stylesheet).toMatch(/::selection\s*{[^}]*background:[^;]+;[^}]*color:\s*transparent;[^}]*-webkit-text-fill-color:\s*transparent;/s);
  });

  it('does not paint PDF.js line-break selection boxes at the page origin', () => {
    expect(stylesheet).toMatch(/br::-moz-selection\s*{[^}]*background:\s*transparent;/s);
    expect(stylesheet).toMatch(/br::selection\s*{[^}]*background:\s*transparent;/s);
    expect(stylesheet).not.toMatch(/br::-moz-selection\s*,/s);
  });

  it('does not dim the visible canvas while a replacement frame renders', () => {
    expect(stylesheet).not.toMatch(/data-openbitfun-state[^}]*rendering[^}]*canvas\s*{/s);
    expect(stylesheet).not.toMatch(/transition:\s*opacity/);
  });
});
