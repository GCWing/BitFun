import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readRelative(filename: string): string {
  return readFileSync(
    fileURLToPath(new URL(filename, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('Mini App card presentation', () => {
  it('bounds the card without coupling its height to its width', () => {
    const stylesheet = readRelative('./MiniAppCard.scss');
    const rootStart = stylesheet.indexOf('.miniapp-card {');
    const rootEnd = stylesheet.indexOf('&:hover {', rootStart);
    const rootGeometry = stylesheet.slice(rootStart, rootEnd);

    expect(rootGeometry).toContain('max-width: 400px;');
    expect(rootGeometry).toContain('min-height: 152px;');
    expect(rootGeometry).not.toContain('aspect-ratio:');
    expect(stylesheet).toContain('grid-template-columns: clamp(60px, 18%, 72px) minmax(0, 1fr);');
    expect(stylesheet).toMatch(/&__footer \{[\s\S]*?margin-top: auto;/);
  });

  it('packs cards densely and keeps skeleton geometry aligned', () => {
    const source = readRelative('../views/MiniAppGalleryView.tsx');
    const stylesheet = readRelative('../views/MiniAppGalleryView.scss');

    expect(source).toContain('const MINIAPP_CARD_MIN_WIDTH = 280;');
    expect(source.match(/minCardWidth=\{MINIAPP_CARD_MIN_WIDTH\}/g)).toHaveLength(3);
    expect(stylesheet).toMatch(/&__card-grid \{\s+justify-items: start;/);
    expect(stylesheet).toMatch(
      /&__card-grid\.gallery-grid--skeleton \.gallery-skeleton-card \{[\s\S]*?max-width: 400px;[\s\S]*?height: 152px;/,
    );
    expect(stylesheet).toMatch(
      /@media \(max-width: 480px\) \{[\s\S]*?\.miniapp-card,[\s\S]*?\.gallery-skeleton-card \{\s+max-width: 100%;/,
    );
    expect(stylesheet).not.toContain('aspect-ratio: 12 / 5;');
  });

  it('bounds market cards while preserving the media preview ratio', () => {
    const source = readRelative('../views/MiniAppMarketView.tsx');
    const stylesheet = readRelative('../views/MiniAppMarketView.scss');

    expect(source.match(/className="miniapp-market-native__card-grid"/g)).toHaveLength(2);
    expect(stylesheet).toContain('max-width: 360px;');
    expect(stylesheet).toMatch(/&__card-grid \{\s+justify-items: center;/);
    expect(stylesheet).toMatch(/&__visual \{[\s\S]*?aspect-ratio: 16 \/ 9;/);
  });
});
