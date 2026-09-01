/**
 * MainNav — primary product navigation sidebar.
 *
 * Layout (top to bottom):
 *   1. Search and New Session
 *   2. AI Assistant, Task Board, Mini Apps, then Extensions & Compatibility
 *   3. Unified Sessions (all or grouped by project / assistant)
 *
 * When a scene-nav transition is active (`isDeparting=true`), items receive
 * positional CSS classes for the split-open animation effect.
 */

import React, { useCallback, useState, useMemo, useEffect, useRef, useSyncExternalStore } from 'react';
import { createPortal } from 'react-dom';
import {
  Icon,
  KeyHint,
  Menu,
  MenuItem,
  MenuSection,
  MenuSeparator,
  NavigationPanel,
  NavigationPanelBody,
  NavigationPanelContent,
  NavigationPanelHeader,
  ScrollArea,
  Tooltip,
} from '@bitfun/ui';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { isImeOwnedKeyboardEvent } from '@/shared/utils/ime';
import { Plus, FolderOpen, FolderPlus, History, Users, Network, CalendarClock } from 'lucide-react';
// import { PanelsTopLeft } from 'lucide-react'; // temporarily hidden: Pages nav entry
import { BITFUN_ICON_SIZE, NavigationExtensionsCompatibilityIcon, NavigationSessionContextAddIcon } from '@/app/icons';
import { useSceneManager } from '../../hooks/useSceneManager';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import type { SceneTabId } from '../SceneBar/types';
import SectionHeader from './components/SectionHeader';
import StickySectionHeader from './components/StickySectionHeader';
import WorkspaceSessionGroupingToggle from './components/WorkspaceSessionGroupingToggle';
import WorkspaceSessionFilterMenu from './components/WorkspaceSessionFilterMenu';
import MiniAppEntry from './components/MiniAppEntry';
import WorkspaceListSection from './sections/workspaces/WorkspaceListSection';
import { useSceneStore } from '../../stores/sceneStore';
import { useMiniAppCatalogSync } from '../../scenes/miniapps/hooks/useMiniAppCatalogSync';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { isRemoteWorkspace } from '@/shared/types';
import { getRecentWorkspaceLineParts } from '@/shared/utils/recentWorkspaceDisplay';
import { computeFixedPopoverPosition } from '@/shared/utils/fixedPopoverViewport';
import { useSSHRemoteContext, SSHConnectionDialog, RemoteFileBrowser } from '@/features/ssh-remote';
import { openGlobalSearch } from '@/app/global-search/globalSearchStore';
import { activateProductAction } from '@/app/global-search/productActionActivator';
import {
  getGlobalSearchShortcutLabel,
  splitGlobalSearchShortcutLabel,
  subscribeGlobalSearchShortcut,
} from '@/app/global-search/globalSearchShortcut';
import { useExternalAppAwareness } from '@/infrastructure/config/components/external-sources/useExternalAppAwareness';

import './NavPanel.scss';

const log = createLogger('MainNav');

interface MainNavProps {
  isDeparting?: boolean;
  anchorNavSceneId?: SceneTabId | null;
}

