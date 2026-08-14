import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function source(relativePath: string): string {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8');
}

describe('unified project session creation', () => {
  it('exposes one unified action as the first sidebar navigation item', () => {
    const mainNav = source('./MainNav.tsx');
    const workspaceItem = source('./sections/workspaces/WorkspaceItem.tsx');
    const newSessionIndex = mainNav.indexOf('data-testid="nav-new-session-btn"');
    const smartMembersIndex = mainNav.indexOf('data-testid="nav-smart-members-btn"');

    expect(newSessionIndex).toBeGreaterThan(-1);
    expect(smartMembersIndex).toBeGreaterThan(newSessionIndex);
    expect(mainNav).toContain("new CustomEvent('toolbar-create-session')");
    expect(mainNav).not.toContain('nav-new-code-session-btn');
    expect(mainNav).not.toContain('nav-new-cowork-session-btn');
    expect(workspaceItem).toContain('data-testid="nav-workspace-menu-create-session"');
  });

  it('exposes persistent navigation objects and reuses assistant sessions under Smart Members', () => {
    const mainNav = source('./MainNav.tsx');

    expect(mainNav).toContain('data-testid="nav-smart-members-btn"');
    expect(mainNav).toContain('data-testid="nav-long-term-tracking-btn"');
    expect(mainNav).toContain('data-testid="nav-todos-btn"');
    expect(mainNav).toContain("t('nav.messages.longTermTrackingComingSoon')");
    expect(mainNav).toContain("new Set(['workspace'])");
    expect(mainNav).toContain('<SessionsSection');
  });

  it('keeps Extensions & Compatibility in the persistent bottom area', () => {
    const mainNav = source('./MainNav.tsx');
    const bottomBarIndex = mainNav.indexOf('data-testid="nav-bottom-bar"');
    const extensionIndex = mainNav.indexOf('data-testid="agent-skill-entry"');

    expect(bottomBarIndex).toBeGreaterThan(-1);
    expect(extensionIndex).toBeGreaterThan(bottomBarIndex);
    expect(mainNav).toContain('data-testid="ecosystem-compatibility-tab"');
    expect(mainNav).toContain("openTab('external-sources')");
  });

  it('places the structured session view controls beside the workspace add action', () => {
    const mainNav = source('./MainNav.tsx');
    const filterIndex = mainNav.indexOf('<WorkspaceSessionFilterMenu />');
    const addIndex = mainNav.indexOf('data-testid="nav-workspace-add-btn"');
    const filterMenu = source('./components/WorkspaceSessionFilterMenu.tsx');

    expect(filterIndex).toBeGreaterThan(-1);
    expect(addIndex).toBeGreaterThan(filterIndex);
    expect(filterMenu).toContain('data-testid="nav-session-filter-btn"');
    expect(filterMenu).toContain("type Submenu = 'grouping' | 'ordering' | 'show'");
    expect(filterMenu).toContain("{row('status'");
    expect(filterMenu).toContain("{row('environment'");
    expect(filterMenu).toContain("{row('source'");
    expect(filterMenu).toContain("t('nav.sessions.viewMenu.collapseAll')");
    expect(filterMenu).toContain("t('nav.sessions.viewMenu.markAllRead')");
  });

  it('projects the same session model as workspace groups or one flat list', () => {
    const workspaceList = source('./sections/workspaces/WorkspaceListSection.tsx');
    const sessionsSection = source('./sections/sessions/SessionsSection.tsx');

    expect(workspaceList).toContain("grouping === 'all'");
    expect(workspaceList).toContain('workspaceScopes={workspaceScopes}');
    expect(workspaceList).toContain('layout="flat"');
    expect(sessionsSection).toContain("'sessions_nav_all_grouping'");
    expect(sessionsSection).toContain('matchesWorkspaceSessionView(');
    expect(sessionsSection).toContain('loadArchivedSessionMetadata(');
    expect(sessionsSection).toContain("layout === 'flat' ? ' is-flat-workspace-view'");
    expect(sessionsSection).toContain('bitfun-nav-panel__inline-item-workspace-name');
  });

  it('keeps workspace and floating menus free of Code/Cowork creation choices', () => {
    const workspaceItem = source('./sections/workspaces/WorkspaceItem.tsx');
    const sessionMenu = source('../../../flow_chat/components/session-menu/SessionMenu.tsx');

    expect(workspaceItem).toContain('data-testid="nav-workspace-menu-create-session"');
    expect(workspaceItem).not.toContain('nav-workspace-menu-create-code-session');
    expect(workspaceItem).not.toContain('nav-workspace-menu-create-cowork-session');
    expect(sessionMenu).toContain("new CustomEvent('toolbar-create-session')");
    expect(sessionMenu).toContain("t('toolCards.toolbar.newSessionItem')");
    expect(sessionMenu).not.toContain("createSession('cowork')");
  });

  it('does not reintroduce Code/Cowork through project session titles or management icons', () => {
    const sessionsSection = source('./sections/sessions/SessionsSection.tsx');
    const batchModal = source('./sections/workspaces/WorkspaceSessionBatchModal.tsx');

    expect(sessionsSection).not.toContain("t('nav.sessions.newCoworkSession')");
    expect(sessionsSection).not.toContain("t('nav.sessions.newCodeSession')");
    expect(batchModal).toContain("type SessionPresentation = 'project' | 'assistant'");
    expect(batchModal).not.toContain("type SessionMode = 'code' | 'cowork' | 'claw'");
  });
});
