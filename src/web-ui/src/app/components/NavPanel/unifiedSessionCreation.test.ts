import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function source(relativePath: string): string {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8');
}

describe('unified project session creation', () => {
  it('keeps New Session beside search and makes Hello open shared chat before voice', () => {
    const mainNav = source('./MainNav.tsx');
    const workspaceBody = source('../../layout/WorkspaceBody.tsx');
    const appLayout = source('../../layout/AppLayout.tsx');
    const helloLauncher = source('../../layout/FloatingMiniChat.tsx');
    const helloLauncherStyles = source('../../layout/FloatingMiniChat.scss');
    const voicePanel = source('../../../flow_chat/components/voice/RealtimeVoiceCallPanel.tsx');
    const workspaceItem = source('./sections/workspaces/WorkspaceItem.tsx');
    const utilityRowIndex = mainNav.indexOf('data-bf-part="utilityRow"');
    const newSessionIndex = mainNav.indexOf('data-testid="nav-new-session-btn"');
    const sectionsIndex = mainNav.indexOf('data-testid="nav-sections"');
    const sessionsSectionIndex = mainNav.indexOf('data-bf-section="sessions"');

    expect(newSessionIndex).toBeGreaterThan(utilityRowIndex);
    expect(sectionsIndex).toBeGreaterThan(newSessionIndex);
    expect(sessionsSectionIndex).toBeGreaterThan(newSessionIndex);
    expect(mainNav).toContain('<Icon name="plus" size="lg" style={{ width: 15, height: 15 }} aria-hidden="true" />');
    expect(mainNav).toContain("activateProductAction('session.new')");
    expect(mainNav).not.toContain('<RealtimeVoiceCallButton />');
    expect(workspaceBody).not.toContain('<RealtimeVoiceCallButton />');
    expect(appLayout).toContain('<FloatingMiniChat />');
    expect(appLayout).not.toContain('isWelcomeScene');
    expect(helloLauncher).toContain("tVoice('voiceCall.call.launcherLabel')");
    expect(helloLauncher).not.toContain('<Icon name="side-chat" size="md" />');
    expect(helloLauncher).toContain('onClick={handleOpen}');
    expect(helloLauncher).toContain('<ChatPane');
    expect(helloLauncher).toContain('data-testid="hello-realtime-voice-mode-switch"');
    expect(helloLauncher).toContain('onClick={isVoiceMode ? endVoiceCall : startVoiceCall}');
    expect(helloLauncher).toContain('<RealtimeVoiceCallPanel />');
    expect(helloLauncher).toContain('<LauncherButton');
    expect(helloLauncher).toContain('leadingIcon={<Icon name="mic" />}');
    expect(voicePanel).not.toContain('createPortal');
    expect(helloLauncherStyles).toContain('right: 0;');
    expect(helloLauncherStyles).toContain('bottom: 0;');
    expect(helloLauncherStyles).toContain(
      'z-index: calc(var(--bf-layer-overlay) + 1);',
    );
    expect(helloLauncherStyles).toContain('.bitfun-fmc__mode-switch');
    expect(mainNav).not.toContain('nav-new-code-session-btn');
    expect(mainNav).not.toContain('nav-new-cowork-session-btn');
    expect(workspaceItem).toContain('data-testid="nav-workspace-menu-create-session"');
  });

  it('projects projects and assistants through one grouped session region', () => {
    const mainNav = source('./MainNav.tsx');
    const footerActions = source('./components/PersistentFooterActions.tsx');
    const workspaceList = source('./sections/workspaces/WorkspaceListSection.tsx');
    const projection = source('./sessionNavigationProjection.ts');

    expect(mainNav).not.toContain('data-testid="nav-smart-members-btn"');
    expect(mainNav).not.toContain('data-testid="nav-long-term-tracking-btn"');
    expect(mainNav).toContain('data-testid="nav-todos-btn"');
    expect(footerActions).not.toContain('data-testid="nav-todos-btn"');
    expect(mainNav).toContain('data-bf-part="todoEntry"');
    expect(mainNav).toContain("activateProductAction('surface.todos.open')");
    expect(mainNav).not.toContain("new Set(['sessions'])");
    expect(mainNav).toContain('label={t(\'nav.items.sessions\')}');
    expect(mainNav).toContain('<WorkspaceListSection variant="all" />');
    expect(workspaceList).toContain('variant: SessionNavigationScope');
    expect(workspaceList).toContain("variant === 'all'");
    expect(workspaceList).toContain('openedWorkspacesList');
    expect(workspaceList).toContain('projectWorkspaceBackedSessionGroups');
    expect(projection).toContain("export type SessionNavigationScope = 'all' | 'assistants' | 'projects'");
    expect(projection).toContain("WorkspaceBackedSessionGroupKind = 'assistant' | 'project'");
  });

  it('places Task Board below AI Assistant, before Mini Apps and capability management', () => {
    const mainNav = source('./MainNav.tsx');
    const assistantIndex = mainNav.indexOf('data-testid="nav-assistant-manager"');
    const taskBoardIndex = mainNav.indexOf('data-testid="nav-todos-btn"');
    const miniAppsIndex = mainNav.indexOf('className="bitfun-nav-panel__miniapp-navigation"');
    const extensionIndex = mainNav.indexOf('data-testid="agent-skill-entry"');
    const sessionsIndex = mainNav.indexOf('data-bf-section="sessions"');

    expect(taskBoardIndex).toBeGreaterThan(assistantIndex);
    expect(miniAppsIndex).toBeGreaterThan(taskBoardIndex);
    expect(extensionIndex).toBeGreaterThan(miniAppsIndex);
    expect(sessionsIndex).toBeGreaterThan(extensionIndex);
    expect(mainNav).toContain("t('nav.items.todos')");
    expect(mainNav).not.toContain('data-testid="nav-bottom-bar"');
    expect(mainNav).toContain('className="bitfun-nav-panel__top-action-expand"');
    expect(mainNav).toContain('data-testid="ecosystem-compatibility-tab"');
    expect(mainNav).toContain("activateProductAction('surface.ecosystemCompatibility.open')");
    expect(mainNav).not.toContain("activateProductAction('settings.external-sources.open')");
  });

  it('opens the footer utility list from Settings without Star, More, or Insights', () => {
    const footerActions = source('./components/PersistentFooterActions.tsx');
    const appearanceQuickSwitch = source('./components/AppearanceQuickSwitchMenuItem.tsx');
    const floatingIndex = footerActions.indexOf('data-testid="nav-settings-floating-item"');
    const notificationIndex = footerActions.indexOf('<NotificationButton menuItem');
    const appearanceIndex = footerActions.indexOf('<AppearanceQuickSwitchMenuItem');
    const openSettingsIndex = footerActions.indexOf('data-testid="nav-settings-open-item"');
    const aboutIndex = footerActions.indexOf('data-testid="nav-settings-about-item"');

    expect(footerActions).toContain('data-testid="nav-footer-settings-item"');
    expect(footerActions).toContain('icon={<Icon name="gear" size="sm" aria-hidden="true" />}');
    expect(footerActions).toContain('data-testid="nav-settings-menu"');
    expect(floatingIndex).toBeGreaterThan(-1);
    expect(notificationIndex).toBeGreaterThan(floatingIndex);
    expect(appearanceIndex).toBeGreaterThan(notificationIndex);
    expect(openSettingsIndex).toBeGreaterThan(appearanceIndex);
    expect(aboutIndex).toBeGreaterThan(openSettingsIndex);
    expect(footerActions).toContain("useSettingsStore.getState().openPage('application.appearance')");
    expect(footerActions).not.toContain('GithubStarButton');
    expect(footerActions).not.toContain('nav-footer-github-star-btn');
    expect(appearanceQuickSwitch).toContain('data-testid="nav-settings-appearance-item"');
    expect(appearanceQuickSwitch).toContain('data-testid="nav-settings-appearance-menu"');
    expect(appearanceQuickSwitch).toContain('role="menuitemradio"');
    expect(appearanceQuickSwitch).toContain('selectedDisplayName');
    expect(footerActions).not.toContain('onMouseMove=');
    expect(footerActions).not.toContain('onFocusCapture=');
    expect(footerActions).not.toContain('data-testid="nav-footer-more-btn"');
    expect(footerActions).not.toContain("activateProductAction('surface.insights.open')");
  });

  it('places one semantic all/grouped toggle beside the session filter and group add action', () => {
    const mainNav = source('./MainNav.tsx');
    const groupingIndex = mainNav.indexOf('<WorkspaceSessionGroupingToggle />');
    const filterIndex = mainNav.indexOf('<WorkspaceSessionFilterMenu />');
    const addIndex = mainNav.indexOf('data-testid="nav-workspace-add-btn"');
    const groupingToggle = source('./components/WorkspaceSessionGroupingToggle.tsx');
    const filterMenu = source('./components/WorkspaceSessionFilterMenu.tsx');

    expect(groupingIndex).toBeGreaterThan(-1);
    expect(filterIndex).toBeGreaterThan(groupingIndex);
    expect(addIndex).toBeGreaterThan(filterIndex);
    expect(groupingToggle).toContain('data-testid="nav-workspace-session-view-toggle"');
    expect(groupingToggle).toContain('const ViewIcon = isAll ? List : ListTree');
    expect(groupingToggle).toContain('data-session-view-icon={grouping}');
    expect(groupingToggle).toContain('getNextWorkspaceSessionGrouping(grouping)');
    expect(groupingToggle).not.toContain('VIEW_OPTIONS');
    expect(groupingToggle).not.toContain('nav-session-view-all');
    expect(groupingToggle).not.toContain('nav-session-view-grouped');
    expect(filterMenu).toContain('data-testid="nav-session-filter-btn"');
    expect(filterMenu).toContain("type Submenu = 'ordering' | 'show'");
    expect(filterMenu).not.toContain("row('grouping'");
    expect(filterMenu).toContain("{row('status'");
    expect(filterMenu).toContain("{row('environment'");
    expect(filterMenu).toContain("{row('source'");
    expect(filterMenu).toContain("t('nav.sessions.viewMenu.collapseAll')");
    expect(filterMenu).toContain("t('nav.sessions.viewMenu.markAllRead')");
  });

  it('keeps global utilities outside the scroll root and docks one session filter header', () => {
    const mainNav = source('./MainNav.tsx');
    const navPanel = source('./NavPanel.tsx');
    const sectionHeader = source('./components/SectionHeader.tsx');
    const stickyHeader = source('./components/StickySectionHeader.tsx');
    const navStyles = source('./NavPanel.scss');
    const brandHeaderIndex = mainNav.indexOf('data-bf-part="brandHeader"');
    const sectionsIndex = mainNav.indexOf('data-testid="nav-sections"');
    const contentIndex = navPanel.indexOf('data-bf-part="content"');
    const persistentFooterIndex = navPanel.indexOf('<PersistentFooterActions />');

    expect(brandHeaderIndex).toBeGreaterThan(-1);
    expect(sectionsIndex).toBeGreaterThan(brandHeaderIndex);
    expect(contentIndex).toBeGreaterThan(-1);
    expect(persistentFooterIndex).toBeGreaterThan(contentIndex);
    expect(mainNav).toContain('<NavigationPanelBody className="bitfun-nav-panel__sections" ref={sectionsScrollRef}>');
    expect(mainNav).toContain('<StickySectionHeader scrollRootRef={sectionsScrollRef}>');
    expect(mainNav).not.toContain('expandedSections');
    expect(mainNav).not.toContain('toggleSection');
    expect(mainNav).not.toContain('bitfun-nav-panel__collapsible');
    expect(sectionHeader).not.toContain('bitfun-nav-panel__section-header--interactive');
    expect(sectionHeader).not.toContain('aria-expanded');
    expect(stickyHeader).toContain('new IntersectionObserver');
    expect(stickyHeader).toContain('root: scrollRoot');
    expect(stickyHeader).toContain('{children}');
    expect(stickyHeader).toContain('data-testid="nav-sessions-sticky-header"');
    expect(stickyHeader).toContain('data-bf-state={isStuck ? \'stuck\' : undefined}');
    expect(navStyles).toContain('&__sticky-section-header');
    expect(navStyles).toContain('position: sticky;');
    expect(navStyles).toContain('top: 0;');
  });

  it('projects the same session model as mixed groups or one flat list', () => {
    const workspaceList = source('./sections/workspaces/WorkspaceListSection.tsx');
    const projection = source('./sessionNavigationProjection.ts');
    const sessionsSection = source('./sections/sessions/SessionsSection.tsx');

    expect(workspaceList).toContain("grouping === 'all'");
    expect(workspaceList).toContain('data-session-group-kind={group.kind}');
    expect(projection).toContain("workspace.workspaceKind === 'assistant'");
    expect(workspaceList).toContain('workspaceScopes={workspaceScopes}');
    expect(workspaceList).toContain('layout="flat"');
    expect(sessionsSection).toContain("'sessions_nav_all_grouping'");
    expect(sessionsSection).toContain('Promise.allSettled(workspaceScopes.map');
    expect(sessionsSection).toContain('matchesWorkspaceSessionView(');
    expect(sessionsSection).toContain('loadArchivedSessionMetadata(');
    expect(sessionsSection).toContain("layout === 'flat' ? ' is-flat-workspace-view'");
    expect(sessionsSection).toContain("const showAllWithoutLimit = layout === 'flat'");
    expect(sessionsSection).toContain('!showAllWithoutLimit && expandLevel === 2');
    expect(sessionsSection).toContain('!showAllWithoutLimit && expandToggleState.shouldRender');
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
