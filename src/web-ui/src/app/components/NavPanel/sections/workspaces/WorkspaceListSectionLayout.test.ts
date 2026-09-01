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

function readWorkspaceItemSource(): string {
  return readFileSync(
    fileURLToPath(new URL('./WorkspaceItem.tsx', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function readWorkspaceListSource(): string {
  return readFileSync(
    fileURLToPath(new URL('./WorkspaceListSection.tsx', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function extractBlock(stylesheet: string, selector: string): string {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = stylesheet.match(new RegExp(`^\\s*${escapedSelector}\\s*\\{(?<body>[\\s\\S]*?)\\n\\s*\\}`, 'm'));
  return match?.groups?.body ?? '';
}

describe('WorkspaceListSection layout styles', () => {
  it('keeps peer session groups airier than rows inside one group', () => {
    const stylesheet = readWorkspaceListStylesheet();
    const workspaceList = extractBlock(stylesheet, '&__workspace-list');
    const workspaceDropTarget = extractBlock(stylesheet, '&__workspace-drop-target');

    expect(workspaceList).toContain('gap: var(--bf-space-2);');
    expect(workspaceDropTarget).toContain('gap: 0;');
  });

  it('keeps workspace rows constrained while only visible row actions reserve title space', () => {
    const stylesheet = readWorkspaceListStylesheet();
    const workspaceList = extractBlock(stylesheet, '&__workspace-list');
    const workspaceGroup = extractBlock(stylesheet, '&__workspace-group');
    const workspaceItem = extractBlock(stylesheet, '&__workspace-item');
    const workspaceCard = extractBlock(stylesheet, '&__workspace-item-card');
    const workspaceIcon = extractBlock(stylesheet, '&__workspace-item-icon');
    const workspaceNameButton = extractBlock(stylesheet, '&__workspace-item-name-btn');
    const workspaceTitle = extractBlock(stylesheet, '&__workspace-item-title');
    const workspaceLabel = extractBlock(stylesheet, '&__workspace-item-label');
    const workspaceActions = extractBlock(stylesheet, '&__workspace-item-actions');
    const workspaceMenu = extractBlock(stylesheet, '&__workspace-item-menu');
    const assistantItem = extractBlock(stylesheet, '&__assistant-item');
    const assistantCard = extractBlock(stylesheet, '&__assistant-item-card');
    const assistantCollapseButton = extractBlock(stylesheet, '&__assistant-item-collapse-btn');
    const assistantIcon = extractBlock(stylesheet, '&__assistant-item-avatar');
    const assistantNameButton = extractBlock(stylesheet, '&__assistant-item-name-btn');
    const assistantLabel = extractBlock(stylesheet, '&__assistant-item-label');
    const assistantMenu = extractBlock(stylesheet, '&__assistant-item-menu');

    expect(workspaceList).toContain('min-width: 0;');
    expect(workspaceList).toContain('max-width: 100%;');
    expect(workspaceGroup).toContain('min-width: 0;');
    expect(workspaceItem).toContain('min-width: 0;');
    expect(workspaceItem).toContain('max-width: 100%;');
    expect(workspaceItem).toContain('gap: calc(var(--bf-space-1) / 2);');
    expect(workspaceCard).toContain('max-width: 100%;');
    expect(workspaceCard).toContain('overflow: hidden;');
    expect(workspaceIcon).toContain('width: 16px;');
    expect(workspaceIcon).toContain('height: 16px;');
    expect(workspaceNameButton).toContain('flex: 0 1 auto;');
    expect(workspaceNameButton).toContain('overflow: hidden;');
    expect(workspaceNameButton).not.toContain('58px');
    expect(stylesheet).toContain('padding-right: 52px;');
    expect(stylesheet).toContain('&__workspace-item:hover &__workspace-item-name-stack');
    expect(stylesheet).toContain('&__workspace-item.is-menu-open &__workspace-item-name-stack');
    expect(stylesheet).not.toContain('&__workspace-item.is-active &__workspace-item-name-btn');
    expect(stylesheet).toContain('&:not(:hover):not(:focus-within):not(.is-menu-open)');
    expect(workspaceTitle).toContain('flex: 1 1 0;');
    expect(workspaceTitle).toContain('max-width: 100%;');
    expect(workspaceLabel).toContain('flex: 1 1 0;');
    expect(workspaceLabel).toContain('text-overflow: ellipsis;');
    expect(workspaceActions).toContain('position: absolute;');
    expect(workspaceActions).toContain('right: 4px;');
    expect(workspaceActions).toContain('gap: 4px;');
    expect(workspaceMenu).toContain('gap: 4px;');

    expect(assistantItem).toContain('min-width: 0;');
    expect(assistantItem).toContain('max-width: 100%;');
    expect(assistantItem).toContain('gap: calc(var(--bf-space-1) / 2);');
    expect(assistantCard).toContain('max-width: 100%;');
    expect(assistantCard).toContain('min-height: 30px;');
    expect(assistantCard).toContain('overflow: hidden;');
    expect(assistantCollapseButton).toContain('width: 26px;');
    expect(assistantCollapseButton).toContain('min-height: 30px;');
    expect(assistantCollapseButton).toContain('padding: 0 0 0 4px;');
    expect(assistantIcon).toContain('width: 16px;');
    expect(assistantIcon).toContain('height: 16px;');
    expect(assistantIcon).toContain('color: inherit;');
    expect(assistantIcon).toContain('opacity: 1;');
    expect(assistantNameButton).toContain('flex: 1 1 0;');
    expect(assistantNameButton).toContain('min-height: 30px;');
    expect(assistantNameButton).toContain('padding: 0 4px;');
    expect(assistantNameButton).toContain('overflow: hidden;');
    expect(assistantNameButton).not.toContain('58px');
    expect(stylesheet).toContain('&__assistant-item:hover &__assistant-item-name-btn');
    expect(stylesheet).toContain('padding-right: 52px;');
    expect(stylesheet).not.toContain('padding-right: 48px;');
    expect(stylesheet).toContain('&__assistant-item.is-menu-open &__assistant-item-name-btn');
    expect(stylesheet).not.toContain('&__assistant-item.is-active &__assistant-item-name-btn');
    expect(assistantLabel).toContain('flex: 1 1 0;');
    expect(assistantLabel).toContain('text-overflow: ellipsis;');
    expect(assistantMenu).toContain('position: absolute;');
    expect(assistantMenu).toContain('right: 4px;');
    expect(assistantMenu).toContain('gap: 4px;');
    // The 30px session indent now lives on the list as a shared rail so the
    // rows and their sibling "show more" toggle stay on one text axis.
    expect(stylesheet).toContain('--bf-nav-session-rail: 30px;');
    expect(stylesheet).toContain('padding-right: 0;');
  });

  it('keeps nested selection surfaces full-width while aligning their titles', () => {
    const stylesheet = readWorkspaceListStylesheet();
    const fullWidthSessionLists = stylesheet.match(
      /\.bitfun-nav-panel__inline-list \{\n\s+\/\/[^\n]+\n\s+margin-left: 0;/g,
    );
    const sessionRails = stylesheet.match(/--bf-nav-session-rail: 30px;/g);

    expect(fullWidthSessionLists).toHaveLength(2);
    expect(sessionRails).toHaveLength(2);
    // SessionsSection derives both the child connector and title offsets from
    // this rail. Context-specific child padding would split those axes again.
    expect(stylesheet).not.toMatch(/&\.is-child\s*\{\s*padding-left:/);
    expect(stylesheet).not.toContain('margin-left: 22px;');
    expect(stylesheet).toContain(
      '&__workspace-item.is-active:has(&__workspace-item-sessions &__inline-item.is-active)',
    );
    expect(stylesheet).toContain(
      '&__assistant-item.is-active:has(&__assistant-item-sessions &__inline-item.is-active)',
    );
  });

  it('keeps workspace and assistant menu triggers at the compact row size', () => {
    const stylesheet = readWorkspaceListStylesheet();
    const workspaceTrigger = extractBlock(stylesheet, '&__workspace-item-menu-trigger');
    const assistantTrigger = extractBlock(stylesheet, '&__assistant-item-menu-trigger');

    for (const block of [workspaceTrigger, assistantTrigger]) {
      expect(block).toContain('width: 20px;');
      expect(block).toContain('height: 20px;');
    }
  });

  it('keeps workspace and assistant rows flat on hover', () => {
    const stylesheet = readWorkspaceListStylesheet();

    expect(stylesheet).toContain(
      '.bitfun-nav-panel__workspace-item:hover,\n' +
      '  .bitfun-nav-panel__assistant-item:hover,\n' +
      '  .bitfun-nav-panel__workspace-item-card:hover,\n' +
      '  .bitfun-nav-panel__assistant-item-card:hover {\n' +
      '    transform: none;\n' +
      '    box-shadow: none;\n' +
      '  }',
    );
  });

  it('keeps remote connection metadata inside the sidebar with the status dot on the right', () => {
    const stylesheet = readWorkspaceListStylesheet();
    const remoteChip = extractBlock(stylesheet, '&__workspace-item-remote');
    const remoteName = extractBlock(stylesheet, '&__workspace-item-remote-name');

    expect(remoteChip).toContain('margin-left: -6px;');
    expect(remoteChip).toContain('padding: 0 5px 0 6px;');
    expect(remoteName).toContain('flex: 0 1 auto;');
    expect(remoteName).toContain('max-width: 160px;');
    expect(remoteName).toContain('overflow: hidden;');
    expect(remoteName).toContain('text-overflow: ellipsis;');

    const source = readWorkspaceItemSource();
    const remoteMetaStart = source.indexOf('data-testid="nav-workspace-remote-meta"');
    const remoteMetaEnd = source.indexOf('</Tooltip>', remoteMetaStart);
    const remoteMetaMarkup = source.slice(remoteMetaStart, remoteMetaEnd);

    expect(remoteMetaStart).toBeGreaterThanOrEqual(0);
    expect(remoteMetaEnd).toBeGreaterThan(remoteMetaStart);
    expect(remoteMetaMarkup.indexOf('workspace-item-remote-name'))
      .toBeLessThan(remoteMetaMarkup.indexOf('workspace-item-status-dot'));
  });

  it('anchors remote workspace icons to the primary title line', () => {
    const stylesheet = readWorkspaceListStylesheet();
    const workspaceCard = extractBlock(stylesheet, '&__workspace-item-card');
    const collapseButton = extractBlock(stylesheet, '&__workspace-item-collapse-btn');
    const remoteCollapseButton = extractBlock(
      stylesheet,
      "&__workspace-item[data-bf-state~='remote'] &__workspace-item-collapse-btn",
    );

    expect(workspaceCard).toContain('align-items: center;');
    expect(collapseButton).toContain('min-height: 30px;');
    expect(remoteCollapseButton).toContain('align-self: flex-start;');
    expect(remoteCollapseButton).toContain('align-items: flex-start;');
    expect(remoteCollapseButton).toContain('padding-top: 2px;');
  });

  it('uses stable BitFun semantics for grouped entries without a redundant aggregate header', () => {
    const itemSource = readWorkspaceItemSource();
    const listSource = readWorkspaceListSource();
    const stylesheet = readWorkspaceListStylesheet();

    expect(itemSource).toContain('<Icon name="user" size="sm" />');
    expect(itemSource).toContain('<Network size={16} />');
    expect(itemSource).toContain('<Icon name="folder" size="sm" />');
    expect(itemSource).not.toContain('SessionGroup');
    expect(listSource).not.toContain('nav.sessions.viewMenu.grouping.all');
    expect(listSource).not.toContain('workspace-all-sessions-header');
    expect(stylesheet).not.toContain('&__workspace-all-sessions-header');
    expect(stylesheet).not.toContain('&__workspace-all-sessions-icon');
    expect(stylesheet).toContain('&__assistant-item-group-icon');
    expect(extractBlock(stylesheet, '&__workspace-item:not(.is-active) &__workspace-item-icon'))
      .toContain('opacity: 1;');
    expect(extractBlock(stylesheet, '&__assistant-item:not(.is-active) &__assistant-item-avatar'))
      .toContain('opacity: 1;');
  });

  it('uses the standard workspace session-row presentation inside assistant groups', () => {
    const itemSource = readWorkspaceItemSource();
    const assistantSessionsStart = itemSource.indexOf('bitfun-nav-panel__assistant-item-sessions');
    const assistantSessionsEnd = itemSource.indexOf('useWorkspaceViewPreferences', assistantSessionsStart);
    const assistantSessionsMarkup = itemSource.slice(assistantSessionsStart, assistantSessionsEnd);

    expect(assistantSessionsStart).toBeGreaterThanOrEqual(0);
    expect(assistantSessionsEnd).toBeGreaterThan(assistantSessionsStart);
    expect(assistantSessionsMarkup).toContain('<SessionsSection');
    expect(assistantSessionsMarkup).not.toContain('presentation=');
  });
});
