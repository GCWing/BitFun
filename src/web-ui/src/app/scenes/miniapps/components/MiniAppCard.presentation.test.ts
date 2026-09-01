import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { compile } from 'sass';
import { describe, expect, it } from 'vitest';

function readRelative(filename: string): string {
  return readFileSync(
    fileURLToPath(new URL(filename, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

const cardCss = compile(fileURLToPath(new URL('./MiniAppCard.scss', import.meta.url))).css;

describe('Mini App card presentation', () => {
  it('uses the catalog deletion glyph and shared running-count badge', () => {
    expect(readRelative('./MiniAppCard.tsx')).toContain('icon={<Icon name="delete" size="lg" />}');
    expect(readRelative('../views/MiniAppGalleryView.tsx')).toContain('<NumberBadge value={activeApps.length} />');
    expect(readRelative('../views/MiniAppGalleryView.tsx')).not.toContain('gallery-zone-badge');
  });

  it('uses shared controls and card anatomy across Mini App surfaces', () => {
    const gallery = readRelative('../views/MiniAppGalleryView.tsx');
    const market = readRelative('../views/MiniAppMarketView.tsx');
    const submissions = readRelative('../views/MiniAppSubmissionsView.tsx');

    expect(gallery).toContain('<SegmentedControl');
    expect(gallery).not.toContain('gallery-cat-chip');
    expect(market).toContain('<SegmentedControl');
    expect(market).toContain('<Card');
    expect(market).toContain('<CardMedia');
    expect(market).toContain('<CardBody');
    expect(market).toContain('<IconButton');
    expect(market).not.toContain('gallery-cat-chip');
    expect(submissions).toContain('<Disclosure');
    expect(submissions).toContain('<IconButton');
    expect(submissions).not.toContain('miniapp-submissions__advanced-toggle');
  });
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

  it('lets actions move to a new line instead of squeezing tags into the remaining width', () => {
    const footer = cardCss.match(/\.miniapp-card__footer \{([^}]+)\}/)?.[1];
    const actions = cardCss.match(/\.miniapp-card__actions \{([^}]+)\}/)?.[1];

    expect(footer).toContain('display: flex;');
    expect(footer).toContain('flex-wrap: wrap;');
    expect(footer).not.toContain('grid-template-columns:');
    expect(actions).toContain('margin-inline-start: auto;');
    expect(actions).toContain('flex-shrink: 0;');
    expect(actions).not.toContain('grid-column:');
  });

  it('wraps whole tag pills and applies ellipsis in a block formatting context', () => {
    const tags = cardCss.match(/\.miniapp-card__tags \{([^}]+)\}/)?.[1];
    const tag = cardCss.match(/\.miniapp-card__tag \{([^}]+)\}/)?.[1];
    const sharedTag = cardCss.match(/\.miniapp-card__tag,\s*\.miniapp-card__tag-overflow \{([^}]+)\}/)?.[1];

    expect(tags).toContain('flex: 1 1 auto;');
    expect(tags).toContain('flex-wrap: wrap;');
    expect(tags).toContain('max-width: 100%;');
    expect(tags).not.toContain('overflow: hidden;');
    expect(tag).toContain('display: block;');
    expect(tag).toContain('flex: 0 0 auto;');
    expect(tag).toContain('box-sizing: border-box;');
    expect(tag).toContain('max-width: min(100%, 16ch);');
    expect(sharedTag).toContain('white-space: nowrap;');
    expect(sharedTag).toContain('overflow: hidden;');
    expect(sharedTag).toContain('text-overflow: ellipsis;');
  });

  it('keeps the overflow count intact and retains the compact tag summary', () => {
    const overflow = cardCss.match(/(?:^|\})\s*\.miniapp-card__tag-overflow \{([^}]+)\}/)?.[1];

    expect(overflow).toContain('flex: 0 0 auto;');
    expect(cardCss).toMatch(/@container miniapp-card \(max-width: 519px\) \{\s*\.miniapp-card__tag--compact-hidden,\s*\.miniapp-card__tag-overflow--wide \{\s*display: none;/);
    expect(cardCss).toMatch(/@container miniapp-card[\s\S]*?\.miniapp-card__tag-overflow--compact \{\s*display: inline-flex;/);
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
