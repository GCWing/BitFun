/**
 * MainNav — default workspace navigation sidebar.
 *
 * Renders nav sections. When a scene-nav transition
 * is active (`isDeparting=true`), every item/section receives a positional
 * CSS class relative to the anchor item (`anchorNavSceneId`):
 *   - items above the anchor  → `.is-departing-up`   (slide up + fade)
 *   - the anchor item itself  → `.is-departing-anchor` (brief highlight)
 *   - items below the anchor  → `.is-departing-down`  (slide down + fade)
 * This creates the visual "split-open from the clicked item" effect while
 * the outer Grid accordion handles the actual height collapse.
 */

import React, { useCallback, useState, useMemo, useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import { Plus, FolderOpen, FolderPlus, History, Check, Bot, Clock3 } from 'lucide-react';
import { Badge, Tooltip } from '@/component-library';
import { useApp } from '../../hooks/useApp';
import { useSceneManager } from '../../hooks/useSceneManager';
import { useNavSceneStore } from '../../stores/navSceneStore';
import { useSessionModeStore } from '../../stores/sessionModeStore';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { NAV_SECTIONS } from './config';
import type { PanelType } from '../../types';
import type { NavItem as NavItemConfig } from './types';
import type { SceneTabId } from '../SceneBar/types';
import NavItem from './components/NavItem';
import SectionHeader from './components/SectionHeader';
import MiniAppEntry from './components/MiniAppEntry';
import WorkspaceListSection from './sections/workspaces/WorkspaceListSection';
import ScheduledJobsDialog from '../ScheduledJobsDialog/ScheduledJobsDialog';
import { useSceneStore } from '../../stores/sceneStore';
import { useMyAgentStore } from '../../scenes/my-agent/myAgentStore';
import { useMiniAppCatalogSync } from '../../scenes/miniapps/hooks/useMiniAppCatalogSync';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { compareSessionsForDisplay } from '@/flow_chat/utils/sessionOrdering';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { WorkspaceKind } from '@/shared/types';

const DEFAULT_MODE_CONFIG_KEY = 'app.session_config.default_mode';
const NAV_DISPLAY_MODE_STORAGE_KEY = 'bitfun.nav.displayMode';
import './NavPanel.scss';

const log = createLogger('MainNav');

type DepartDir = 'up' | 'anchor' | 'down' | null;
type NavDisplayMode = 'pro' | 'assistant';

/**
 * Build a flat ordered list of (sectionId, itemTab) tuples so we can
 * determine each element's position relative to the anchor item.
 */
function buildFlatItemOrder(): { sectionId: string; tab: PanelType; navSceneId?: SceneTabId }[] {
  const list: { sectionId: string; tab: PanelType; navSceneId?: SceneTabId }[] = [];
  for (const section of NAV_SECTIONS) {
    for (const item of section.items) {
      list.push({ sectionId: section.id, tab: item.tab, navSceneId: item.navSceneId });
    }
  }
  return list;
}

const FLAT_ITEMS = buildFlatItemOrder();

function getAnchorIndex(anchorId: SceneTabId | null): number {
  if (!anchorId) return -1;
  return FLAT_ITEMS.findIndex(i => i.navSceneId === anchorId);
}

function getInitialNavDisplayMode(): NavDisplayMode {
  if (typeof window === 'undefined') return 'pro';
  return window.localStorage.getItem(NAV_DISPLAY_MODE_STORAGE_KEY) === 'assistant'
    ? 'assistant'
    : 'pro';
}

interface MainNavProps {
  isDeparting?: boolean;
  anchorNavSceneId?: SceneTabId | null;
}

const MainNav: React.FC<MainNavProps> = ({
  isDeparting = false,
  anchorNavSceneId = null,
}) => {
  useMiniAppCatalogSync();
  const { state, switchLeftPanelTab } = useApp();
  const { openScene } = useSceneManager();
  const openNavScene = useNavSceneStore(s => s.openNavScene);
  const activeTabId = useSceneStore(s => s.activeTabId);
  const setMyAgentView = useMyAgentStore(s => s.setActiveView);
  const selectedAssistantWorkspaceId = useMyAgentStore((s) => s.selectedAssistantWorkspaceId);
  const setSelectedAssistantWorkspaceId = useMyAgentStore((s) => s.setSelectedAssistantWorkspaceId);
  const { t } = useI18n('common');
  const {
    currentWorkspace,
    recentWorkspaces,
    openedWorkspacesList,
    assistantWorkspacesList,
    switchWorkspace,
    setActiveWorkspace,
  } = useWorkspaceContext();

  const activeTab = state.layout.leftPanelActiveTab;
  const activeMiniAppId = useMemo(
    () => (typeof activeTabId === 'string' && activeTabId.startsWith('miniapp:') ? activeTabId.slice('miniapp:'.length) : null),
    [activeTabId]
  );

  const anchorIdx = useMemo(() => getAnchorIndex(anchorNavSceneId), [anchorNavSceneId]);

  const getDepartDir = useCallback(
    (flatIdx: number): DepartDir => {
      if (!isDeparting) return null;
      if (anchorIdx < 0) return 'up';
      if (flatIdx < anchorIdx) return 'up';
      if (flatIdx === anchorIdx) return 'anchor';
      return 'down';
    },
    [isDeparting, anchorIdx]
  );

  const getSectionDepartDir = useCallback(
    (sectionId: string): DepartDir => {
      if (!isDeparting) return null;
      if (anchorIdx < 0) return 'up';
      const first = FLAT_ITEMS.findIndex(i => i.sectionId === sectionId);
      const last  = FLAT_ITEMS.length - 1 - [...FLAT_ITEMS].reverse().findIndex(i => i.sectionId === sectionId);
      if (last < anchorIdx) return 'up';
      if (first > anchorIdx) return 'down';
      return null;
    },
    [isDeparting, anchorIdx]
  );

  const [expandedSections, setExpandedSections] = useState<Set<string>>(() => {
    const init = new Set<string>();
    NAV_SECTIONS.forEach(s => {
      if (s.defaultExpanded !== false) init.add(s.id);
    });
    return init;
  });
  const workspaceMenuButtonRef = useRef<HTMLButtonElement | null>(null);
  const workspaceMenuRef = useRef<HTMLDivElement | null>(null);
  const [workspaceMenuOpen, setWorkspaceMenuOpen] = useState(false);
  const [workspaceMenuClosing, setWorkspaceMenuClosing] = useState(false);
  const [workspaceMenuPos, setWorkspaceMenuPos] = useState({ top: 0, left: 0 });


  const getSectionLabel = useCallback(
    (sectionId: string, fallbackLabel: string | null) => {
      if (!fallbackLabel) return null;
      const keyMap: Record<string, string> = {
        assistants: 'nav.workspaces.groups.assistants',
        workspace: 'nav.sections.workspace',
        'my-agent': 'nav.sections.myAgent',
        miniapps: 'scenes.miniApps',
      };
      const key = keyMap[sectionId];
      return key ? t(key) || fallbackLabel : fallbackLabel;
    },
    [t]
  );

  const toggleSection = useCallback((id: string) => {
    setExpandedSections(prev => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
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

  const openWorkspaceMenu = useCallback(async () => {
    try {
      await workspaceManager.cleanupInvalidWorkspaces();
    } catch (error) {
      log.warn('Failed to cleanup invalid workspaces before opening workspace menu', { error });
    }

    const rect = workspaceMenuButtonRef.current?.getBoundingClientRect();
    if (!rect) return;
    setWorkspaceMenuPos({
      top: rect.bottom + 6,
      left: rect.left,
    });
    setWorkspaceMenuOpen(true);
    setWorkspaceMenuClosing(false);
  }, []);

  const toggleWorkspaceMenu = useCallback(() => {
    if (workspaceMenuOpen) {
      closeWorkspaceMenu();
      return;
    }
    void openWorkspaceMenu();
  }, [closeWorkspaceMenu, openWorkspaceMenu, workspaceMenuOpen]);

  const setSessionMode = useSessionModeStore(s => s.setMode);
  const isAssistantWorkspaceActive = currentWorkspace?.workspaceKind === WorkspaceKind.Assistant;
  const defaultAssistantWorkspace = useMemo(
    () => assistantWorkspacesList.find(workspace => !workspace.assistantId) ?? assistantWorkspacesList[0] ?? null,
    [assistantWorkspacesList]
  );
  const selectedAssistantWorkspace = useMemo(() => {
    if (!selectedAssistantWorkspaceId) {
      return null;
    }

    return assistantWorkspacesList.find(
      (workspace) => workspace.id === selectedAssistantWorkspaceId
    ) ?? null;
  }, [assistantWorkspacesList, selectedAssistantWorkspaceId]);
  const resolvedAssistantWorkspace = useMemo(() => {
    if (isAssistantWorkspaceActive && currentWorkspace?.workspaceKind === WorkspaceKind.Assistant) {
      return currentWorkspace;
    }

    if (selectedAssistantWorkspace) {
      return selectedAssistantWorkspace;
    }

    return defaultAssistantWorkspace;
  }, [
    currentWorkspace,
    defaultAssistantWorkspace,
    isAssistantWorkspaceActive,
    selectedAssistantWorkspace,
  ]);
  const resolvedAssistantDisplayName = useMemo(() => {
    if (!resolvedAssistantWorkspace) {
      return '';
    }

    return resolvedAssistantWorkspace.identity?.name?.trim() || resolvedAssistantWorkspace.name;
  }, [resolvedAssistantWorkspace]);
  const resolvedAssistantSessionId = useMemo(() => {
    const workspacePath = resolvedAssistantWorkspace?.rootPath;
    if (!workspacePath) {
      return undefined;
    }

    const workspaceSessions = Array.from(flowChatStore.getState().sessions.values())
      .filter(session =>
        (session.workspacePath || workspacePath) === workspacePath && !session.parentSessionId
      )
      .sort(compareSessionsForDisplay);

    return workspaceSessions[0]?.sessionId;
  }, [resolvedAssistantWorkspace]);

  const [defaultSessionMode, setDefaultSessionMode] = useState<'code' | 'cowork'>('code');
  const [navDisplayMode, setNavDisplayMode] = useState<NavDisplayMode>(getInitialNavDisplayMode);
  const [isModeSwitching, setIsModeSwitching] = useState(false);
  const [modeLogoSrc, setModeLogoSrc] = useState('/panda_1.png');
  const [modeLogoHoverSrc, setModeLogoHoverSrc] = useState('/panda_2.png');
  const [isScheduledJobsDialogOpen, setIsScheduledJobsDialogOpen] = useState(false);
  const modeSwitchTimerRef = useRef<number | null>(null);
  const modeSwitchSwapTimerRef = useRef<number | null>(null);

  useEffect(() => {
    configManager.getConfig<'code' | 'cowork'>(DEFAULT_MODE_CONFIG_KEY).then(mode => {
      if (mode === 'code' || mode === 'cowork') {
        setDefaultSessionMode(mode);
        setSessionMode(mode);
      }
    }).catch(() => {});

    const unwatch = configManager.watch(DEFAULT_MODE_CONFIG_KEY, () => {
      configManager.getConfig<'code' | 'cowork'>(DEFAULT_MODE_CONFIG_KEY).then(mode => {
        if (mode === 'code' || mode === 'cowork') {
          setDefaultSessionMode(mode);
          setSessionMode(mode);
        }
      }).catch(() => {});
    });
    return () => unwatch();
  }, [setSessionMode]);

  useEffect(() => () => {
    if (modeSwitchTimerRef.current !== null) {
      window.clearTimeout(modeSwitchTimerRef.current);
    }
    if (modeSwitchSwapTimerRef.current !== null) {
      window.clearTimeout(modeSwitchSwapTimerRef.current);
    }
  }, []);

  useEffect(() => {
    openedWorkspacesList.forEach(workspace => {
      void flowChatStore.initializeFromDisk(workspace.rootPath);
    });
  }, [openedWorkspacesList]);




  const handleCreateAssistantWorkspace = useCallback(async () => {
    try {
      await workspaceManager.createAssistantWorkspace();
    } catch (err) {
      log.error('Failed to create assistant workspace', err);
    }
  }, []);

  const handleItemClick = useCallback(
    (tab: PanelType, item: NavItemConfig) => {
      if (item.behavior === 'scene' && item.sceneId) {
        openScene(item.sceneId);
      } else {
        if (item.navSceneId) {
          openNavScene(item.navSceneId);
        }
        switchLeftPanelTab(tab);
      }
    },
    [switchLeftPanelTab, openScene, openNavScene]
  );

  const handleCreateSession = useCallback(async (mode?: 'agentic' | 'Cowork' | 'Claw') => {
    openScene('session');
    switchLeftPanelTab('sessions');
    try {
      await flowChatManager.createChatSession(
        {},
        mode ?? (
          isAssistantWorkspaceActive
          ? 'Claw'
          : defaultSessionMode === 'cowork'
            ? 'Cowork'
            : 'agentic'
        )
      );
    } catch (err) {
      log.error('Failed to create session', err);
    }
  }, [openScene, switchLeftPanelTab, defaultSessionMode, isAssistantWorkspaceActive]);

  const handleCreateCodeSession = useCallback(() => {
    setSessionMode('code');
    void handleCreateSession('agentic');
  }, [handleCreateSession, setSessionMode]);

  const handleCreateCoworkSession = useCallback(() => {
    setSessionMode('cowork');
    void handleCreateSession('Cowork');
  }, [handleCreateSession, setSessionMode]);

  const handleCreateAssistantSession = useCallback(async () => {
    const targetAssistantWorkspace =
      isAssistantWorkspaceActive && currentWorkspace?.workspaceKind === WorkspaceKind.Assistant
        ? currentWorkspace
        : defaultAssistantWorkspace;

    if (targetAssistantWorkspace && !isAssistantWorkspaceActive) {
      try {
        await setActiveWorkspace(targetAssistantWorkspace.id);
      } catch (error) {
        log.warn('Failed to activate assistant workspace before creating session', { error });
      }
    }

    await handleCreateSession('Claw');
  }, [
    currentWorkspace,
    defaultAssistantWorkspace,
    handleCreateSession,
    isAssistantWorkspaceActive,
    setActiveWorkspace,
  ]);

  const handleOpenProject = useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('header.selectProjectDirectory'),
      });
      if (selected && typeof selected === 'string') {
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
      if (event.key === 'Escape') {
        closeWorkspaceMenu();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [closeWorkspaceMenu, workspaceMenuOpen]);


  const handleOpenProfile = useCallback(() => {
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
    setMyAgentView('profile');
    switchLeftPanelTab('profile');
    openScene('my-agent');
  }, [
    currentWorkspace,
    defaultAssistantWorkspace,
    isAssistantWorkspaceActive,
    openScene,
    setActiveWorkspace,
    setMyAgentView,
    setSelectedAssistantWorkspaceId,
    switchLeftPanelTab,
  ]);

  const handleOpenScheduledJobsDialog = useCallback(() => {
    if (resolvedAssistantWorkspace?.id) {
      setSelectedAssistantWorkspaceId(resolvedAssistantWorkspace.id);
    }
    setIsScheduledJobsDialogOpen(true);
  }, [resolvedAssistantWorkspace, setSelectedAssistantWorkspaceId]);

  const handleOpenProModeSession = useCallback(async () => {
    // 找到项目工作区（非 assistant 类型）
    const projectWorkspaces = openedWorkspacesList.filter(
      w => w.workspaceKind !== WorkspaceKind.Assistant
    );

    const targetWorkspace =
      currentWorkspace?.workspaceKind !== WorkspaceKind.Assistant
        ? currentWorkspace
        : projectWorkspaces[0] ?? null;

    // 若当前激活的是 assistant workspace，先切回项目工作区
    if (targetWorkspace && currentWorkspace?.id !== targetWorkspace.id) {
      await setActiveWorkspace(targetWorkspace.id).catch(() => {});
    }

    const workspacePath = targetWorkspace?.rootPath;
    const state = flowChatStore.getState();

    if (workspacePath) {
      const workspaceSessions = Array.from(state.sessions.values())
        .filter(s =>
          (s.workspacePath || workspacePath) === workspacePath &&
          !s.parentSessionId
        )
        .sort(compareSessionsForDisplay);

      if (workspaceSessions.length > 0) {
        const firstSession = workspaceSessions[0];
        if (firstSession.isHistorical) {
          await flowChatStore.loadSessionHistory(firstSession.sessionId, workspacePath);
        }
        flowChatStore.switchSession(firstSession.sessionId);
        openScene('session');
        switchLeftPanelTab('sessions');
        return;
      }
    }

    // 没有已有会话，显式传入 workspacePath 创建 Code 会话，避免被 assistant workspace 覆盖
    openScene('session');
    switchLeftPanelTab('sessions');
    await flowChatManager.createChatSession({ workspacePath: workspacePath || undefined }, 'agentic');
  }, [currentWorkspace, openedWorkspacesList, openScene, setActiveWorkspace, switchLeftPanelTab]);

  const handleOpenAssistantModeSession = useCallback(async () => {
    const targetAssistantWorkspace =
      isAssistantWorkspaceActive && currentWorkspace?.workspaceKind === WorkspaceKind.Assistant
        ? currentWorkspace
        : defaultAssistantWorkspace;

    if (targetAssistantWorkspace && !isAssistantWorkspaceActive) {
      await setActiveWorkspace(targetAssistantWorkspace.id).catch(() => {});
    }

    const workspacePath = targetAssistantWorkspace?.rootPath;
    const state = flowChatStore.getState();

    if (workspacePath) {
      const workspaceSessions = Array.from(state.sessions.values())
        .filter(s =>
          (s.workspacePath || workspacePath) === workspacePath &&
          !s.parentSessionId
        )
        .sort(compareSessionsForDisplay);

      if (workspaceSessions.length > 0) {
        const firstSession = workspaceSessions[0];
        if (firstSession.isHistorical) {
          await flowChatStore.loadSessionHistory(firstSession.sessionId, workspacePath);
        }
        flowChatStore.switchSession(firstSession.sessionId);
        openScene('session');
        switchLeftPanelTab('sessions');
        return;
      }
    }

    // 没有已有会话，新建 Claw 会话
    await handleCreateSession('Claw');
  }, [currentWorkspace, defaultAssistantWorkspace, handleCreateSession, isAssistantWorkspaceActive, openScene, setActiveWorkspace, switchLeftPanelTab]);

  const handleToggleNavDisplayMode = useCallback(() => {
    // 防止动画进行中重复触发
    if (modeSwitchTimerRef.current !== null) return;

    setIsModeSwitching(true);

    // 点击时同步计算目标模式，避免 timeout 闭包中读取到过期值
    const nextMode: NavDisplayMode = navDisplayMode === 'pro' ? 'assistant' : 'pro';

    // 200ms（clip-path 收缩到最小圆点）：只切换 nav 显示状态，不触发任何场景/会话操作
    if (modeSwitchSwapTimerRef.current !== null) {
      window.clearTimeout(modeSwitchSwapTimerRef.current);
    }
    modeSwitchSwapTimerRef.current = window.setTimeout(() => {
      setNavDisplayMode(nextMode);
      window.localStorage.setItem(NAV_DISPLAY_MODE_STORAGE_KEY, nextMode);
      modeSwitchSwapTimerRef.current = null;
    }, 200);

    // 480ms（动画完全结束）：再切场景和会话，避免 tab 文字在动画期间闪动
    modeSwitchTimerRef.current = window.setTimeout(() => {
      setIsModeSwitching(false);
      modeSwitchTimerRef.current = null;
      if (nextMode === 'assistant') {
        void handleOpenAssistantModeSession();
      } else {
        void handleOpenProModeSession();
      }
    }, 480);
  }, [navDisplayMode, handleOpenAssistantModeSession, handleOpenProModeSession]);

  let flatCounter = 0;

  const workspaceMenuPortal = workspaceMenuOpen ? createPortal(
    <div
      ref={workspaceMenuRef}
      className={`bitfun-nav-panel__workspace-menu${workspaceMenuClosing ? ' is-closing' : ''}`}
      role="menu"
      style={{ top: workspaceMenuPos.top, left: workspaceMenuPos.left }}
    >
      <button
        type="button"
        className="bitfun-nav-panel__workspace-menu-item"
        role="menuitem"
        onClick={() => {
          closeWorkspaceMenu();
          void handleOpenProject();
        }}
      >
        <FolderOpen size={13} />
        <span>{t('header.openProject')}</span>
      </button>
      <button
        type="button"
        className="bitfun-nav-panel__workspace-menu-item"
        role="menuitem"
        onClick={() => {
          closeWorkspaceMenu();
          handleNewProject();
        }}
      >
        <FolderPlus size={13} />
        <span>{t('header.newProject')}</span>
      </button>
      <div className="bitfun-nav-panel__workspace-menu-divider" role="separator" />
      <div className="bitfun-nav-panel__workspace-menu-section-title">
        <History size={12} aria-hidden="true" />
        <span>{t('header.recentWorkspaces')}</span>
      </div>
      {recentWorkspaces.length === 0 ? (
        <div className="bitfun-nav-panel__workspace-menu-empty">
          <span>{t('header.noRecentWorkspaces')}</span>
        </div>
      ) : (
        <div className="bitfun-nav-panel__workspace-menu-workspaces">
          {recentWorkspaces.map((workspace) => (
            <button
              key={workspace.id}
              type="button"
              className="bitfun-nav-panel__workspace-menu-item bitfun-nav-panel__workspace-menu-item--workspace"
              role="menuitem"
              title={workspace.rootPath}
              onClick={() => { void handleSwitchWorkspace(workspace.id); }}
            >
              <FolderOpen size={13} aria-hidden="true" />
              <span className="bitfun-nav-panel__workspace-menu-item-main">{workspace.name}</span>
              {workspace.id === currentWorkspace?.id ? <Check size={12} aria-hidden="true" /> : null}
            </button>
          ))}
        </div>
      )}
    </div>,
    document.body
  ) : null;

  const personaTooltip = t('nav.items.persona');
  const createSessionTooltip = t('nav.sessions.newClawSession');
  const createAssistantTooltip = t('nav.workspaces.actions.newAssistant');
  const scheduledJobsTooltip = t('nav.scheduledJobs.open');
  const openProjectTooltip = t('header.openProject');
  const createCodeTooltip = t('nav.sessions.newCodeSession');
  const createCoworkTooltip = t('nav.sessions.newCoworkSession');
  const isAssistantNavMode = navDisplayMode === 'assistant';
  const navModeLabel = isAssistantNavMode
    ? t('nav.displayModes.assistant')
    : t('nav.displayModes.pro');
  const navModeHint = isAssistantNavMode
    ? t('nav.displayModes.switchToPro')
    : t('nav.displayModes.switchToAssistant');
  const navModeDesc = isAssistantNavMode
    ? t('nav.displayModes.assistantDesc')
    : t('nav.displayModes.proDesc');
  const navSections = useMemo(
    () => NAV_SECTIONS.filter(section => isAssistantNavMode ? section.id === 'assistants' : section.id === 'workspace'),
    [isAssistantNavMode]
  );
  const myAgentEntryLabel = t('nav.actions.openMyAgent');
  return (
    <>
      <div className="bitfun-nav-panel__workspace-toolbar">
          <button
            type="button"
            className={[
              'bitfun-nav-panel__mode-switch',
              isAssistantNavMode && 'is-assistant',
              isModeSwitching && 'is-switching',
            ].filter(Boolean).join(' ')}
            onClick={handleToggleNavDisplayMode}
            aria-label={navModeHint}
            aria-pressed={isAssistantNavMode}
          >
            <span className="bitfun-nav-panel__mode-switch-logo" aria-hidden="true">
              {isAssistantNavMode ? (
                <>
                  <img
                    className="bitfun-nav-panel__mode-switch-logo-image bitfun-nav-panel__mode-switch-logo-image--default"
                    src={modeLogoSrc}
                    alt=""
                    onError={() => setModeLogoSrc('/Logo-ICON.png')}
                  />
                  <img
                    className="bitfun-nav-panel__mode-switch-logo-image bitfun-nav-panel__mode-switch-logo-image--hover"
                    src={modeLogoHoverSrc}
                    alt=""
                    onError={() => setModeLogoHoverSrc('/Logo-ICON.png')}
                  />
                </>
              ) : (
                <img
                  className="bitfun-nav-panel__mode-switch-logo-image bitfun-nav-panel__mode-switch-logo-image--static"
                  src="/Logo-ICON.png"
                  alt=""
                />
              )}
            </span>
            <span className="bitfun-nav-panel__mode-switch-copy">
              <span className="bitfun-nav-panel__mode-switch-label">
                {navModeLabel}
                {isAssistantNavMode && <Badge variant="neutral">Beta</Badge>}
              </span>
              <span className="bitfun-nav-panel__mode-switch-sub">
                <span className="bitfun-nav-panel__mode-switch-desc">
                  <span className="bitfun-nav-panel__mode-switch-desc-main">{navModeDesc}</span>
                </span>
                <span className="bitfun-nav-panel__mode-switch-hint">{navModeHint}</span>
              </span>
            </span>
          </button>
        <div className="bitfun-nav-panel__workspace-create-group">
          {isAssistantNavMode ? (
            <>
              <Tooltip content={createSessionTooltip} placement="right" followCursor>
                <button
                  type="button"
                  className="bitfun-nav-panel__workspace-create-main bitfun-nav-panel__workspace-create-main--split-left"
                  onClick={() => { void handleCreateAssistantSession(); }}
                  aria-label={createSessionTooltip}
                >
                  <Plus size={14} />
                  <span>{t('nav.sessions.newSession')}</span>
                </button>
              </Tooltip>
              <Tooltip content={scheduledJobsTooltip} placement="right" followCursor>
                <button
                  type="button"
                  className="bitfun-nav-panel__workspace-create-main bitfun-nav-panel__workspace-create-main--split-right"
                  onClick={handleOpenScheduledJobsDialog}
                  aria-label={scheduledJobsTooltip}
                  disabled={!resolvedAssistantWorkspace}
                >
                  <Clock3 size={14} />
                  <span>{t('nav.scheduledJobs.shortLabel')}</span>
                </button>
              </Tooltip>
            </>
          ) : (
            <>
              <Tooltip content={createCodeTooltip} placement="right" followCursor>
                <button
                  type="button"
                  className="bitfun-nav-panel__workspace-create-main bitfun-nav-panel__workspace-create-main--split-left"
                  onClick={handleCreateCodeSession}
                  aria-label={createCodeTooltip}
                >
                  <Plus size={14} />
                  <span>{t('nav.sessions.modeCode')}</span>
                </button>
              </Tooltip>
              <Tooltip content={createCoworkTooltip} placement="right" followCursor>
                <button
                  type="button"
                  className="bitfun-nav-panel__workspace-create-main bitfun-nav-panel__workspace-create-main--split-right"
                  onClick={handleCreateCoworkSession}
                  aria-label={createCoworkTooltip}
                >
                  <Plus size={14} />
                  <span>{t('nav.sessions.modeCowork')}</span>
                </button>
              </Tooltip>
            </>
          )}
        </div>
      </div>

      <div className={`bitfun-nav-panel__sections${isModeSwitching ? ' is-mode-switching' : ''}`}>
        {navSections.map(section => {
          const isSectionOpen = expandedSections.has(section.id);
          const isCollapsible = !!section.collapsible;
          const showItems     = !isCollapsible || isSectionOpen;
          const sectionDir    = getSectionDepartDir(section.id);
          const sectionDepartCls = sectionDir ? ` is-departing-${sectionDir}` : '';

          const sectionSceneId = section.sceneId;

          return (
            <div key={section.id} className={`bitfun-nav-panel__section${sectionDepartCls}`}>
              {section.label && (
                <SectionHeader
                  label={getSectionLabel(section.id, section.label) ?? section.label}
                  collapsible={isCollapsible}
                  isOpen={isSectionOpen}
                  onToggle={() => toggleSection(section.id)}
                  onSceneOpen={sectionSceneId ? () => openScene(sectionSceneId) : undefined}
                  actions={section.id === 'assistants' ? (
                    <div className="bitfun-nav-panel__workspace-action-wrap">
                      <Tooltip content={createAssistantTooltip} placement="right" followCursor>
                        <button
                          type="button"
                          className="bitfun-nav-panel__section-action"
                          aria-label={createAssistantTooltip}
                          onClick={() => { void handleCreateAssistantWorkspace(); }}
                        >
                          <Plus size={13} />
                        </button>
                      </Tooltip>
                    </div>
                  ) : section.id === 'workspace' ? (
                    <div className="bitfun-nav-panel__workspace-action-wrap">
                      <Tooltip content={openProjectTooltip} placement="right" followCursor disabled={workspaceMenuOpen}>
                        <button
                          ref={workspaceMenuButtonRef}
                          type="button"
                          className={`bitfun-nav-panel__section-action${workspaceMenuOpen ? ' is-active' : ''}`}
                          aria-label={openProjectTooltip}
                          aria-expanded={workspaceMenuOpen}
                          onClick={toggleWorkspaceMenu}
                        >
                          <Plus size={13} />
                        </button>
                      </Tooltip>
                    </div>
                  ) : undefined}
                />
              )}

              <div className={`bitfun-nav-panel__collapsible${showItems ? '' : ' is-collapsed'}`}>
                <div className="bitfun-nav-panel__collapsible-inner">
                  <div className="bitfun-nav-panel__items">
                    {section.id === 'assistants' && <WorkspaceListSection variant="assistants" />}
                    {section.id === 'workspace' && <WorkspaceListSection variant="projects" />}
                    {section.items.map(item => {
                      const currentFlatIdx = flatCounter++;
                      const { tab } = item;
                      const dir = getDepartDir(currentFlatIdx);
                      const isActive = item.navSceneId
                        ? false
                        : item.sceneId
                          ? item.sceneId === activeTabId
                          : activeTabId === 'session' && tab === activeTab;
                      const displayLabel = item.labelKey ? t(item.labelKey) : (item.label ?? '');
                      const tooltipContent = item.tooltipKey ? t(item.tooltipKey) : undefined;
                      const departCls = dir ? ` is-departing-${dir}` : '';

                      return (
                        <div key={tab} className={`bitfun-nav-panel__item-slot${departCls}`}>
                          <NavItem
                            item={item}
                            displayLabel={displayLabel}
                            tooltipContent={tooltipContent}
                            isActive={isActive}
                            onClick={() => handleItemClick(tab, item)}
                            actionIcon={tab === 'sessions' ? Plus : undefined}
                            actionTitle={
                              tab === 'sessions'
                                ? isAssistantWorkspaceActive
                                  ? t('nav.sessions.newClawSession')
                                  : defaultSessionMode === 'cowork'
                                    ? t('nav.sessions.newCoworkSession')
                                    : t('nav.sessions.newCodeSession')
                                : undefined
                            }
                            onActionClick={tab === 'sessions' ? handleCreateSession : undefined}
                          />
                        </div>
                      );
                    })}
                  </div>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {isAssistantNavMode && (
        <div className="bitfun-nav-panel__assistant-footer">
          <Tooltip content={personaTooltip} placement="right" followCursor>
            <button
              type="button"
              className={`bitfun-nav-panel__assistant-entry${activeTabId === 'my-agent' ? ' is-active' : ''}`}
              onClick={handleOpenProfile}
              aria-label={myAgentEntryLabel}
            >
              <Bot size={16} className="bitfun-nav-panel__assistant-entry-icon" />
              <span className="bitfun-nav-panel__assistant-entry-label">{myAgentEntryLabel}</span>
            </button>
          </Tooltip>
        </div>
      )}

      <div className="bitfun-nav-panel__miniapp-footer">
        <MiniAppEntry
          isActive={activeTabId === 'miniapps' || !!activeMiniAppId}
          activeMiniAppId={activeMiniAppId}
          onOpenMiniApps={() => openScene('miniapps')}
          onOpenMiniApp={(appId) => openScene(`miniapp:${appId}`)}
        />
      </div>

      {workspaceMenuPortal}
      <ScheduledJobsDialog
        isOpen={isScheduledJobsDialogOpen}
        onClose={() => setIsScheduledJobsDialogOpen(false)}
        targetWorkspacePath={resolvedAssistantWorkspace?.rootPath}
        targetSessionId={resolvedAssistantSessionId}
        assistantName={resolvedAssistantDisplayName}
        hideTargetFields
        listScope="workspace"
      />
    </>
  );
};

export default MainNav;