const MainNav: React.FC<MainNavProps> = ({
  isDeparting: _isDeparting = false,
  anchorNavSceneId: _anchorNavSceneId = null,
}) => {
  const sshRemote = useSSHRemoteContext();
  const [isSSHConnectionDialogOpen, setIsSSHConnectionDialogOpen] = useState(false);

  useEffect(() => {
    if (sshRemote.showFileBrowser) {
      setIsSSHConnectionDialogOpen(false);
    }
  }, [sshRemote.showFileBrowser]);

  const { openScene } = useSceneManager();
  const activeTabId = useSceneStore(s => s.activeTabId);
  const { t } = useI18n('common');
  const searchShortcutLabel = useSyncExternalStore(
    subscribeGlobalSearchShortcut,
    getGlobalSearchShortcutLabel,
    getGlobalSearchShortcutLabel,
  );
  const searchShortcutHint = splitGlobalSearchShortcutLabel(searchShortcutLabel);
  // const { t: tPages } = useI18n('scenes/pages'); // temporarily hidden: Pages nav entry
  const {
    currentWorkspace,
    loading: workspaceLoading,
    recentWorkspaces,
    openedWorkspacesList,
    switchWorkspace,
  } = useWorkspaceContext();

  useMiniAppCatalogSync({
    enabled: !workspaceLoading,
    initialLoad: 'idle',
  });

  const activeMiniAppId = useMemo(
    () => (typeof activeTabId === 'string' && activeTabId.startsWith('miniapp:') ? activeTabId.slice('miniapp:'.length) : null),
    [activeTabId]
  );

  // Section expand state
  const [expandedSections, setExpandedSections] = useState<Set<string>>(
    () => new Set(['sessions'])
  );

  const workspaceMenuButtonRef = useRef<HTMLButtonElement | null>(null);
  const workspaceMenuRef = useRef<HTMLDivElement | null>(null);
  const sectionsScrollRef = useRef<HTMLDivElement | null>(null);
  const [workspaceMenuOpen, setWorkspaceMenuOpen] = useState(false);
  const [workspaceMenuClosing, setWorkspaceMenuClosing] = useState(false);
  const [workspaceMenuPos, setWorkspaceMenuPos] = useState({ top: 0, left: 0 });
  const [isExtensionsOpen, setIsExtensionsOpen] = useState(false);

  const toggleSection = useCallback((id: string) => {
    setExpandedSections(prev => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const closeWorkspaceMenu = useCallback(() => {
    setWorkspaceMenuClosing(true);
    window.setTimeout(() => {
      setWorkspaceMenuOpen(false);
      setWorkspaceMenuClosing(false);
    }, 150);
  }, []);

  const updateWorkspaceMenuPos = useCallback(() => {
    const btn = workspaceMenuButtonRef.current;
    if (!btn || !workspaceMenuOpen) return;
    const rect = btn.getBoundingClientRect();
    const viewportPadding = 8;
    const gap = 6;
    const fallbackWidth = 300;
    const fallbackHeight = 420;

    const apply = () => {
      const menuEl = workspaceMenuRef.current;
      const w = menuEl?.offsetWidth ?? fallbackWidth;
      const h = menuEl?.offsetHeight ?? fallbackHeight;
      setWorkspaceMenuPos(computeFixedPopoverPosition(rect, w, h, gap, viewportPadding));
    };

    apply();
    requestAnimationFrame(apply);
  }, [workspaceMenuOpen]);

  const openWorkspaceMenu = useCallback(async () => {
    try {
      await workspaceManager.cleanupInvalidWorkspaces();
    } catch (error) {
      log.warn('Failed to cleanup invalid workspaces before opening workspace menu', { error });
    }
    const rect = workspaceMenuButtonRef.current?.getBoundingClientRect();
    if (!rect) return;
    setWorkspaceMenuPos(computeFixedPopoverPosition(rect, 300, 420, 6, 8));
    setWorkspaceMenuOpen(true);
    setWorkspaceMenuClosing(false);
  }, []);

  const toggleWorkspaceMenu = useCallback(() => {
    if (workspaceMenuOpen) { closeWorkspaceMenu(); return; }
    void openWorkspaceMenu();
  }, [closeWorkspaceMenu, openWorkspaceMenu, workspaceMenuOpen]);

  const handleOpenProject = useCallback(async () => {
    try {
      await activateProductAction('project.open', { t });
    } catch (err) {
      log.error('Failed to open project', err);
    }
  }, [t]);

  const handleNewProject = useCallback(() => {
    void activateProductAction('project.new');
  }, []);

  const handleOpenAssistantManager = useCallback(() => {
    closeWorkspaceMenu();
    openScene('assistant');
  }, [closeWorkspaceMenu, openScene]);

  const handleSwitchWorkspace = useCallback(async (workspaceId: string) => {
    const targetWorkspace = recentWorkspaces.find(item => item.id === workspaceId);
    if (!targetWorkspace) return;
    closeWorkspaceMenu();
    await switchWorkspace(targetWorkspace);
  }, [closeWorkspaceMenu, recentWorkspaces, switchWorkspace]);

  const handleOpenRemoteSSH = useCallback(() => {
    closeWorkspaceMenu();
    setIsSSHConnectionDialogOpen(true);
  }, [closeWorkspaceMenu]);

  const handleSelectRemoteWorkspace = useCallback(async (path: string) => {
    try {
      await sshRemote.openWorkspace(path);
      sshRemote.setShowFileBrowser(false);
      setIsSSHConnectionDialogOpen(false);
    } catch (err) {
      log.error('Failed to open remote workspace', err);
    }
  }, [sshRemote]);

  useEffect(() => {
    if (!workspaceMenuOpen) return;
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (workspaceMenuButtonRef.current?.contains(target)) return;
      if (workspaceMenuRef.current?.contains(target)) return;
      closeWorkspaceMenu();
    };
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isImeOwnedKeyboardEvent(event)) closeWorkspaceMenu();
    };
    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [closeWorkspaceMenu, workspaceMenuOpen]);

  useEffect(() => {
    if (!workspaceMenuOpen) return;

    updateWorkspaceMenuPos();

    const handleViewportChange = () => updateWorkspaceMenuPos();
    window.addEventListener('resize', handleViewportChange);
    window.addEventListener('scroll', handleViewportChange, true);

    return () => {
      window.removeEventListener('resize', handleViewportChange);
      window.removeEventListener('scroll', handleViewportChange, true);
    };
  }, [workspaceMenuOpen, updateWorkspaceMenuPos]);

  const handleCreateSession = useCallback(() => {
    void activateProductAction('session.new');
  }, []);

  const handleOpenAgents = useCallback(() => {
    void activateProductAction('surface.agents.open');
  }, []);

  const handleOpenTodos = useCallback(() => {
    void activateProductAction('surface.todos.open');
  }, []);

  const handleOpenSkills = useCallback(() => {
    void activateProductAction('surface.skills.open');
  }, []);

  const handleOpenEcosystemCompatibility = useCallback(() => {
    void activateProductAction('surface.ecosystemCompatibility.open');
  }, []);

  const isAgentsActive = activeTabId === 'agents';
  const isSkillsActive = activeTabId === 'skills';
  const isEcosystemCompatibilityActive = activeTabId === 'ecosystem-compatibility';
  const hasUnseenEcosystemCompatibility = useExternalAppAwareness(
    isEcosystemCompatibilityActive,
  );

  useEffect(() => {
    if (isAgentsActive || isSkillsActive || isEcosystemCompatibilityActive) {
      setIsExtensionsOpen(true);
    }
  }, [isAgentsActive, isEcosystemCompatibilityActive, isSkillsActive]);

  const workspaceMenuPortal = workspaceMenuOpen ? createPortal(
    <Menu
      ref={workspaceMenuRef}
      className={`bitfun-nav-panel__workspace-menu${workspaceMenuClosing ? ' is-closing' : ''}`}
      style={{ top: workspaceMenuPos.top, left: workspaceMenuPos.left }}
    >
      <MenuItem
        leading={<FolderOpen size={13} />}
        onClick={() => { closeWorkspaceMenu(); void handleOpenProject(); }}
      >
        {t('header.openProject')}
      </MenuItem>
      <MenuItem
        leading={<FolderPlus size={13} />}
        onClick={() => { closeWorkspaceMenu(); handleNewProject(); }}
      >
        {t('header.newProject')}
      </MenuItem>
      <MenuItem
        leading={<Icon name="user" size="xs" />}
        onClick={handleOpenAssistantManager}
        data-testid="nav-session-group-add-assistant"
      >
        {t('nav.workspaces.actions.newAssistant')}
      </MenuItem>
      <MenuItem
        leading={(
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
            <path d="M9 3H5a2 2 0 0 0-2 2v4m6-6h10a2 2 0 0 1 2 2v4M9 3v18m0 0h10a2 2 0 0 0 2-2v-4M9 21H5a2 2 0 0 1-2-2v-4m0-6v6" />
          </svg>
        )}
        onClick={handleOpenRemoteSSH}
      >
        {t('ssh.remote.connect')}
      </MenuItem>
      <MenuSeparator />
      <MenuSection
        title={(
          <>
            <History size={12} aria-hidden="true" />
            <span>{t('header.recentWorkspaces')}</span>
          </>
        )}
      >
        <ScrollArea className="bitfun-nav-panel__workspace-menu-workspaces">
        {recentWorkspaces.length === 0 ? (
          <div className="bitfun-nav-panel__workspace-menu-empty">
            <span>{t('header.noRecentWorkspaces')}</span>
          </div>
        ) : (
          recentWorkspaces.map((workspace) => {
            const { hostPrefix, folderLabel, tooltip } = getRecentWorkspaceLineParts(workspace);
            const isCurrent = workspace.id === currentWorkspace?.id;
            return (
              <MenuItem
                key={workspace.id}
                leading={<FolderOpen size={13} aria-hidden="true" />}
                role="menuitemradio"
                checked={isCurrent}
                metadata={isCurrent ? <Icon name="check-line" size="xs" /> : undefined}
                title={tooltip}
                onClick={() => { void handleSwitchWorkspace(workspace.id); }}
                data-testid="nav-workspace-menu-recent-workspace"
                data-workspace-id={workspace.id}
              >
                <span className="bitfun-nav-panel__workspace-menu-item-main">
                  {hostPrefix ? (
                    <>
                      <span className="bitfun-nav-panel__workspace-menu-item-host">{hostPrefix}</span>
                      <span className="bitfun-nav-panel__workspace-menu-item-host-sep" aria-hidden>
                        ·
                      </span>
                    </>
                  ) : null}
                  <span className="bitfun-nav-panel__workspace-menu-item-name">{folderLabel}</span>
                </span>
              </MenuItem>
            );
          })
        )}
        </ScrollArea>
      </MenuSection>
    </Menu>,
    getAppearanceOverlayHost()
  ) : null;

  const createSessionLabel = t('nav.sessions.newSession');
  const addSessionGroupTooltip = t('nav.tooltips.addSessionGroup');
  const agentsTooltip = t('nav.tooltips.agents');
  const skillsTooltip = t('nav.tooltips.skills');
  const ecosystemCompatibilityTooltip = hasUnseenEcosystemCompatibility
    ? t('nav.tooltips.ecosystemCompatibilityUnseen')
    : t('nav.tooltips.ecosystemCompatibility');
  const assistantManagerLabel = t('nav.items.assistant');
  const taskBoardLabel = t('nav.items.todos');
  const extensionsLabel = t('nav.sections.extensions');
  const isAssistantManagerActive = activeTabId === 'assistant' || activeTabId === 'profile';
  const isTaskBoardActive = activeTabId === 'todos';
  return (
    <>
    <NavigationPanel
      className="bitfun-nav-panel__main-nav"
    >
      <NavigationPanelHeader className="bitfun-nav-panel__main-nav-header">
        <div data-bf-component="nav-panel" data-bf-part="brandHeader" className="bitfun-nav-panel__brand-header">
        <div className="bitfun-nav-panel__utility-row" data-bf-component="nav-panel" data-bf-part="utilityRow">
          <div className="bitfun-nav-panel__brand-search" data-bf-component="nav-panel" data-bf-part="search">
            <Tooltip content={t('nav.search.triggerTooltip')} placement="right" followCursor>
              <button
                type="button"
                className="bitfun-nav-panel__search-trigger"
                data-bf-component="nav-panel"
                data-bf-part="searchTrigger"
                onClick={() => openGlobalSearch()}
                aria-label={t('nav.search.triggerTooltip')}
                data-testid="nav-search-trigger"
              >
                <span className="bitfun-nav-panel__search-trigger__icon" aria-hidden="true">
                  <span className="bitfun-nav-panel__search-trigger__icon-inner">
                    <Icon name="search" size="xs" />
                  </span>
                </span>
                <span className="bitfun-nav-panel__search-trigger__label">
                  {t('nav.search.triggerPlaceholder')}
                </span>
                <KeyHint
                  data-testid="nav-search-shortcut"
                  aria-hidden="true"
                  icon={searchShortcutHint.modifier}
                >
                  {searchShortcutHint.key}
                </KeyHint>
              </button>
            </Tooltip>
          </div>
          <Tooltip content={createSessionLabel} placement="right" followCursor>
            <button
              type="button"
              className="bitfun-nav-panel__utility-action"
              data-bf-component="nav-panel"
              data-bf-part="topAction"
              data-bf-action="new-session"
              onClick={handleCreateSession}
              aria-label={createSessionLabel}
              data-testid="nav-new-session-btn"
            >
              <Plus size={15} aria-hidden="true" />
            </button>
          </Tooltip>
        </div>
        </div>
      </NavigationPanelHeader>
      <NavigationPanelBody className="bitfun-nav-panel__sections" ref={sectionsScrollRef}>
        <NavigationPanelContent className="bitfun-nav-panel__main-nav-content">
        <div data-testid="nav-sections" className="bitfun-nav-panel__sections-slot">
        <div data-bf-component="nav-panel" data-bf-part="topActions" className="bitfun-nav-panel__top-actions">
          <Tooltip content={assistantManagerLabel} placement="right" followCursor>
            <button
              type="button"
              className={[
                'bitfun-nav-panel__top-action-btn',
                isAssistantManagerActive ? 'is-active' : '',
              ].filter(Boolean).join(' ')}
              data-bf-component="nav-panel"
              data-bf-part="topAction"
              data-bf-action="assistant-manager"
              data-bf-state={isAssistantManagerActive ? 'active' : ''}
              onClick={handleOpenAssistantManager}
              aria-label={assistantManagerLabel}
              data-testid="nav-assistant-manager"
            >
              <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                <Icon name="user" size="sm" />
              </span>
              <span>{assistantManagerLabel}</span>
            </button>
          </Tooltip>

          <Tooltip content={t('nav.tooltips.todos')} placement="right" followCursor>
            <button
              type="button"
              className={[
                'bitfun-nav-panel__top-action-btn',
                isTaskBoardActive ? 'is-active' : '',
              ].filter(Boolean).join(' ')}
              data-bf-component="nav-panel"
              data-bf-part="todoEntry"
              data-bf-action="todos"
              data-bf-state={isTaskBoardActive ? 'active' : ''}
              onClick={handleOpenTodos}
              aria-label={taskBoardLabel}
              aria-pressed={isTaskBoardActive}
              data-testid="nav-todos-btn"
            >
              <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                <CalendarClock size={15} />
              </span>
              <span>{taskBoardLabel}</span>
            </button>
          </Tooltip>

          <div className="bitfun-nav-panel__miniapp-navigation" data-bf-component="nav-panel" data-bf-part="miniAppFooter">
            <MiniAppEntry
              isActive={activeTabId === 'miniapps' || !!activeMiniAppId}
              activeMiniAppId={activeMiniAppId}
              onOpenMiniApps={() => openScene('miniapps')}
              onOpenMiniApp={(appId) => openScene(`miniapp:${appId}`)}
            />
          </div>

          <div className="bitfun-nav-panel__top-action-expand" data-bf-component="nav-panel" data-bf-part="extensionGroup" data-bf-state={isExtensionsOpen ? 'open' : ''} data-testid="agent-skill-panel">
            <Tooltip content={extensionsLabel} placement="right" followCursor>
              <button
                type="button"
                className={[
                  'bitfun-nav-panel__top-action-btn',
                  'bitfun-nav-panel__top-action-btn--expand',
                  isExtensionsOpen ? 'is-open' : '',
                ].filter(Boolean).join(' ')}
                data-bf-component="nav-panel"
                data-bf-part="topAction"
                data-bf-action="extensions"
                data-bf-state={isExtensionsOpen ? 'open' : ''}
                onClick={() => setIsExtensionsOpen(v => !v)}
                aria-expanded={isExtensionsOpen}
                aria-label={extensionsLabel}
                data-testid="agent-skill-entry"
              >
                <span className="bitfun-nav-panel__top-action-expand-icons" aria-hidden="true">
                  <NavigationExtensionsCompatibilityIcon
                    size={BITFUN_ICON_SIZE.navigation}
                    className="bitfun-nav-panel__top-action-expand-icon-default"
                  />
                  <Icon
                    name="chevron-down"
                    size="sm"
                    className={[
                      'bitfun-nav-panel__top-action-expand-icon-chevron',
                      isExtensionsOpen ? 'is-open' : '',
                    ].filter(Boolean).join(' ')}
                  />
                </span>
                <span>{extensionsLabel}</span>
              </button>
            </Tooltip>

            <div
              className={`bitfun-nav-panel__top-action-sublist${isExtensionsOpen ? ' is-open' : ''}`}
              data-testid="agent-skill-tabs"
            >
              <Tooltip content={agentsTooltip} placement="right" followCursor>
                <button
                  type="button"
                  className={[
                    'bitfun-nav-panel__top-action-btn',
                    'bitfun-nav-panel__top-action-btn--sub',
                    isAgentsActive ? 'is-active' : '',
                  ].filter(Boolean).join(' ')}
                  data-bf-component="nav-panel"
                  data-bf-part="topAction"
                  data-bf-action="agents"
                  data-bf-state={isAgentsActive ? 'active' : ''}
                  onClick={handleOpenAgents}
                  aria-label={agentsTooltip}
                  data-testid="agent-tab"
                >
                  <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                    <Users size={15} />
                  </span>
                  <span>{t('nav.items.agents')}</span>
                </button>
              </Tooltip>

              <Tooltip content={skillsTooltip} placement="right" followCursor>
                <button
                  type="button"
                  className={[
                    'bitfun-nav-panel__top-action-btn',
                    'bitfun-nav-panel__top-action-btn--sub',
                    isSkillsActive ? 'is-active' : '',
                  ].filter(Boolean).join(' ')}
                  data-bf-component="nav-panel"
                  data-bf-part="topAction"
                  data-bf-action="skills"
                  data-bf-state={isSkillsActive ? 'active' : ''}
                  onClick={handleOpenSkills}
                  aria-label={skillsTooltip}
                  data-testid="skill-tab"
                >
                  <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                    <Icon name="extension" size="sm" />
                  </span>
                  <span>{t('nav.items.skills')}</span>
                </button>
              </Tooltip>

              <Tooltip content={ecosystemCompatibilityTooltip} placement="right" followCursor>
                <button
                  type="button"
                  className={[
                    'bitfun-nav-panel__top-action-btn',
                    'bitfun-nav-panel__top-action-btn--sub',
                    isEcosystemCompatibilityActive ? 'is-active' : '',
                  ].filter(Boolean).join(' ')}
                  data-bf-component="nav-panel"
                  data-bf-part="topAction"
                  data-bf-action="ecosystem-compatibility"
                  data-bf-state={isEcosystemCompatibilityActive ? 'active' : ''}
                  onClick={handleOpenEcosystemCompatibility}
                  aria-label={ecosystemCompatibilityTooltip}
                  data-testid="ecosystem-compatibility-tab"
                >
                  <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                    <Network size={15} />
                  </span>
                  <span>{t('nav.items.ecosystemCompatibility')}</span>
                  {hasUnseenEcosystemCompatibility ? (
                    <span
                      className="bitfun-nav-panel__top-action-unseen"
                      data-bf-component="nav-panel"
                      data-bf-part="topActionUnseen"
                      aria-hidden="true"
                    />
                  ) : null}
                </button>
              </Tooltip>
            </div>
          </div>
        </div>

        {/* Unified sessions */}
        <div className="bitfun-nav-panel__section" data-bf-component="nav-panel" data-bf-part="section" data-bf-section="sessions">
          <StickySectionHeader scrollRootRef={sectionsScrollRef}>
            <SectionHeader
              label={t('nav.items.sessions')}
              collapsible
              isOpen={expandedSections.has('sessions')}
              onToggle={() => toggleSection('sessions')}
              actions={
                <>
                  <WorkspaceSessionGroupingToggle />
                  <WorkspaceSessionFilterMenu />
                  <div className="bitfun-nav-panel__workspace-action-wrap">
                    <Tooltip content={addSessionGroupTooltip} placement="right" followCursor disabled={workspaceMenuOpen}>
                      <button
                        ref={workspaceMenuButtonRef}
                        type="button"
                        className={`bitfun-nav-panel__section-action${workspaceMenuOpen ? ' is-active' : ''}`}
                        aria-label={addSessionGroupTooltip}
                        aria-haspopup="menu"
                        aria-expanded={workspaceMenuOpen}
                        onClick={toggleWorkspaceMenu}
                        data-testid="nav-workspace-add-btn"
                      >
                        <NavigationSessionContextAddIcon size={BITFUN_ICON_SIZE.compact} />
                      </button>
                    </Tooltip>
                  </div>
                </>
              }
            />
          </StickySectionHeader>
          <div className={`bitfun-nav-panel__collapsible${expandedSections.has('sessions') ? '' : ' is-collapsed'}`} data-bf-component="nav-panel" data-bf-part="sectionContent" data-bf-state={expandedSections.has('sessions') ? 'open' : ''}>
            <div className="bitfun-nav-panel__collapsible-inner">
              <div className="bitfun-nav-panel__items">
                <WorkspaceListSection variant="all" />
              </div>
            </div>
          </div>
        </div>
        </div>
        </NavigationPanelContent>
      </NavigationPanelBody>
    </NavigationPanel>

      {workspaceMenuPortal}

      {/* SSH Remote Dialogs */}
      <SSHConnectionDialog
        open={isSSHConnectionDialogOpen}
        onClose={() => setIsSSHConnectionDialogOpen(false)}
      />
      {sshRemote.showFileBrowser && sshRemote.connectionId && (
        <RemoteFileBrowser
          connectionId={sshRemote.connectionId}
          initialPath={sshRemote.remoteFileBrowserInitialPath}
          homePath={sshRemote.remoteFileBrowserInitialPath}
          selectDirectoriesOnly
          onSelect={handleSelectRemoteWorkspace}
          onCancel={() => {
            const hasActiveRemoteWorkspace =
              Boolean(sshRemote.remoteWorkspace) ||
              openedWorkspacesList.some(workspace =>
                isRemoteWorkspace(workspace) &&
                workspace.connectionId === sshRemote.connectionId
              );
            sshRemote.setShowFileBrowser(false);
            if (!hasActiveRemoteWorkspace) {
              void sshRemote.disconnect();
            }
          }}
        />
      )}
    </>
  );
};

export default MainNav;
