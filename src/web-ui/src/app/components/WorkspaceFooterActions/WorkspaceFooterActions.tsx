import React, { useState, useCallback, useMemo } from 'react';
import {
  SquareTerminal,
  ChevronUp,
  ChevronRight,
  Orbit,
  RotateCcw,
  User,
  AppWindow,
  ChevronDown,
  Puzzle,
  Settings,
  Code2,
  Wrench,
  Bot,
} from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useOverlayManager } from '../../hooks/useOverlayManager';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useOverlayStore } from '../../stores/overlayStore';
import { useMyAgentStore } from '../../scenes/my-agent/myAgentStore';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { openDispatcherSession } from '@/flow_chat/services/openDispatcherSession';
import { WorkspaceKind } from '@/shared/types';
import { createLogger } from '@/shared/utils/logger';
import { useApp } from '../../hooks/useApp';
import './WorkspaceFooterActions.scss';

const log = createLogger('WorkspaceFooterActions');

const GREETING_KEYS = ['greetingMorning', 'greetingAfternoon', 'greetingEvening', 'greetingNight'] as const;

const WorkspaceFooterActions: React.FC = () => {
  const { t } = useI18n('common');
  const { openOverlay, toggleOverlay } = useOverlayManager();
  const { switchLeftPanelTab } = useApp();

  const activeOverlay = useOverlayStore(s => s.activeOverlay);
  const setSelectedAssistantWorkspaceId = useMyAgentStore(state => state.setSelectedAssistantWorkspaceId);

  const {
    currentWorkspace,
    assistantWorkspacesList,
    setActiveWorkspace,
  } = useWorkspaceContext();

  const defaultAssistantWorkspace = useMemo(
    () => assistantWorkspacesList.find(workspace => !workspace.assistantId) ?? assistantWorkspacesList[0] ?? null,
    [assistantWorkspacesList]
  );

  const isAssistantWorkspaceActive = currentWorkspace?.workspaceKind === WorkspaceKind.Assistant;

  const greeting = useMemo(() => {
    const hour = new Date().getHours();
    const key = hour >= 5 && hour < 12
      ? GREETING_KEYS[0]
      : hour >= 12 && hour < 18
        ? GREETING_KEYS[1]
        : hour >= 18 && hour < 22
          ? GREETING_KEYS[2]
          : GREETING_KEYS[3];
    return t(`welcome.${key}`);
  }, [t]);

  const [menuOpen, setMenuOpen] = useState(false);
  const [menuClosing, setMenuClosing] = useState(false);
  const [isDevKitSubmenuOpen, setIsDevKitSubmenuOpen] = useState(false);

  const closeMenu = useCallback(() => {
    setMenuClosing(true);
    setIsDevKitSubmenuOpen(false);
    setTimeout(() => {
      setMenuOpen(false);
      setMenuClosing(false);
    }, 150);
  }, []);

  const toggleMenu = useCallback(() => {
    if (menuOpen) {
      closeMenu();
      return;
    }
    setMenuOpen(true);
  }, [closeMenu, menuOpen]);

  const handleOpenShell = useCallback(() => {
    closeMenu();
    toggleOverlay('shell');
  }, [closeMenu, toggleOverlay]);

  const handleOpenDispatcher = useCallback(async () => {
    closeMenu();
    try {
      await openDispatcherSession({
        assistantWorkspace: defaultAssistantWorkspace
          ? { rootPath: defaultAssistantWorkspace.rootPath, id: defaultAssistantWorkspace.id }
          : null,
      });
    } catch (error) {
      log.error('Failed to open Dispatcher', error);
    }
  }, [closeMenu, defaultAssistantWorkspace]);

  const handleCreateDispatcherSession = useCallback(async (event: React.MouseEvent) => {
    event.stopPropagation();
    try {
      if (defaultAssistantWorkspace) {
        await flowChatManager.createChatSession(
          { workspacePath: defaultAssistantWorkspace.rootPath, workspaceId: defaultAssistantWorkspace.id },
          'Dispatcher'
        );
      } else {
        await flowChatManager.createChatSession({}, 'Dispatcher');
      }
    } catch (error) {
      log.error('Failed to create new Dispatcher session', error);
    }
  }, [defaultAssistantWorkspace]);

  const handleOpenAssistant = useCallback(() => {
    closeMenu();
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
    openOverlay('assistant');
  }, [
    closeMenu,
    currentWorkspace,
    defaultAssistantWorkspace,
    isAssistantWorkspaceActive,
    openOverlay,
    setActiveWorkspace,
    setSelectedAssistantWorkspaceId,
    switchLeftPanelTab,
  ]);

  const handleOpenApps = useCallback(() => {
    closeMenu();
    openOverlay('apps');
  }, [closeMenu, openOverlay]);

  const handleOpenSkills = useCallback(() => {
    closeMenu();
    openOverlay('skills');
  }, [closeMenu, openOverlay]);

  const handleOpenTools = useCallback(() => {
    closeMenu();
    openOverlay('tools');
  }, [closeMenu, openOverlay]);

  const handleOpenSubagents = useCallback(() => {
    closeMenu();
    openOverlay('subagents');
  }, [closeMenu, openOverlay]);

  const handleOpenSettings = useCallback(() => {
    closeMenu();
    openOverlay('settings');
  }, [closeMenu, openOverlay]);

  const isAssistantActive = activeOverlay === 'assistant';
  const isAppsActive = activeOverlay === 'apps'
    || (typeof activeOverlay === 'string' && activeOverlay.startsWith('live-app:'));
  const isSkillsActive = activeOverlay === 'skills';
  const isToolsActive = activeOverlay === 'tools';
  const isSubagentsActive = activeOverlay === 'subagents';
  const isSettingsActive = activeOverlay === 'settings';
  const isShellActive = activeOverlay === 'shell';

  return (
    <>
      <div className="bitfun-nav-panel__footer">
        <div className="bitfun-nav-panel__footer-left">
          <div className="bitfun-nav-panel__footer-more-wrap">
            <Tooltip content={t('nav.moreOptions')} placement="right" followCursor disabled={menuOpen}>
              <button
                type="button"
                className={`bitfun-nav-panel__footer-btn bitfun-nav-panel__footer-btn--icon${menuOpen ? ' is-active' : ''}`}
                aria-label={t('nav.moreOptions')}
                aria-expanded={menuOpen}
                onClick={toggleMenu}
              >
                {menuOpen ? (
                  <ChevronUp size={15} aria-hidden="true" />
                ) : (
                  <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
                    <Orbit size={14} className="bitfun-nav-panel__footer-btn-icon-swap-default" />
                    <ChevronUp size={15} className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
                  </span>
                )}
              </button>
            </Tooltip>

            {menuOpen && (
              <>
                <div
                  className="bitfun-nav-panel__footer-backdrop"
                  onClick={closeMenu}
                />
                <div
                  className={`bitfun-nav-panel__footer-menu${menuClosing ? ' is-closing' : ''}`}
                  role="menu"
                >
                  <div className="bitfun-nav-panel__footer-menu-col-actions">
                    <button
                      type="button"
                      className={`bitfun-nav-panel__footer-menu-item${isAssistantActive ? ' is-active' : ''}`}
                      role="menuitem"
                      onClick={handleOpenAssistant}
                    >
                      <User size={14} />
                      <span>{t('nav.items.persona')}</span>
                    </button>

                    <button
                      type="button"
                      className={`bitfun-nav-panel__footer-menu-item${isAppsActive ? ' is-active' : ''}`}
                      role="menuitem"
                      onClick={handleOpenApps}
                    >
                      <AppWindow size={14} />
                      <span>{t('nav.sections.agentApp')}</span>
                    </button>

                    <button
                      type="button"
                      className={`bitfun-nav-panel__footer-menu-item bitfun-nav-panel__footer-menu-item--expandable${isDevKitSubmenuOpen ? ' is-open' : ''}`}
                      role="menuitem"
                      aria-expanded={isDevKitSubmenuOpen}
                      onClick={() => setIsDevKitSubmenuOpen(value => !value)}
                    >
                      <Code2 size={14} />
                      <span>{t('nav.sections.devKit')}</span>
                      <ChevronDown
                        size={13}
                        className={`bitfun-nav-panel__footer-menu-chevron${isDevKitSubmenuOpen ? ' is-open' : ''}`}
                        aria-hidden="true"
                      />
                    </button>

                    <div className={`bitfun-nav-panel__footer-menu-sublist${isDevKitSubmenuOpen ? ' is-open' : ''}`}>
                      <div>
                        <button
                          type="button"
                          className={`bitfun-nav-panel__footer-menu-item bitfun-nav-panel__footer-menu-item--sub${isSkillsActive ? ' is-active' : ''}`}
                          role="menuitem"
                          onClick={handleOpenSkills}
                        >
                          <Puzzle size={13} />
                          <span>{t('nav.items.skills')}</span>
                        </button>

                        <button
                          type="button"
                          className={`bitfun-nav-panel__footer-menu-item bitfun-nav-panel__footer-menu-item--sub${isToolsActive ? ' is-active' : ''}`}
                          role="menuitem"
                          onClick={handleOpenTools}
                        >
                          <Wrench size={13} />
                          <span>{t('nav.items.tools')}</span>
                        </button>

                        <button
                          type="button"
                          className={`bitfun-nav-panel__footer-menu-item bitfun-nav-panel__footer-menu-item--sub${isSubagentsActive ? ' is-active' : ''}`}
                          role="menuitem"
                          onClick={handleOpenSubagents}
                        >
                          <Bot size={13} />
                          <span>{t('nav.items.subAgent')}</span>
                        </button>
                      </div>
                    </div>

                    <div className="bitfun-nav-panel__footer-menu-divider" />

                    <button
                      type="button"
                      className={`bitfun-nav-panel__footer-menu-item${isShellActive ? ' is-active' : ''}`}
                      role="menuitem"
                      aria-pressed={isShellActive}
                      onClick={handleOpenShell}
                    >
                      <SquareTerminal size={14} />
                      <span>{t('scenes.shell')}</span>
                    </button>

                    <div className="bitfun-nav-panel__footer-menu-divider" />

                    <button
                      type="button"
                      className={`bitfun-nav-panel__footer-menu-item${isSettingsActive ? ' is-active' : ''}`}
                      role="menuitem"
                      aria-pressed={isSettingsActive}
                      onClick={handleOpenSettings}
                    >
                      <Settings size={14} />
                      <span>{t('tabs.settings')}</span>
                    </button>

                    <div className="bitfun-nav-panel__footer-menu-row bitfun-nav-panel__footer-menu-row--bottom">
                      <button
                        type="button"
                        className="bitfun-nav-panel__footer-menu-item bitfun-nav-panel__footer-menu-item--row-main"
                        role="menuitem"
                        onClick={handleOpenDispatcher}
                      >
                        <Orbit size={14} />
                        <span>{t('nav.sessions.dispatcherShort')}</span>
                      </button>
                      <Tooltip content={t('nav.tooltips.newDispatcherSession')} placement="right">
                        <button
                          type="button"
                          className="bitfun-nav-panel__footer-menu-item-inline-btn"
                          onClick={handleCreateDispatcherSession}
                          aria-label={t('nav.tooltips.newDispatcherSession')}
                        >
                          <RotateCcw size={12} />
                        </button>
                      </Tooltip>
                    </div>
                  </div>

                  <div className="bitfun-nav-panel__footer-menu-col-sep" aria-hidden="true" />

                  <div className="bitfun-nav-panel__footer-menu-greeting">
                    <p className="bitfun-nav-panel__footer-menu-greeting-title">{greeting}</p>
                    <p className="bitfun-nav-panel__footer-menu-greeting-sub">{t('nav.menuPanel.subtitle')}</p>

                    <div className="bitfun-nav-panel__footer-menu-greeting-actions">
                      <button
                        type="button"
                        className="bitfun-nav-panel__footer-menu-greeting-action"
                        onClick={handleOpenDispatcher}
                      >
                        <span className="bitfun-nav-panel__footer-menu-greeting-action-icon">
                          <Orbit size={15} />
                        </span>
                        <span className="bitfun-nav-panel__footer-menu-greeting-action-body">
                          <span className="bitfun-nav-panel__footer-menu-greeting-action-title">
                            {t('nav.sessions.dispatcherShort')}
                          </span>
                          <span className="bitfun-nav-panel__footer-menu-greeting-action-desc">
                            {t('nav.menuPanel.agenticOSDesc')}
                          </span>
                        </span>
                        <ChevronRight size={12} className="bitfun-nav-panel__footer-menu-greeting-action-arrow" aria-hidden="true" />
                      </button>

                      <button
                        type="button"
                        className="bitfun-nav-panel__footer-menu-greeting-action"
                        onClick={handleOpenAssistant}
                      >
                        <span className="bitfun-nav-panel__footer-menu-greeting-action-icon">
                          <User size={15} />
                        </span>
                        <span className="bitfun-nav-panel__footer-menu-greeting-action-body">
                          <span className="bitfun-nav-panel__footer-menu-greeting-action-title">
                            {t('nav.items.persona')}
                          </span>
                          <span className="bitfun-nav-panel__footer-menu-greeting-action-desc">
                            {t('nav.menuPanel.assistantDesc')}
                          </span>
                        </span>
                        <ChevronRight size={12} className="bitfun-nav-panel__footer-menu-greeting-action-arrow" aria-hidden="true" />
                      </button>
                    </div>
                  </div>
                </div>
              </>
            )}
          </div>
        </div>
      </div>
    </>
  );
};

export default WorkspaceFooterActions;
