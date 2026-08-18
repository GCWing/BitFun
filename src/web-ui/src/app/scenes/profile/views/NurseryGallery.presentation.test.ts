import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSibling(filename: string): string {
  return readFileSync(
    fileURLToPath(new URL(filename, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('Nursery gallery presentation', () => {
  it('uses the curated assistant artwork without loading the unused source pack', () => {
    const source = readSibling('./NurseryGallery.tsx');

    expect(source).toContain('src="/assets/assistant/defaults-illustration.webp"');
    expect(source).toContain('src="/assets/assistant/gallery-companion.webp"');
    expect(source).not.toContain('/panda_1.png');
    expect(source).not.toContain('/panda_wink.png');
  });

  it('keeps the gallery surface white and collapses the decorative column responsively', () => {
    const stylesheet = readSibling('./NurseryView.scss');

    expect(stylesheet).toMatch(
      /\.nursery-gallery \{\s+background: var\(--bf-appearance-token-color-static-white\);/,
    );
    expect(stylesheet).toContain('.nursery-gallery__assistant-showcase--with-companion');
    expect(stylesheet).toContain('grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));');
    expect(stylesheet).toMatch(
      /@media \(max-width: 1100px\)[\s\S]*\.nursery-gallery__companion \{\s+display: none;/,
    );
  });

  it('keeps assistant card content and actions in bounded regions', () => {
    const source = readSibling('./AssistantCard.tsx');
    const stylesheet = readSibling('./NurseryView.scss');
    const cardStart = stylesheet.indexOf('.assistant-card {');
    const cardEnd = stylesheet.indexOf('// ── Sub-page chrome', cardStart);
    const cardSection = stylesheet.slice(cardStart, cardEnd);

    expect(cardSection).toContain('&__main {');
    expect(cardSection).toContain('min-height: 168px;');
    expect(cardSection).toContain('padding: $size-gap-3 14px;');
    expect(cardSection).toContain('min-height: 52px;');
    expect(cardSection).toContain('--assistant-card-start-bg: var(--bf-appearance-token-element-bg-base);');
    expect(cardSection).toContain('background: var(--assistant-card-start-bg);');
    expect(cardSection).toContain('background: var(--assistant-card-inverse-bg);');
    expect(cardSection).toContain('&__session-actions {');
    expect(cardSection).toContain('border-top: 1px solid var(--bf-appearance-token-color-overlay-black-12);');
    expect(cardSection).not.toContain('min-height: clamp(310px, 23.8vw, 366px);');
    expect(cardSection).not.toContain('height: 100%;');
    expect(source).toContain('className="assistant-card__configure"');
    expect(source).toContain('className="assistant-card__session-actions"');
    expect(source).not.toContain('className="assistant-card__body"');
  });
});
