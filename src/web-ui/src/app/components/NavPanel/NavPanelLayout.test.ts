import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readNavPanelStylesheet(): string {
  const stylesheet = readFileSync(
    fileURLToPath(new URL('./NavPanel.scss', import.meta.url)),
    'utf8',
  );
  return stylesheet.replace(/\r\n/g, '\n');
}

function extractBlock(stylesheet: string, selector: string): string {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = stylesheet.match(new RegExp(`${escapedSelector}\\s*\\{(?<body>[\\s\\S]*?)\\n\\s*\\}`));
  return match?.groups?.body ?? '';
}

describe('NavPanel layout styles', () => {
  it('allows navigation list wrappers to shrink instead of inheriting long item widths', () => {
    const stylesheet = readNavPanelStylesheet();
    const rootBlock = extractBlock(stylesheet, '.bitfun-nav-panel');
    const contentBlock = extractBlock(stylesheet, '&__content');
    const mainLayerBlock = extractBlock(stylesheet, '&--main');
    const collapsibleBlock = extractBlock(stylesheet, '&__collapsible');
    const collapsibleInnerBlock = extractBlock(stylesheet, '&__collapsible-inner');
    const itemsBlock = extractBlock(stylesheet, '&__items');

    for (const block of [
      rootBlock,
      contentBlock,
      mainLayerBlock,
      collapsibleBlock,
      collapsibleInnerBlock,
      itemsBlock,
    ]) {
      expect(block).toContain('min-width: 0;');
      expect(block).toContain('max-width: 100%;');
    }
  });

  it('keeps root navigation rows close to the panel edge', () => {
    const stylesheet = readNavPanelStylesheet();
    const sectionHeaderBlock = extractBlock(stylesheet, '&__section-header');
    const itemsBlock = extractBlock(stylesheet, '&__items');

    expect(itemsBlock).toContain('padding: 2px $size-gap-1;');
    expect(sectionHeaderBlock).toContain('margin: 0 $size-gap-1;');
  });

  it('clips collapsible section content when collapsed via grid 0fr', () => {
    const stylesheet = readNavPanelStylesheet();
    const collapsibleInnerBlock = extractBlock(stylesheet, '&__collapsible-inner');

    expect(stylesheet).toContain('&.is-collapsed {\n      grid-template-rows: 0fr;');
    expect(collapsibleInnerBlock).toContain('overflow: hidden;');
    expect(collapsibleInnerBlock).toContain('min-height: 0;');
  });

  it('keeps root action buttons at the compact row size', () => {
    const stylesheet = readNavPanelStylesheet();
    const sectionActionBlock = extractBlock(stylesheet, '&__section-action');
    const itemActionBlock = extractBlock(stylesheet, '&__item-action');

    for (const block of [sectionActionBlock, itemActionBlock]) {
      expect(block).toContain('width: 20px;');
      expect(block).toContain('height: 20px;');
    }
  });
});
