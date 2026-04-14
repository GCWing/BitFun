/**
 * MainNav — default workspace navigation sidebar.
 *
 * Layout (top to bottom):
 *   1. Workspace file search
 *   2. Top: Dispatcher | Assistant | Agent App (expand → Agents | Mini App) | Skills
 *   3. Flat session list (all opened workspaces)
 *
 * When a scene-nav transition is active (`isDeparting=true`), items receive
 * positional CSS classes for the split-open animation effect.
 */

import React, { useCallback, useState, useMemo, useEffect } from 'react';
import { User, Users, Puzzle, AppWindow, ChevronDown, Search, Orbit, MonitorPlay, RotateCcw } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useApp } from '../../hooks/useApp';
import { useSceneManager } from '../../hooks/useSceneManager';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import type { SceneTabId } from '../SceneBar/types';
import SectionHeader from './components/SectionHeader';
import MiniAppEntry from './components/MiniAppEntry';
import SessionsSection from './sections/sessions/SessionsSection';
import { useSceneStore } from '../../stores/sceneStore';
import { useMyAgentStore } from '../../scenes/my-agent/myAgentStore';
import { useMiniAppCatalogSync } from '../../scenes/miniapps/hooks/useMiniAppCatalogSync';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { openMainSession } from '@/flow_chat/services/openBtwSession';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { WorkspaceKind } from '@/shared/types';
import { useSSHRemoteContext, RemoteFileBrowser } from '@/features/ssh-remote';
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
  useMiniAppCatalogSync();

  const sshRemote = useSSHRemoteContext();

  const { switchLeftPanelTab } = useApp();
  const { openScene } = useSceneManager();
  const activeTabId = useSceneStore(s => s.activeTabId);
  const setSelectedAssistantWorkspaceId = useMyAgentStore((s) => s.setSelectedAssistantWorkspaceId);
  const { t } = useI18n('common');
  const {
    currentWorkspace,
    openedWorkspacesList,
    assistantWorkspacesList,
    setActiveWorkspace,
  } = useWorkspaceContext();

  const activeMiniAppId = useMemo(
    () => (typeof activeTabId === 'string' && activeTabId.startsWith('miniapp:') ? activeTabId.slice('miniapp:'.length) : null),
    [activeTabId]
  );

  // Section expand state
  const [expandedSections, setExpandedSections] = useState<Set<string>>(
    () => new Set(['sessions'])
  );

  const [isAgentAppOpen, setIsAgentAppOpen] = useState(false);
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

  const isAssistantWorkspaceActive = currentWorkspace?.workspaceKind === WorkspaceKind.Assistant;

  // Global workspace = default assistant workspace (no assistantId), fallback to first assistant ws
  const defaultAssistantWorkspace = useMemo(
    () => assistantWorkspacesList.find(w => !w.assistantId) ?? assistantWorkspacesList[0] ?? null,
    [assistantWorkspacesList]
  );

  const handleOpenDispatcher = useCallback(async () => {
    try {
      // Find an existing Dispatcher session across all loaded sessions (mode field, not config.agentType)
      const storeState = flowChatStore.getState();
      const existing = Array.from(storeState.sessions.values())
        .filter((s) => s.mode === 'Dispatcher')
        .sort((a, b) => (b.lastActiveAt ?? b.createdAt ?? 0) - (a.lastActiveAt ?? a.createdAt ?? 0))[0] ?? null;
      if (existing) {
        // Switch directly — no workspace change needed, Dispatcher is nav-level
        await openMainSession(existing.sessionId);
        return;
      }
      // No existing Dispatcher: create in the assistant workspace for persistence
      // The workspace is just storage — no need to activate it
      const globalWs = defaultAssistantWorkspace;
      if (globalWs) {
        await flowChatManager.createChatSession(
          { workspacePath: globalWs.rootPath, workspaceId: globalWs.id },
          'Dispatcher'
        );
      } else {
        await flowChatManager.createChatSession({}, 'Dispatcher');
      }
    } catch (err) {
      log.error('Failed to open Dispatcher', err);
    }
  }, [defaultAssistantWorkspace]);

  const handleNewDispatcherSession = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const globalWs = defaultAssistantWorkspace;
      if (globalWs) {
        await flowChatManager.createChatSession(
          { workspacePath: globalWs.rootPath, workspaceId: globalWs.id },
          'Dispatcher'
        );
      } else {
        await flowChatManager.createChatSession({}, 'Dispatcher');
      }
    } catch (err) {
      log.error('Failed to create new Dispatcher session', err);
    }
  }, [defaultAssistantWorkspace]);

  useEffect(() => {
    // Initialize sessions from disk for all opened workspaces, including assistant workspaces
    const allWorkspaces = [...openedWorkspacesList, ...assistantWorkspacesList];
    allWorkspaces.forEach(workspace => {
      if (workspace.workspaceKind === WorkspaceKind.Remote) {
        void flowChatStore.initializeFromDisk(
          workspace.rootPath,
          workspace.connectionId ?? undefined,
          workspace.sshHost ?? undefined
        );
      } else {
        void flowChatStore.initializeFromDisk(workspace.rootPath);
      }
    });
  }, [openedWorkspacesList, assistantWorkspacesList]);

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

  const handleSelectRemoteWorkspace = useCallback(async (path: string) => {
    try {
      await sshRemote.openWorkspace(path);
      sshRemote.setShowFileBrowser(false);
    } catch (err) {
      log.error('Failed to open remote workspace', err);
    }
  }, [sshRemote]);

  const handleOpenAssistant = useCallback(() => {
    const targetAssistantWorkspace =
      isAssistantWorkspaceActive && currentWorkspace?.workspaceKind === WorkspaceKind.Assistant
        ? currentWorkspace
        : defaultAssistantWorkspace;

    if (targetAssistantWorkspace?.id) {
      setSelectedAssistantWorkspaceId(targetAssistantWorkspace.id);
    }
    if (!isAssistantWorkspaceActive && targetAssistantWorkspace) {
      void setActiveWorkspace(targetAssistantWorkspace.id).catch(error => {
        log.warn('Failed to activate default assistant workspace', { error });
      });
    }
    switchLeftPanelTab('profile');
    openScene('assistant');
  }, [
    currentWorkspace,
    defaultAssistantWorkspace,
    isAssistantWorkspaceActive,
    openScene,
    setActiveWorkspace,
    setSelectedAssistantWorkspaceId,
    switchLeftPanelTab,
  ]);

  const handleOpenAgents = useCallback(() => {
    openScene('agents');
  }, [openScene]);

  const handleOpenSkills = useCallback(() => {
    openScene('skills');
  }, [openScene]);

  const isAgentsActive = activeTabId === 'agents';
  const isSkillsActive = activeTabId === 'skills';

  const isMiniAppsSceneActive = activeTabId === 'miniapps' || !!activeMiniAppId;

  useEffect(() => {
    if (isAgentsActive || isMiniAppsSceneActive) {
      setIsAgentAppOpen(true);
    }
  }, [isAgentsActive, isMiniAppsSceneActive]);

  const isDispatcherActive = useMemo(() => {
    const storeState = flowChatStore.getState();
    const activeId = storeState.activeSessionId;
    if (!activeId) return false;
    const active = storeState.sessions.get(activeId);
    return active?.mode === 'Dispatcher';
  }, [activeTabId]);

  const dispatcherTooltip = t('nav.sessions.dispatcher');
  const assistantTooltip = t('nav.items.persona');
  const isAssistantActive = activeTabId === 'assistant';
  const agentsTooltip = t('nav.tooltips.thinkAApp');
  const skillsTooltip = t('nav.tooltips.skills');
  const agentAppLabel = t('nav.sections.agentApp');
  const aappTooltip = t('nav.tooltips.aapp');
  const runAAppTooltip = t('nav.tooltips.runAApp');
  const driveAAppTooltip = t('nav.tooltips.driveAApp');
  return (
    <>
      {/* ── Workspace search ───────────────────────── */}
      <div className="bitfun-nav-panel__brand-header">
        <div className="bitfun-nav-panel__brand-search">
          <Tooltip content={t('nav.search.triggerTooltip')} placement="right" followCursor>
            <button
              type="button"
              className="bitfun-nav-panel__search-trigger"
              onClick={() => setSearchOpen(true)}
              aria-label={t('nav.search.triggerTooltip')}
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
      </div>

      {/* ── Top action strip ────────────────────────── */}
      <div className="bitfun-nav-panel__top-actions">
        <div className={`bitfun-nav-panel__top-action-row${isDispatcherActive ? ' is-active' : ''}`}>
          <Tooltip content={dispatcherTooltip} placement="right" followCursor>
            <button
              type="button"
              className="bitfun-nav-panel__top-action-btn bitfun-nav-panel__top-action-btn--in-row"
              onClick={handleOpenDispatcher}
              aria-label={dispatcherTooltip}
            >
              <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                <Orbit size={15} />
              </span>
              <span>{t('nav.sessions.dispatcherShort')}</span>
            </button>
          </Tooltip>
          <Tooltip content={t('nav.tooltips.newDispatcherSession')} placement="right" followCursor>
            <button
              type="button"
              className="bitfun-nav-panel__top-action-inline-btn"
              onClick={handleNewDispatcherSession}
              aria-label={t('nav.tooltips.newDispatcherSession')}
            >
              <RotateCcw size={13} />
            </button>
          </Tooltip>
        </div>

        <Tooltip content={assistantTooltip} placement="right" followCursor>
          <button
            type="button"
            className={`bitfun-nav-panel__top-action-btn${isAssistantActive ? ' is-active' : ''}`}
            onClick={handleOpenAssistant}
            aria-label={assistantTooltip}
          >
            <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
              <User size={15} />
            </span>
            <span>{t('nav.items.persona')}</span>
          </button>
        </Tooltip>

        <div className="bitfun-nav-panel__top-action-expand">
          <Tooltip content={aappTooltip} placement="right" followCursor>
            <button
              type="button"
              className={[
                'bitfun-nav-panel__top-action-btn',
                'bitfun-nav-panel__top-action-btn--expand',
                isAgentAppOpen ? 'is-open' : '',
              ].filter(Boolean).join(' ')}
              onClick={() => setIsAgentAppOpen(v => !v)}
              aria-expanded={isAgentAppOpen}
              aria-label={agentAppLabel}
            >
              <span className="bitfun-nav-panel__top-action-expand-icons" aria-hidden="true">
                <AppWindow size={15} className="bitfun-nav-panel__top-action-expand-icon-default" />
                <ChevronDown
                  size={15}
                  className={[
                    'bitfun-nav-panel__top-action-expand-icon-chevron',
                    isAgentAppOpen ? 'is-open' : '',
                  ].filter(Boolean).join(' ')}
                />
              </span>
              <span>{agentAppLabel}</span>
            </button>
          </Tooltip>

          <div className={`bitfun-nav-panel__top-action-sublist bitfun-nav-panel__top-action-sublist--agent-app${isAgentAppOpen ? ' is-open' : ''}`}>
            <Tooltip content={agentsTooltip} placement="right" followCursor>
              <button
                type="button"
                className={[
                  'bitfun-nav-panel__top-action-btn',
                  'bitfun-nav-panel__top-action-btn--sub',
                  isAgentsActive ? 'is-active' : '',
                ].filter(Boolean).join(' ')}
                onClick={handleOpenAgents}
                aria-label={agentsTooltip}
              >
                <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                  <Users size={15} />
                </span>
                <span>{t('nav.items.agents')}</span>
              </button>
            </Tooltip>

            <div className="bitfun-nav-panel__top-action-miniapp-slot">
              <Tooltip content={runAAppTooltip} placement="right" followCursor>
                <MiniAppEntry
                  isActive={activeTabId === 'miniapps' || !!activeMiniAppId}
                  activeMiniAppId={activeMiniAppId}
                  onOpenMiniApps={() => openScene('miniapps')}
                  onOpenMiniApp={(appId) => openScene(`miniapp:${appId}`)}
                />
              </Tooltip>
            </div>

            <Tooltip content={driveAAppTooltip} placement="right" followCursor>
              <button
                type="button"
                className="bitfun-nav-panel__top-action-btn bitfun-nav-panel__top-action-btn--sub bitfun-nav-panel__top-action-btn--coming-soon"
                disabled
                aria-label={driveAAppTooltip}
              >
                <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
                  <MonitorPlay size={15} />
                </span>
                <span>{t('nav.items.driveAApp')}</span>
                <span className="bitfun-nav-panel__top-action-badge">{t('nav.badges.comingSoon')}</span>
              </button>
            </Tooltip>
          </div>
        </div>

        <Tooltip content={skillsTooltip} placement="right" followCursor>
          <button
            type="button"
            className={`bitfun-nav-panel__top-action-btn${isSkillsActive ? ' is-active' : ''}`}
            onClick={handleOpenSkills}
            aria-label={skillsTooltip}
          >
            <span className="bitfun-nav-panel__top-action-icon-slot" aria-hidden="true">
              <Puzzle size={15} />
            </span>
            <span>{t('nav.items.skills')}</span>
          </button>
        </Tooltip>
      </div>

      {/* ── Sections ────────────────────────────────── */}
      <div className="bitfun-nav-panel__sections">

        <div className="bitfun-nav-panel__section">
          <SectionHeader
            label={t('nav.sections.sessions')}
            collapsible
            isOpen={expandedSections.has('sessions')}
            onToggle={() => toggleSection('sessions')}
          />
          <div className={`bitfun-nav-panel__collapsible${expandedSections.has('sessions') ? '' : ' is-collapsed'}`}>
            <div className="bitfun-nav-panel__collapsible-inner">
              <div className="bitfun-nav-panel__items">
                <SessionsSection listAllSessions />
              </div>
            </div>
          </div>
        </div>

      </div>

      {sshRemote.showFileBrowser && sshRemote.connectionId && (
        <RemoteFileBrowser
          connectionId={sshRemote.connectionId}
          initialPath={sshRemote.remoteFileBrowserInitialPath}
          homePath={sshRemote.remoteFileBrowserInitialPath}
          onSelect={handleSelectRemoteWorkspace}
          onCancel={() => {
            sshRemote.setShowFileBrowser(false);
            void sshRemote.disconnect();
          }}
        />
      )}
    </>
  );
};

export default MainNav;
