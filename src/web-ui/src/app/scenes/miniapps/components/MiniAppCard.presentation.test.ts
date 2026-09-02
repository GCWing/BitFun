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
  it('keeps destructive actions in details and places counts beside section titles', () => {
    expect(readRelative('./MiniAppCard.tsx')).not.toContain('name="delete"');
    expect(readRelative('../views/MiniAppGalleryView.tsx')).toContain('titleAdornment={<NumberBadge value={activeApps.length} />}');
    expect(readRelative('../views/MiniAppGalleryView.tsx')).toContain('titleAdornment={<NumberBadge value={filtered.length} />}');
    expect(readRelative('../views/MiniAppGalleryView.tsx')).not.toContain('gallery-zone-badge');
  });

  it('uses shared controls and card anatomy across Mini App surfaces', () => {
    const gallery = readRelative('../views/MiniAppGalleryView.tsx');
    const market = readRelative('../views/MiniAppMarketView.tsx');
    const submissions = readRelative('../views/MiniAppSubmissionsView.tsx');
    const tabs = readRelative('../MiniAppGalleryScene.tsx');

    expect(gallery).toContain('<SegmentedControl');
    expect(gallery).toContain('variant="pills"');
    expect(gallery).not.toContain('gallery-cat-chip');
    expect(tabs).toContain('distribution="fill"');
    expect(tabs).toContain('size="md"');
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

  it('keeps every gallery pane on the same compact content rail', () => {
    const sceneStyles = readRelative('../MiniAppGalleryScene.scss');
    const gallery = readRelative('../views/MiniAppGalleryView.tsx');
    const market = readRelative('../views/MiniAppMarketView.tsx');
    const submissions = readRelative('../views/MiniAppSubmissionsView.tsx');
    const submissionStyles = readRelative('../views/MiniAppSubmissionsView.scss');

    expect(gallery).toContain('className="miniapp-gallery-pane miniapp-gallery"');
    expect(market).toContain('className="miniapp-gallery-pane miniapp-market-native"');
    expect(submissions.match(/className="miniapp-gallery-pane miniapp-submissions"/g)).toHaveLength(3);
    expect(submissions.match(/<GalleryPageHeader/g)).toHaveLength(3);
    expect(sceneStyles).toContain(
      '$miniapp-gallery-content-inline-size: min(calc(100% - var(--bf-space-6)), 680px);',
    );
    expect(sceneStyles).toMatch(
      /\.miniapp-gallery-pane \{[\s\S]*?\.gallery-page-header \{[\s\S]*?width: \$miniapp-gallery-content-inline-size;/,
    );
    expect(sceneStyles).toMatch(
      /\.miniapp-gallery-pane \{[\s\S]*?\.gallery-zones \{[\s\S]*?width: \$miniapp-gallery-content-inline-size;/,
    );
    expect(sceneStyles).toContain('scrollbar-gutter: stable;');
    expect(submissionStyles).not.toContain('1360px');
    expect(submissionStyles).toMatch(/&__workspace \{[\s\S]*?grid-template-columns: minmax\(0, 1fr\);/);
  });

  it('uses the compact vertical catalog-card geometry', () => {
    const stylesheet = readRelative('./MiniAppCard.scss');
    const rootStart = stylesheet.indexOf('.miniapp-card {');
    const rootEnd = stylesheet.indexOf('&:hover {', rootStart);
    const rootGeometry = stylesheet.slice(rootStart, rootEnd);

    expect(rootGeometry).not.toContain('max-width:');
    expect(rootGeometry).toContain('min-height: 193px;');
    expect(rootGeometry).not.toContain('aspect-ratio:');
    expect(stylesheet).toMatch(/&__main \{[\s\S]*?flex-direction: column;/);
    expect(stylesheet).toContain('-webkit-line-clamp: 3;');
    expect(stylesheet).toMatch(/&__footer \{[\s\S]*?margin-top: auto;/);
  });

  it('packs cards densely and keeps skeleton geometry aligned', () => {
    const source = readRelative('../views/MiniAppGalleryView.tsx');
    const stylesheet = readRelative('../views/MiniAppGalleryView.scss');
    const sceneStyles = readRelative('../MiniAppGalleryScene.scss');

    expect(source).toContain('const MINIAPP_CARD_MIN_WIDTH = 280;');
    expect(source.match(/minCardWidth=\{MINIAPP_CARD_MIN_WIDTH\}/g)).toHaveLength(3);
    expect(stylesheet).toMatch(/&__card-grid \{[\s\S]*?grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);[\s\S]*?justify-items: stretch;/);
    expect(stylesheet).toMatch(
      /&__card-grid\.gallery-grid--skeleton \.gallery-skeleton-card \{[\s\S]*?height: 193px;/,
    );
    expect(sceneStyles).toContain('container-name: miniapp-gallery-scene;');
    expect(sceneStyles).toMatch(
      /@container miniapp-gallery-scene \(max-width: 760px\) \{[\s\S]*?\.gallery-page-header \{[\s\S]*?flex-direction: column;/,
    );
    expect(stylesheet).toMatch(
      /@container miniapp-gallery-scene \(max-width: 760px\) \{[\s\S]*?grid-template-columns: 1fr;/,
    );
    expect(stylesheet).not.toContain('aspect-ratio: 12 / 5;');
  });

  it('uses dedicated artwork for built-in catalog cards and keeps a glyph fallback', () => {
    const card = readRelative('./MiniAppCard.tsx');
    const icons = readRelative('../utils/miniAppIcons.tsx');

    expect(card).toContain('getMiniAppIconAsset(app.id)');
    expect(card).toContain('className="miniapp-card__icon-image"');
    expect(card).toContain("renderMiniAppIcon(app.icon || 'box', 40)");
    expect(icons).toContain("'builtin-ppt-live': pptLiveIcon");
    expect(icons).toContain("'builtin-coding-selfie': codingFootprintIcon");
    expect(icons).toContain("'builtin-regex-playground': regexPlaygroundIcon");
    expect(icons).toContain("'builtin-daily-divination': dailyDivinationIcon");
    expect(icons).toContain("'builtin-gomoku': gomokuIcon");
  });

  it('keeps tags and the circular run action on one footer row', () => {
    const stylesheet = readRelative('./MiniAppCard.scss');
    const footer = cardCss.match(/\.miniapp-card__footer \{([^}]+)\}/)?.[1];
    const actionsStart = stylesheet.lastIndexOf('&__actions {');
    const actions = stylesheet.slice(actionsStart, stylesheet.indexOf('}', actionsStart));
    const source = readRelative('./MiniAppCard.tsx');

    expect(footer).toContain('margin-top: auto;');
    expect(footer).not.toContain('flex-wrap: wrap;');
    expect(footer).not.toContain('grid-template-columns:');
    expect(actions).toContain('margin-inline-start: auto;');
    expect(actions).toContain('flex: 0 0 auto;');
    expect(actions).not.toContain('grid-column:');
    expect(source).toContain('shape="circle"');
    expect(source).toContain('variant="primary"');
  });

  it('keeps version and tags in a clipped single-line metadata rail', () => {
    const tags = cardCss.match(/\.miniapp-card__tags \{([^}]+)\}/)?.[1];
    const tag = cardCss.match(/\.miniapp-card__tag \{([^}]+)\}/)?.[1];
    const source = readRelative('./MiniAppCard.tsx');

    expect(tags).toContain('flex: 1 1 auto;');
    expect(tags).toContain('overflow: hidden;');
    expect(tags).toContain('white-space: nowrap;');
    expect(tag).toContain('display: block;');
    expect(tag).toContain('flex: 0 1 auto;');
    expect(tag).toContain('box-sizing: border-box;');
    expect(tag).toContain('text-overflow: ellipsis;');
    expect(source).toContain('V{marketReleaseNumber ?? app.version}');
    expect(source).toContain('localizedTags.slice(0, 4)');
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
