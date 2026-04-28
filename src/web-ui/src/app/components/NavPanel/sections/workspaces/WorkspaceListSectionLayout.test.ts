import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readWorkspaceListStylesheet(): string {
  const stylesheet = readFileSync(
    fileURLToPath(new URL('./WorkspaceListSection.scss', import.meta.url)),
    'utf8',
  );
  return stylesheet.replace(/\r\n/g, '\n');
}

function extractBlock(stylesheet: string, selector: string): string {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = stylesheet.match(new RegExp(`${escapedSelector}\\s*\\{(?<body>[\\s\\S]*?)\\n\\s*\\}`));
  return match?.groups?.body ?? '';
}

describe('WorkspaceListSection layout styles', () => {
  it('keeps workspace rows constrained while only visible row actions reserve title space', () => {
    const stylesheet = readWorkspaceListStylesheet();
    const workspaceList = extractBlock(stylesheet, '&__workspace-list');
    const workspaceGroup = extractBlock(stylesheet, '&__workspace-group');
    const workspaceItem = extractBlock(stylesheet, '&__workspace-item');
    const workspaceCard = extractBlock(stylesheet, '&__workspace-item-card');
    const workspaceNameButton = extractBlock(stylesheet, '&__workspace-item-name-btn');
    const workspaceTitle = extractBlock(stylesheet, '&__workspace-item-title');
    const workspaceLabel = extractBlock(stylesheet, '&__workspace-item-label');
    const workspaceActions = extractBlock(stylesheet, '&__workspace-item-actions');
    const workspaceMenu = extractBlock(stylesheet, '&__workspace-item-menu');
    const assistantItem = extractBlock(stylesheet, '&__assistant-item');
    const assistantCard = extractBlock(stylesheet, '&__assistant-item-card');
    const assistantNameButton = extractBlock(stylesheet, '&__assistant-item-name-btn');
    const assistantLabel = extractBlock(stylesheet, '&__assistant-item-label');
    const assistantMenu = extractBlock(stylesheet, '&__assistant-item-menu');

    expect(workspaceList).toContain('min-width: 0;');
    expect(workspaceList).toContain('max-width: 100%;');
    expect(workspaceGroup).toContain('min-width: 0;');
    expect(workspaceItem).toContain('min-width: 0;');
    expect(workspaceItem).toContain('max-width: 100%;');
    expect(workspaceCard).toContain('max-width: 100%;');
    expect(workspaceCard).toContain('overflow: hidden;');
    expect(workspaceNameButton).toContain('flex: 0 1 auto;');
    expect(workspaceNameButton).toContain('overflow: hidden;');
    expect(workspaceNameButton).not.toContain('58px');
    expect(stylesheet).toContain('var(--bitfun-nav-row-action-size) +\n      var(--bitfun-nav-row-action-size)');
    expect(stylesheet).toContain('&__workspace-item:hover &__workspace-item-name-stack');
    expect(stylesheet).toContain('&__workspace-item.is-menu-open &__workspace-item-name-stack');
    expect(stylesheet).not.toContain('&__workspace-item.is-active &__workspace-item-name-btn');
    expect(stylesheet).toContain('&:not(:hover):not(:focus-within):not(.is-menu-open)');
    expect(workspaceTitle).toContain('flex: 1 1 0;');
    expect(workspaceTitle).toContain('max-width: 100%;');
    expect(workspaceLabel).toContain('flex: 1 1 0;');
    expect(workspaceLabel).toContain('text-overflow: ellipsis;');
    expect(workspaceActions).toContain('position: absolute;');
    expect(workspaceActions).toContain('right: var(--bitfun-nav-row-action-offset);');
    expect(workspaceActions).toContain('gap: var(--bitfun-nav-row-action-gap);');
    expect(workspaceMenu).toContain('gap: var(--bitfun-nav-row-action-gap);');

    expect(assistantItem).toContain('min-width: 0;');
    expect(assistantItem).toContain('max-width: 100%;');
    expect(assistantCard).toContain('max-width: 100%;');
    expect(assistantCard).toContain('overflow: hidden;');
    expect(assistantNameButton).toContain('flex: 1 1 0;');
    expect(assistantNameButton).toContain('overflow: hidden;');
    expect(assistantNameButton).not.toContain('58px');
    expect(stylesheet).toContain('&__assistant-item:hover &__assistant-item-name-btn');
    expect(stylesheet).toContain('&__assistant-item.is-menu-open &__assistant-item-name-btn');
    expect(stylesheet).not.toContain('&__assistant-item.is-active &__assistant-item-name-btn');
    expect(assistantLabel).toContain('flex: 1 1 0;');
    expect(assistantLabel).toContain('text-overflow: ellipsis;');
    expect(assistantMenu).toContain('position: absolute;');
    expect(assistantMenu).toContain('right: var(--bitfun-nav-row-action-offset);');
    expect(assistantMenu).toContain('gap: var(--bitfun-nav-row-action-gap);');
    expect(stylesheet).toContain('.bitfun-nav-panel__inline-list {\n      margin-left: 8px;');
    expect(stylesheet).toContain('padding-left: 2px;');
    expect(stylesheet).toContain('padding-right: 0;');
  });

  it('uses the shared nav row-action size for workspace and assistant menu triggers', () => {
    const stylesheet = readWorkspaceListStylesheet();
    const workspaceTrigger = extractBlock(stylesheet, '&__workspace-item-menu-trigger');
    const assistantTrigger = extractBlock(stylesheet, '&__assistant-item-menu-trigger');

    for (const block of [workspaceTrigger, assistantTrigger]) {
      expect(block).toContain('width: var(--bitfun-nav-row-action-size);');
      expect(block).toContain('height: var(--bitfun-nav-row-action-size);');
    }
  });
});
