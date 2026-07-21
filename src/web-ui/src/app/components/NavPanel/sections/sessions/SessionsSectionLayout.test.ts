import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSessionsSectionStylesheet(): string {
  const stylesheet = readFileSync(
    fileURLToPath(new URL('./SessionsSection.scss', import.meta.url)),
    'utf8',
  );
  return stylesheet.replace(/\r\n/g, '\n');
}

function extractInlineItemActionsBlock(stylesheet: string): string {
  const match = stylesheet.match(/&__inline-item-actions\s*\{(?<body>[\s\S]*?)\n\s*\}/);
  return match?.groups?.body ?? '';
}

function extractInlineItemBlock(stylesheet: string, element: string): string {
  const match = stylesheet.match(new RegExp(`&__inline-item-${element}\\s*\\{(?<body>[\\s\\S]*?)\\n\\s*\\}`));
  return match?.groups?.body ?? '';
}

function extractBlock(stylesheet: string, selector: string): string {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = stylesheet.match(new RegExp(`${escapedSelector}\\s*\\{(?<body>[\\s\\S]*?)\\n\\s*\\}`));
  return match?.groups?.body ?? '';
}

describe('SessionsSection layout styles', () => {
  it('keeps session rows visually compact without reducing the click target height', () => {
    const stylesheet = readSessionsSectionStylesheet();
    const inlineListBlock = extractBlock(stylesheet, '&__inline-list');
    const inlineItemBlock = extractBlock(stylesheet, '&__inline-item');

    expect(inlineListBlock).toContain('padding: 2px $size-gap-1 2px;');
    expect(inlineListBlock).toContain('margin: 0 $size-gap-1 0 calc(#{$size-gap-1} + 4px);');
    expect(inlineListBlock).toContain('gap: 0;');
    expect(inlineItemBlock).toContain('height: 26px;');
    expect(stylesheet).toContain('margin-top: -2px;');
  });

  it('keeps hidden session row actions from reserving title width', () => {
    const stylesheet = readSessionsSectionStylesheet();
    const inlineItemBlock = extractBlock(stylesheet, '&__inline-item');
    const mainBlock = extractInlineItemBlock(stylesheet, 'main');
    const actionsBlock = extractInlineItemActionsBlock(stylesheet);

    expect(stylesheet).toContain('&__inline-item-main {\n    flex: 1 1 0;');
    expect(inlineItemBlock).toContain('position: relative;');
    expect(mainBlock).not.toContain('padding-right');
    expect(stylesheet).toContain('&__inline-item:hover &__inline-item-main');
    expect(stylesheet).toContain('&__inline-item:focus-within &__inline-item-main');
    expect(stylesheet).toContain('padding-right: 24px;');
    expect(actionsBlock).not.toContain('display: none;');
    expect(actionsBlock).toContain('position: absolute;');
    expect(actionsBlock).toContain('right: 4px;');
    expect(actionsBlock).toContain('gap: 4px;');
    expect(actionsBlock).toContain('visibility: hidden;');
    expect(actionsBlock).toContain('opacity: 0;');
    expect(actionsBlock).toContain('pointer-events: none;');
    expect(actionsBlock).toContain('.bitfun-nav-panel__inline-item:hover &');
    expect(actionsBlock).toContain('&.is-open');
    expect(actionsBlock).toContain('visibility: visible;');
  });

  it('keeps session menu buttons at the compact row size', () => {
    const stylesheet = readSessionsSectionStylesheet();
    const actionButtonBlock = extractBlock(stylesheet, '&__inline-item-action-btn');

    expect(actionButtonBlock).toContain('width: 20px;');
    expect(actionButtonBlock).toContain('height: 20px;');
  });

  it('keeps child-session badges visible while long titles are ellipsized', () => {
    const stylesheet = readSessionsSectionStylesheet();
    const labelBlock = extractInlineItemBlock(stylesheet, 'label');
    const btwBadgeBlock = extractInlineItemBlock(stylesheet, 'btw-badge');
    const reviewBadgeBlock = extractInlineItemBlock(stylesheet, 'review-badge');
    const backgroundSubagentBadgeBlock = extractInlineItemBlock(stylesheet, 'background-subagent-badge');

    expect(labelBlock).toContain('flex: 1 1 0;');
    expect(labelBlock).toContain('overflow: hidden;');
    expect(labelBlock).toContain('text-overflow: ellipsis;');
    expect(btwBadgeBlock).toContain('white-space: nowrap;');
    expect(btwBadgeBlock).toContain('overflow: visible;');
    expect(btwBadgeBlock).toContain('color: color-mix(in srgb, var(--color-accent-400) 62%, var(--color-text-primary));');
    expect(btwBadgeBlock).toContain('font-weight: 600;');
    expect(btwBadgeBlock).toContain('opacity: 0.96;');
    expect(reviewBadgeBlock).toContain('white-space: nowrap;');
    expect(reviewBadgeBlock).toContain('color: color-mix(in srgb, var(--color-accent-400) 82%, var(--color-text-primary));');
    expect(reviewBadgeBlock).toContain('font-weight: 600;');
    expect(backgroundSubagentBadgeBlock).toContain('flex: 0 0 auto;');
    expect(backgroundSubagentBadgeBlock).toContain('display: inline-grid;');
    expect(backgroundSubagentBadgeBlock).toContain('place-items: center;');
    expect(backgroundSubagentBadgeBlock).toContain('line-height: 0;');
    expect(backgroundSubagentBadgeBlock).toContain('width: 16px;');
    expect(backgroundSubagentBadgeBlock).toContain('height: 16px;');

    const backgroundSubagentIconBlock = extractInlineItemBlock(stylesheet, 'background-subagent-icon');
    expect(backgroundSubagentIconBlock).toContain('place-self: center;');
    expect(backgroundSubagentIconBlock).toContain('display: block;');
    expect(backgroundSubagentIconBlock).toContain('transform-origin: center center;');
    expect(backgroundSubagentIconBlock).toContain('--bitfun-subagent-bot-optical-y: -1px;');
    expect(stylesheet).toContain(
      'transform: translateY(var(--bitfun-subagent-bot-optical-y)) scale(1);',
    );
  });
});
