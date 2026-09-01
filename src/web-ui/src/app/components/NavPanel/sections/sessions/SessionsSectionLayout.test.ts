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

function readSessionsSectionSource(): string {
  const source = readFileSync(
    fileURLToPath(new URL('./SessionsSection.tsx', import.meta.url)),
    'utf8',
  );
  return source.replace(/\r\n/g, '\n');
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

    expect(inlineListBlock).toContain('padding: 2px var(--bf-space-1) 2px;');
    expect(inlineListBlock).toContain('margin: 0 var(--bf-space-1) 0 calc(var(--bf-space-1) + 4px);');
    expect(inlineListBlock).toContain('gap: calc(var(--bf-space-1) / 2);');
    expect(inlineItemBlock).toContain('height: 26px;');
    expect(stylesheet).toContain('margin-top: 0;');
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

  it('anchors session detail tooltips beside the row instead of over adjacent sessions', () => {
    const source = readSessionsSectionSource();
    const sessionTooltip = source.match(
      /<Tooltip\s+key=\{session\.sessionId\}[\s\S]*?disabled=\{isEditing \|\| openMenuSessionId !== null\}\s*>/,
    )?.[0] ?? '';

    expect(sessionTooltip).toContain('content={tooltipContent}');
    expect(sessionTooltip).toContain('placement="right"');
    expect(sessionTooltip).not.toContain('followCursor');
  });

  it('centers empty session placeholder content', () => {
    const stylesheet = readSessionsSectionStylesheet();
    const emptyBlock = extractBlock(stylesheet, '&__inline-empty');

    expect(emptyBlock).toContain('text-align: center;');
  });

  it('aligns the session expansion toggle to the session row text rail', () => {
    const stylesheet = readSessionsSectionStylesheet();
    const toggleBlock = extractBlock(stylesheet, '&__inline-toggle');
    const inlineItemBlock = extractBlock(stylesheet, '&__inline-item');
    const inlineListBlock = extractBlock(stylesheet, '&__inline-list');

    // The toggle is a sibling of the rows, so both take their left padding from
    // one inherited rail. Context stylesheets that indent rows (see
    // WorkspaceListSection's 30px icon gutter) only have to move the rail.
    expect(inlineListBlock).toContain('--bf-nav-session-rail:');
    expect(toggleBlock).toContain('padding: 0 var(--bf-space-1) 0 var(--bf-nav-session-rail);');
    expect(inlineItemBlock).toContain('padding: 0 var(--bf-space-1) 0 var(--bf-nav-session-rail);');
    expect(toggleBlock).toContain('justify-content: flex-start;');
    expect(toggleBlock).toContain('text-align: left;');
    expect(toggleBlock).toContain(`gap: ${inlineItemBlock.match(/gap: (\d+px);/)?.[1] ?? ''};`);
    expect(toggleBlock).not.toContain('justify-content: center;');
    // Every rule that indents a row must move the rail rather than hard-coding
    // padding-left, or the sibling toggle silently drifts off the rail again.
    const rowPaddingDecls = stylesheet.match(/&__inline-item \{[^}]*?padding(?:-left)?: [^;]+;/g) ?? [];
    expect(rowPaddingDecls.length).toBeGreaterThan(0);
    for (const decl of rowPaddingDecls) {
      expect(decl).toContain('var(--bf-nav-session-rail)');
    }
  });

  it('indents child connectors from the parent session text rail', () => {
    const stylesheet = readSessionsSectionStylesheet();

    expect(stylesheet).toContain(
      'padding-left: calc(var(--bf-nav-session-rail) + 14px);',
    );
    expect(
      stylesheet.match(/left: calc\(var\(--bf-nav-session-rail\) \+ 2px\);/g),
    ).toHaveLength(2);
    expect(stylesheet).not.toContain('left: 8px;');
  });

  it('keeps the remaining session count in a compact trailing chip', () => {
    const stylesheet = readSessionsSectionStylesheet();
    const countBlock = extractBlock(stylesheet, '&__inline-toggle-count');
    const labelBlock = extractBlock(stylesheet, '&__inline-toggle-label');
    const source = readSessionsSectionSource();

    expect(countBlock).toContain('flex: 0 0 auto;');
    expect(countBlock).toContain('border-radius: 999px;');
    expect(countBlock).toContain('font-variant-numeric: tabular-nums;');
    // The label grows to fill the row, pushing the chip and the chevron to the
    // trailing edge, and absorbs overflow so the chip is never ellipsized away.
    expect(labelBlock).toContain('flex: 1 1 auto;');
    expect(labelBlock).toContain('min-width: 0;');
    expect(labelBlock).toContain('text-overflow: ellipsis;');
    // The chip is decorative; the full sentence stays on the button's aria-label.
    expect(source).toContain('className="bitfun-nav-panel__inline-toggle-count" aria-hidden');
    expect(source).toContain("aria-label={t('nav.sessions.showMore', {");
    expect(source).toContain('aria-label={expandToggleLabels.ariaLabel}');
    expect(source).not.toContain('inline-toggle-dots');
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
    expect(btwBadgeBlock).toContain('color: color-mix(in srgb, color-mix(in srgb, var(--bf-color-accent-default) 40%, transparent) 62%, var(--bf-color-content-primary));');
    expect(btwBadgeBlock).toContain('font-weight: var(--bf-font-weight-semibold);');
    expect(btwBadgeBlock).toContain('opacity: 0.96;');
    expect(reviewBadgeBlock).toContain('white-space: nowrap;');
    expect(reviewBadgeBlock).toContain('color: color-mix(in srgb, color-mix(in srgb, var(--bf-color-accent-default) 40%, transparent) 82%, var(--bf-color-content-primary));');
    expect(reviewBadgeBlock).toContain('font-weight: var(--bf-font-weight-semibold);');
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
    expect(stylesheet).not.toContain('--bitfun-subagent-bot-optical-y');
    expect(stylesheet).not.toContain('translateY(var(--bitfun-subagent-bot-optical-y))');
  });
});
