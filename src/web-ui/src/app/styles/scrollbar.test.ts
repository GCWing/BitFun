import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const stylesheet = readFileSync(
  fileURLToPath(new URL('./utilities/scrollbar.css', import.meta.url)),
  'utf8',
).replace(/\r\n?/g, '\n');

describe('global scrollbar presentation', () => {
  it('keeps all WebKit and Chromium non-thumb surfaces transparent', () => {
    expect(stylesheet).toMatch(
      /\*::-webkit-scrollbar\s*\{[^}]*background:\s*transparent;/s,
    );

    for (const selector of [
      '*::-webkit-scrollbar-track',
      '*::-webkit-scrollbar-track-piece',
      '*::-webkit-scrollbar-corner',
      '*::-webkit-scrollbar-button',
      '*::-webkit-resizer',
    ]) {
      expect(stylesheet).toContain(selector);
    }

    expect(stylesheet).toMatch(
      /\*::-webkit-scrollbar-track,[^}]*\{\s*background:\s*transparent;/s,
    );
  });

  it('keeps the thumb visible and the standard track channel transparent', () => {
    expect(stylesheet).toMatch(
      /\*::-webkit-scrollbar-thumb\s*\{[^}]*background:\s*var\(--openbitfun-color-scrollbar-thumb\);/s,
    );
    expect(stylesheet).toMatch(
      /\*::-webkit-scrollbar-thumb:hover\s*\{[^}]*background:\s*var\(--openbitfun-color-scrollbar-thumb-hover\);/s,
    );
    expect(stylesheet).toContain(
      '@supports (scrollbar-color: transparent transparent)',
    );
    expect(stylesheet).toMatch(
      /\*\s*\{[^}]*scrollbar-color:\s*var\(--openbitfun-color-scrollbar-thumb\)\s+transparent;/s,
    );
    expect(stylesheet).not.toMatch(
      /^\s*scrollbar-color:\s*transparent\s+transparent\s*;/m,
    );
  });

  it('keeps Safari 18 on the transparent WebKit scrollbar path', () => {
    expect(stylesheet).toMatch(
      /@supports selector\(::-webkit-scrollbar\)[\s\S]*?@supports not \(scrollbar-color: transparent transparent\)[\s\S]*?scrollbar-width:\s*auto !important;/,
    );
  });
});
