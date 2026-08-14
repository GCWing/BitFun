/**
 * MainNav — default workspace navigation sidebar.
 *
 * Layout (top to bottom):
 *   1. Search and Todo
 *   2. New Session, Smart Members, and Long-term Tracking
 *   3. Workspace
 *   4. Bottom: Extensions & Compatibility, MiniApp
 *
 * When a scene-nav transition is active (`isDeparting=true`), items receive
 * positional CSS classes for the split-open animation effect.
 */

import React, { useCallback, useState, useMemo, useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { Plus, FolderOpen, FolderPlus, History, Check, User, Users, Puzzle, Blocks, CalendarClock, ChevronDown, Goal, Network, Search } from 'lucide-react';
// import { PanelsTopLeft } from 'lucide-react'; // temporarily hidden: Pages nav entry
import { Tooltip } from '@/component-library';
import { useSceneManager } from '../../hooks/useSceneManager';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import type { SceneTabId } from '../SceneBar/types';
import SectionHeader from './components/SectionHeader';
import AssistantSessionCreateMenu from './components/AssistantSessionCreateMenu';
import WorkspaceSessionFilterMenu from './components/WorkspaceSessionFilterMenu';
import MiniAppEntry from './components/MiniAppEntry';
import WorkspaceListSection from './sections/workspaces/WorkspaceListSection';
import SessionsSection from './sections/sessions/SessionsSection';
import { useSceneStore } from '../../stores/sceneStore';
import { useSettingsStore } from '../../scenes/settings/settingsStore';
import { useMiniAppCatalogSync } from '../../scenes/miniapps/hooks/useMiniAppCatalogSync';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { openMainSession } from '@/flow_chat/services/sessionActivation';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { notificationService } from '@/shared/notification-system';
import { WorkspaceKind, isRemoteWorkspace, type WorkspaceInfo } from '@/shared/types';
import {
  flowChatSessionConfigForWorkspace,
  pickPrimaryAssistantWorkspace,
} from '@/app/utils/projectSessionWorkspace';
import { getRecentWorkspaceLineParts } from '@/shared/utils/recentWorkspaceDisplay';
import { computeFixedPopoverPosition } from '@/shared/utils/fixedPopoverViewport';
import { useSSHRemoteContext, SSHConnectionDialog, RemoteFileBrowser } from '@/features/ssh-remote';
import NavSearchDialog from './NavSearchDialog';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { ALL_SHORTCUTS } from '@/shared/constants/shortcuts';

import './NavPanel.scss';

const NAV_TOGGLE_SEARCH_DEF = ALL_SHORTCUTS.find((d) => d.id === 'nav.toggleSearch')!;

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
  const activeSettingsTab = useSettingsStore(s => s.activeTab);
  const { t } = useI18n('common');
  // const { t: tPages } = useI18n('scenes/pages'); // temporarily hidden: Pages nav entry
  const {
    currentWorkspace,
    loading: workspaceLoading,
    recentWorkspaces,
    openedWorkspacesList,
    assistantWorkspacesList,
    primaryAssistantWorkspaceId,
    switchWorkspace,
    setActiveWorkspace,
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
    () => new Set(['workspace'])
  );

  const workspaceMenuButtonRef = useRef<HTMLButtonElement | null>(null);
  const workspaceMenuRef = useRef<HTMLDivElement | null>(null);
  const [workspaceMenuOpen, setWorkspaceMenuOpen] = useState(false);
  const [workspaceMenuClosing, setWorkspaceMenuClosing] = useState(false);
  const [workspaceMenuPos, setWorkspaceMenuPos] = useState({ top: 0, left: 0 });
  const [isExtensionsOpen, setIsExtensionsOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);

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

  const primaryAssistantWorkspace = useMemo(
    () => pickPrimaryAssistantWorkspace(assistantWorkspacesList, primaryAssistantWorkspaceId),
    [assistantWorkspacesList, primaryAssistantWorkspaceId]
  );

  const orderedAssistantWorkspacesList = useMemo(
    () => primaryAssistantWorkspace
      ? [
          primaryAssistantWorkspace,
          ...assistantWorkspacesList.filter(workspace => workspace.id !== primaryAssistantWorkspace.id),
        ]
      : assistantWorkspacesList,
    [assistantWorkspacesList, primaryAssistantWorkspace]
  );

  const toggleNavSearch = useCallback(() => {
    setSearchOpen((v) => !v);
  }, []);

  useShortcut(
    NAV_TOGGLE_SEARCH_DEF.id,
    NAV_TOGGLE_SEARCH_DEF.config,
    toggleNavSearch,
    { priority: 5, description: NAV_TOGGLE_SEARCH_DEF.descriptionKey }
  );

  // Secondary binding (not listed separately in keyboard settings — same action as Mod+K)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        !e.altKey ||
        e.ctrlKey ||
        e.metaKey ||
        e.shiftKey ||
        e.key.toLowerCase() !== 'f'
      ) {
        return;
      }
      e.preventDefault();
      toggleNavSearch();
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [toggleNavSearch]);

  const handleCreateAssistantSession = useCallback(async (workspace: WorkspaceInfo) => {
    try {
      const sessionId = await flowChatManager.createChatSession(
        flowChatSessionConfigForWorkspace(workspace),
        'Claw'
      );
      await openMainSession(sessionId, {
        workspaceId: workspace.id,
        activateWorkspace: setActiveWorkspace,
      });
    } catch (error) {
      log.error('Failed to create assistant session', { workspaceId: workspace.id, error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.createSessionFailed'),
        { duration: 4000 }
      );
    }
  }, [setActiveWorkspace, t]);

  const handleCreatePrimaryAssistantSession = useCallback(async () => {
    if (!primaryAssistantWorkspace) {
      notificationService.warning(t('nav.workspaces.createSessionFailed'), { duration: 4000 });
      return;
    }
    await handleCreateAssistantSession(primaryAssistantWorkspace);
  }, [handleCreateAssistantSession, primaryAssistantWorkspace, t]);

  const handleOpenProject = useCallback(async () => {
    try {
      const { pickWorkspaceDirectory } = await import(
        '@/infrastructure/peer-device/pickWorkspaceDirectory'
      );
      const selected = await pickWorkspaceDirectory({
        title: t('header.selectProjectDirectory'),
      });
      if (selected) {
        await workspaceManager.openWorkspace(selected);
      }
    } catch (err) {
      log.error('Failed to open project', err);
    }
  }, [t]);

  const handleNewProject = useCallback(() => {
    window.dispatchEvent(new Event('nav:new-project'));
  }, []);

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
      if (event.key === 'Escape') closeWorkspaceMenu();
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

  const handleOpenTodos = useCallback(() => {
    openScene('todos');
  }, [openScene]);

  const handleCreateSession = useCallback(() => {
    window.dispatchEvent(new CustomEvent('toolbar-create-session'));
  }, []);

  const handleOpenLongTermTracking = useCallback(() => {
    notificationService.info(t('nav.messages.longTermTrackingComingSoon'), { duration: 3200 });
  }, [t]);

  const handleOpenAgents = useCallback(() => {
    openScene('agents');
  }, [openScene]);

  const handleOpenSkills = useCallback(() => {
    openScene('skills');
  }, [openScene]);

  const handleOpenEcosystemCompatibility = useCallback(() => {
    useSettingsStore.getState().openTab('external-sources');
    openScene('settings');
  }, [openScene]);

  const isAgentsActive = activeTabId === 'agents';
  const isSkillsActive = activeTabId === 'skills';

  useEffect(() => {
    if (isAgentsActive || isSkillsActive) {
      setIsExtensionsOpen(true);
    }
  }, [isAgentsActive, isSkillsActive]);

  const workspaceMenuPortal = workspaceMenuOpen ? createPortal(
    <div
      ref={workspaceMenuRef}
      className={`bitfun-nav-panel__workspace-menu${workspaceMenuClosing ? ' is-closing' : ''}`}
      data-bf-component="nav-panel"
      data-bf-part="workspaceMenu"
      data-bf-state={workspaceMenuClosing ? 'closing' : 'open'}
      role="menu"
      style={{ top: workspaceMenuPos.top, left: workspaceMenuPos.left }}
    >
      <button
        type="button"
        className="bitfun-nav-panel__workspace-menu-item"
        data-bf-component="nav-panel"
        data-bf-part="workspaceMenuItem"
        role="menuitem"
        onClick={() => { closeWorkspaceMenu(); void handleOpenProject(); }}
      >
        <FolderOpen size={13} />
        <span>{t('header.openProject')}</span>
      </button>
      <button
        type="button"
        className="bitfun-nav-panel__workspace-menu-item"
        data-bf-component="nav-panel"
        data-bf-part="workspaceMenuItem"
        role="menuitem"
        onClick={() => { closeWorkspaceMenu(); handleNewProject(); }}
      >
        <FolderPlus size={13} />
        <span>{t('header.newProject')}</span>
      </button>
      <button
        type="button"
        className="bitfun-nav-panel__workspace-menu-item"
        data-bf-component="nav-panel"
        data-bf-part="workspaceMenuItem"
        role="menuitem"
        onClick={handleOpenRemoteSSH}
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
          <path d="M9 3H5a2 2 0 0 0-2 2v4m6-6h10a2 2 0 0 1 2 2v4M9 3v18m0 0h10a2 2 0 0 0 2-2v-4M9 21H5a2 2 0 0 1-2-2v-4m0-6v6" />
        </svg>
        <span>{t('ssh.remote.connect')}</span>
      </button>
      <div className="bitfun-nav-panel__workspace-menu-divider" data-bf-component="nav-panel" data-bf-part="workspaceMenuDivider" role="separator" />
      <div className="bitfun-nav-panel__workspace-menu-section-title" data-bf-component="nav-panel" data-bf-part="workspaceMenuTitle">
        <History size={12} aria-hidden="true" />
        <span>{t('header.recentWorkspaces')}</span>
      </div>
      {recentWorkspaces.length === 0 ? (
        <div className="bitfun-nav-panel__workspace-menu-empty" data-bf-component="nav-panel" data-bf-part="workspaceMenuEmpty" data-bf-state="empty">
          <span>{t('header.noRecentWorkspaces')}</span>
        </div>
      ) : (
        <div className="bitfun-nav-panel__workspace-menu-workspaces">
          {recentWorkspaces.map((workspace) => {
            const { hostPrefix, folderLabel, tooltip } = getRecentWorkspaceLineParts(workspace);
            return (
            <button data-bf-component="nav-panel" data-bf-part="workspaceMenuItem"
              key={workspace.id}
              type="button"
              className="bitfun-nav-panel__workspace-menu-item bitfun-nav-panel__workspace-menu-item--workspace"
              role="menuitem"
              title={tooltip}
              onClick={() => { void handleSwitchWorkspace(workspace.id); }}
              data-testid="nav-workspace-menu-recent-workspace"
              data-workspace-id={workspace.id}
            >
              <FolderOpen size={13} aria-hidden="true" />
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
              {workspace.id === currentWorkspace?.id ? <Check size={12} aria-hidden="true" /> : null}
            </button>
            );
          })}
        </div>
      )}
    </div>,
    getAppearanceOverlayHost()
  ) : null;

  const assistantTooltip = t('nav.items.persona');
  const createSessionLabel = t('nav.sessions.newSession');
  const todosTooltip = t('nav.tooltips.todos');
  const longTermTrackingTooltip = t('nav.tooltips.longTermTracking');
  const addWorkspaceTooltip = t('nav.tooltips.addWorkspace');
  const isTodosActive = activeTabId === 'todos';
  const agentsTooltip = t('nav.tooltips.agents');
  const skillsTooltip = t('nav.tooltips.skills');
  const ecosystemCompatibilityTooltip = t('nav.tooltips.ecosystemCompatibility');
  const extensionsLabel = t('nav.sections.extensions');
  const isEcosystemCompatibilityActive = activeTabId === 'settings' && activeSettingsTab === 'external-sources';
  return (
    <>
      {/* ── Search and Todo ────────────────────────── */}
      <div data-bf-component="nav-panel" data-bf-part="brandHeader" className="bitfun-nav-panel__brand-header">
        <div className="bitfun-nav-panel__utility-row" data-bf-component="nav-panel" data-bf-part="utilityRow">
          <div className="bitfun-nav-panel__brand-search" data-bf-component="nav-panel" data-bf-part="search">
            <Tooltip content={t('nav.search.triggerTooltip')} placement="right" followCursor>
              <button
                type="button"
                className="bitfun-nav-panel__search-trigger"
                data-bf-component="nav-panel"
                data-bf-part="searchTrigger"
                onClick={() => setSearchOpen(true)}
                aria-label={t('nav.search.triggerTooltip')}
                data-testid="nav-search-trigger"
              >
                <span className="bitfun-nav-panel__search-trigger__icon" aria-hidden="true">
                  <span className="bitfun-nav-panel__search-trigger__icon-inner">
                    <Search size={13} />
                  </span>
                </span>
                <span className="bitfun-nav-panel__search-trigger__label">
                  {t('nav.search.triggerPlaceholder')}
                </span>
              </button>
            </Tooltip>
            <NavSearchDialog open={searchOpen} onClose={() => setSearchOpen(false)} />
          </div>
          <Tooltip content={todosTooltip} placement="right" followCursor>
            <button
              type="button"
              className={`bitfun-nav-panel__todo-entry${isTodosActive ? ' is-active' : ''}`}
              data-bf-component="nav-panel"
              data-bf-part="todoEntry"
              data-bf-action="todos"
              data-bf-state={isTodosActive ? 'active' : ''}
              onClick={handleOpenTodos}
              aria-label={todosTooltip}
              data-testid="nav-todos-btn"
            >
              <CalendarClock size={15} aria-hidden="true" />
            </button>
          </Tooltip>
        </div>
      </div>

      {/* ── Long-lived navigation ──────────────────── */}
      <div data-bf-component="nav-panel" data-bf-part="sections" className="bitfun-nav-panel__sections" data-testid="nav-sections">
        <div data-bf-component="nav-panel" data-bf-part="topActions" className="bitfun-nav-panel__top-actions">
          <Tooltip content={createSessionLabel} placement="right" followCursor>
            <button
              type="button"
              className="bitfun-nav-panel__top-action-btn"
              data-bf-component="nav-panel"
              data-bf-part="topAction"
              data-bf-action="new-session"
              onClick={handleCreateSession}
              aria-label={createSessionLabel}
              data-testid="nav-new-session-btn"
            >
              <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                <Plus size={15} />
              </span>
              <span>{createSessionLabel}</span>
            </button>
          </Tooltip>

          <Tooltip content={assistantTooltip} placement="right" followCursor>
            <button
              type="button"
              className={`bitfun-nav-panel__top-action-btn${expandedSections.has('assistant-sessions') ? ' is-open' : ''}`}
              data-bf-component="nav-panel"
              data-bf-part="topAction"
              data-bf-action="smart-members"
              data-bf-state={expandedSections.has('assistant-sessions') ? 'open' : ''}
              onClick={() => toggleSection('assistant-sessions')}
              aria-expanded={expandedSections.has('assistant-sessions')}
              aria-label={assistantTooltip}
              data-testid="nav-smart-members-btn"
            >
              <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                <User size={15} />
              </span>
              <span>{t('nav.items.persona')}</span>
              <span className="bitfun-nav-panel__count-badge">
                {orderedAssistantWorkspacesList.length}
              </span>
            </button>
          </Tooltip>

        <div
          className={`bitfun-nav-panel__collapsible bitfun-nav-panel__smart-members-content${expandedSections.has('assistant-sessions') ? '' : ' is-collapsed'}`}
          data-bf-component="nav-panel"
          data-bf-part="sectionContent"
          data-bf-section="smart-members"
          data-bf-state={expandedSections.has('assistant-sessions') ? 'open' : ''}
        >
          <div className="bitfun-nav-panel__collapsible-inner">
            <div className="bitfun-nav-panel__smart-members-toolbar">
              <span>{t('nav.sections.assistantSessions')}</span>
              <AssistantSessionCreateMenu
                assistants={orderedAssistantWorkspacesList}
                primaryAssistant={primaryAssistantWorkspace}
                onCreatePrimary={handleCreatePrimaryAssistantSession}
                onCreateAssistant={handleCreateAssistantSession}
              />
            </div>
            <div className="bitfun-nav-panel__items bitfun-nav-panel__items--session-blocks bitfun-nav-panel__items--smart-members">
              {orderedAssistantWorkspacesList.map(workspace => {
                const assistantDisplayName =
                  workspace.workspaceKind === WorkspaceKind.Assistant
                    ? workspace.identity?.name?.trim() || workspace.name
                    : workspace.name;
                return (
                  <SessionsSection
                    key={workspace.id}
                    workspaceId={workspace.id}
                    workspacePath={workspace.rootPath}
                    remoteConnectionId={isRemoteWorkspace(workspace) ? workspace.connectionId : null}
                    isActiveWorkspace={workspace.id === currentWorkspace?.id}
                    presentation={{
                      kind: 'assistant',
                      assistant: {
                        id: workspace.assistantId || workspace.id,
                        name: assistantDisplayName,
                        avatar: workspace.identity?.avatar,
                        emoji: workspace.identity?.emoji,
                      },
                    }}
                    isVisible={expandedSections.has('assistant-sessions')}
                  />
                );
              })}
            </div>
          </div>
        </div>

          <Tooltip content={longTermTrackingTooltip} placement="right" followCursor>
            <button
              type="button"
              className="bitfun-nav-panel__top-action-btn"
              data-bf-component="nav-panel"
              data-bf-part="topAction"
              data-bf-action="long-term-tracking"
              onClick={handleOpenLongTermTracking}
              aria-label={longTermTrackingTooltip}
              data-testid="nav-long-term-tracking-btn"
            >
              <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                <Goal size={15} />
              </span>
              <span>{t('nav.items.longTermTracking')}</span>
            </button>
          </Tooltip>
        </div>

        {/* Workspace */}
        <div className="bitfun-nav-panel__section" data-bf-component="nav-panel" data-bf-part="section" data-bf-section="workspace">
          <SectionHeader
            label={t('shared:features.workspace')}
            collapsible
            isOpen={expandedSections.has('workspace')}
            onToggle={() => toggleSection('workspace')}
            actions={
              <>
                <WorkspaceSessionFilterMenu />
                <div className="bitfun-nav-panel__workspace-action-wrap">
                  <Tooltip content={addWorkspaceTooltip} placement="right" followCursor disabled={workspaceMenuOpen}>
                    <button
                      ref={workspaceMenuButtonRef}
                      type="button"
                      className={`bitfun-nav-panel__section-action${workspaceMenuOpen ? ' is-active' : ''}`}
                      aria-label={addWorkspaceTooltip}
                      aria-expanded={workspaceMenuOpen}
                      onClick={toggleWorkspaceMenu}
                      data-testid="nav-workspace-add-btn"
                    >
                      <Plus size={13} />
                    </button>
                  </Tooltip>
                </div>
              </>
            }
          />
          <div className={`bitfun-nav-panel__collapsible${expandedSections.has('workspace') ? '' : ' is-collapsed'}`} data-bf-component="nav-panel" data-bf-part="sectionContent" data-bf-state={expandedSections.has('workspace') ? 'open' : ''}>
            <div className="bitfun-nav-panel__collapsible-inner">
              <div className="bitfun-nav-panel__items">
                <WorkspaceListSection variant="projects" />
              </div>
            </div>
          </div>
        </div>

      </div>

      {/* ── Bottom: Extensions and MiniApp ────────── */}
      <div data-bf-component="nav-panel" data-bf-part="bottomBar" className="bitfun-nav-panel__bottom-bar" data-testid="nav-bottom-bar">
        {/* Temporarily hide Pages entry
        <button
          type="button"
          className={`bitfun-nav-panel__pages-entry${activeTabId === 'pages' ? ' is-active' : ''}`}
          onClick={() => openScene('pages')}
          aria-label={tPages('navLabel')}
          data-testid="nav-pages-entry"
        >
          <PanelsTopLeft size={15} aria-hidden="true" />
          <span>{tPages('navLabel')}</span>
        </button>
        */}
        <div className="bitfun-nav-panel__bottom-extension" data-bf-component="nav-panel" data-bf-part="extensionGroup" data-bf-state={isExtensionsOpen ? 'open' : ''} data-testid="agent-skill-panel">
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
                <Blocks size={15} className="bitfun-nav-panel__top-action-expand-icon-default" />
                <ChevronDown
                  size={15}
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
                  <Puzzle size={15} />
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
              </button>
            </Tooltip>
          </div>
        </div>
        <div className="bitfun-nav-panel__miniapp-footer" data-bf-component="nav-panel" data-bf-part="miniAppFooter">
          <MiniAppEntry
            isActive={activeTabId === 'miniapps' || !!activeMiniAppId}
            activeMiniAppId={activeMiniAppId}
            onOpenMiniApps={() => openScene('miniapps')}
            onOpenMiniApp={(appId) => openScene(`miniapp:${appId}`)}
          />
        </div>
      </div>

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
