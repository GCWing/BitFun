/**
 * Main application layout.
 *
 * Column structure (top to bottom):
 *   WorkspaceBody (flex:1) — contains UnifiedTopBar + full-width content area
 *                             floating: SessionCapsule
 *
 * TitleBar removed; window controls moved to UnifiedTopBar, dialogs managed here.
 */

import React, { useState, useCallback, useEffect, useMemo, useRef, useContext } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useWorkspaceContext } from '../../infrastructure/contexts/WorkspaceContext';
import { useWindowControls } from '../hooks/useWindowControls';
import { useAssistantBootstrap } from '../hooks/useAssistantBootstrap';
import { useApp } from '../hooks/useApp';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { configManager } from '@/infrastructure/config';

type TransitionDirection = 'entering' | 'returning' | null;
import { FlowChatManager } from '../../flow_chat/services/FlowChatManager';
import WorkspaceBody from './WorkspaceBody';
import { ToolbarMode, useToolbarModeContext } from '../../flow_chat';
import { NewProjectDialog } from '../components/NewProjectDialog';
import { AboutDialog } from '../components/AboutDialog';
import { MCPInteractionDialog } from '../components/MCPInteractionDialog/MCPInteractionDialog';
import { WorkspaceManager } from '../../tools/workspace';
import { workspaceAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';
import { WorkspaceKind } from '@/shared/types';
import { SSHContext } from '@/features/ssh-remote/SSHRemoteContext';
import { shortcutManager, parseStoredKeybindings } from '@/infrastructure/services/ShortcutManager';
import { useSessionModeStore } from '../stores/sessionModeStore';
import './AppLayout.scss';

const log = createLogger('AppLayout');
const RECENT_WORKSPACE_PRELOAD_LIMIT = 7;
const RECENT_SESSION_WARMUP_LIMIT = 5;
const RECENT_DISPATCHER_WARMUP_LIMIT = 3;

interface AppLayoutProps {
  className?: string;
}

const AppLayout: React.FC<AppLayoutProps> = ({ className = '' }) => {
  const { t } = useI18n('components');
  const {
    currentWorkspace,
    hasWorkspace,
    openWorkspace,
    switchWorkspace,
    recentWorkspaces,
    loading,
  } = useWorkspaceContext();
  const sshContext = useContext(SSHContext);
  /** When SSH finishes connecting, re-run FlowChat init (first run may have skipped while disconnected). */
  const remoteSshFlowChatKey =
    currentWorkspace?.workspaceKind === WorkspaceKind.Remote && currentWorkspace?.connectionId
      ? sshContext?.workspaceStatuses[currentWorkspace.connectionId] ?? 'unknown'
      : 'local';

  const { isToolbarMode } = useToolbarModeContext();
  const { ensureForWorkspace: ensureAssistantBootstrapForWorkspace } = useAssistantBootstrap();

  const { handleMinimize, handleMaximize, handleClose, isMaximized } =
    useWindowControls({ isToolbarMode });

  const { state, switchLeftPanelTab, toggleLeftPanel, toggleRightPanel } = useApp();

  // ── Load user keybinding overrides from config on startup ────────────────
  useEffect(() => {
    const load = async () => {
      try {
        const raw = await configManager.getConfig('app.keybindings');
        const overrides = parseStoredKeybindings(raw);
        if (Object.keys(overrides).length > 0) {
          shortcutManager.loadUserOverrides(overrides);
        }
      } catch {
        // No overrides stored yet — that's fine
      }
    };

    void load();

    const unsubscribe = configManager.onConfigChange((path) => {
      if (path === 'app.keybindings') void load();
    });

    return () => unsubscribe();
  }, []);
  const isTransitioning = false;
  const transitionDir: TransitionDirection = null;
  const recentPreloadKeyRef = useRef<string | null>(null);

  // Auto-open last workspace on startup
  const autoOpenAttemptedRef = useRef(false);
  useEffect(() => {
    if (autoOpenAttemptedRef.current || loading) return;
    if (!hasWorkspace && recentWorkspaces.length > 0) {
      autoOpenAttemptedRef.current = true;
      switchWorkspace(recentWorkspaces[0]).catch(err => {
        log.warn('Auto-open recent workspace failed', err);
      });
    } else {
      autoOpenAttemptedRef.current = true;
    }
  }, [hasWorkspace, loading, recentWorkspaces, switchWorkspace]);

  // Dialog state (previously in TitleBar)
  const [showNewProjectDialog, setShowNewProjectDialog] = useState(false);
  const [showAboutDialog, setShowAboutDialog] = useState(false);
  const [showWorkspaceStatus, setShowWorkspaceStatus] = useState(false);
  const handleOpenProject = useCallback(async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('header.selectProjectDirectory'),
      });

      if (selected && typeof selected === 'string') {
        await openWorkspace(selected);
      }
    } catch (error) {
      log.error('Failed to open project', error);
    }
  }, [openWorkspace, t]);
  const handleNewProject = useCallback(() => setShowNewProjectDialog(true), []);
  const handleShowAbout  = useCallback(() => setShowAboutDialog(true), []);

  const handleConfirmNewProject = useCallback(async (parentPath: string, projectName: string) => {
    const normalized = parentPath.replace(/\\/g, '/');
    const newProjectPath = `${normalized}/${projectName}`;
    try {
      await workspaceAPI.createDirectory(newProjectPath);
      await openWorkspace(newProjectPath);
    } catch (error) {
      log.error('Failed to create project', error);
      throw error;
    }
  }, [openWorkspace]);

  // Listen for nav-panel events dispatched by the workspace area
  useEffect(() => {
    const onOpenProject = () => { void handleOpenProject(); };
    const onNewProject = () => handleNewProject();
    const onShowAbout = () => handleShowAbout();
    window.addEventListener('nav:open-project', onOpenProject);
    window.addEventListener('nav:new-project', onNewProject);
    window.addEventListener('nav:show-about', onShowAbout);
    return () => {
      window.removeEventListener('nav:open-project', onOpenProject);
      window.removeEventListener('nav:new-project', onNewProject);
      window.removeEventListener('nav:show-about', onShowAbout);
    };
  }, [handleNewProject, handleOpenProject, handleShowAbout]);

  // macOS native menubar events (previously in TitleBar)
  const isMacOS = useMemo(() => {
    const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
    return isTauri && typeof navigator?.platform === 'string' && navigator.platform.toUpperCase().includes('MAC');
  }, []);

  useEffect(() => {
    if (!isMacOS) return;
    let unlistenFns: Array<() => void> = [];
    void (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        const { open } = await import('@tauri-apps/plugin-dialog');
        unlistenFns.push(await listen('bitfun_menu_open_project', async () => {
          try {
            const selected = await open({ directory: true, multiple: false }) as string;
            if (selected) await openWorkspace(selected);
          } catch {}
        }));
        unlistenFns.push(await listen('bitfun_menu_new_project', () => handleNewProject()));
        unlistenFns.push(await listen('bitfun_menu_about', () => handleShowAbout()));
      } catch {}
    })();
    return () => { unlistenFns.forEach(fn => fn()); unlistenFns = []; };
  }, [isMacOS, openWorkspace, handleNewProject, handleShowAbout]);

  // Initialize FlowChatManager
  React.useEffect(() => {
    const initializeFlowChat = async () => {
      if (!currentWorkspace?.rootPath) return;

      // Remote session index and turns live under ~/.bitfun/remote_ssh/... (local disk).
      // Always initialize FlowChat so historical sessions list even when SSH is not connected yet.
      try {
        const explicitPreferredMode =
          sessionStorage.getItem('bitfun:flowchat:preferredMode') ||
          undefined;
        if (explicitPreferredMode) {
          sessionStorage.removeItem('bitfun:flowchat:preferredMode');
        }

        const initializationPreferredMode =
          currentWorkspace.workspaceKind === WorkspaceKind.Assistant
            ? 'Claw'
            : explicitPreferredMode;

        const flowChatManager = FlowChatManager.getInstance();
        const hasHistoricalSessions = await flowChatManager.initialize(
          currentWorkspace.rootPath,
          initializationPreferredMode,
          currentWorkspace.workspaceKind === WorkspaceKind.Remote
            ? currentWorkspace.connectionId
            : undefined,
          currentWorkspace.workspaceKind === WorkspaceKind.Remote
            ? currentWorkspace.sshHost
            : undefined
        );

        let sessionId: string | undefined;
        const { flowChatStore } = await import('@/flow_chat/store/FlowChatStore');
        if (!hasHistoricalSessions) {
          const initialSessionMode =
            currentWorkspace.workspaceKind === WorkspaceKind.Assistant
              ? 'Claw'
              : explicitPreferredMode || 'agentic';
          sessionId = await flowChatManager.createChatSession({}, initialSessionMode);
        }

        const activeSessionId = sessionId || flowChatStore.getState().activeSessionId;
        if (currentWorkspace.workspaceKind === WorkspaceKind.Assistant && activeSessionId) {
          ensureAssistantBootstrapForWorkspace(currentWorkspace, activeSessionId);
        }

        const pendingDescription = sessionStorage.getItem('pendingProjectDescription');
        if (pendingDescription && pendingDescription.trim()) {
          sessionStorage.removeItem('pendingProjectDescription');

          setTimeout(async () => {
            try {
              const targetSessionId = sessionId || flowChatStore.getState().activeSessionId;

              if (!targetSessionId) {
                log.error('Cannot find active session ID');
                return;
              }

              const fullMessage = t('appLayout.projectRequestMessage', { description: pendingDescription });
              await flowChatManager.sendMessage(fullMessage, targetSessionId);

              import('@/shared/notification-system').then(({ notificationService }) => {
                notificationService.success(t('appLayout.projectRequestSent'), { duration: 3000 });
              });
            } catch (sendError) {
              log.error('Failed to send project description', sendError);
              import('@/shared/notification-system').then(({ notificationService }) => {
                notificationService.error(t('appLayout.projectRequestSendFailed'), { duration: 5000 });
              });
            }
          }, 500);
        }

        const pendingSettings = sessionStorage.getItem('pendingOpenSettings');
        if (pendingSettings) {
          sessionStorage.removeItem('pendingOpenSettings');
          setTimeout(async () => {
            try {
              const { quickActions } = await import('@/shared/services/ide-control');
              await quickActions.openSettings(pendingSettings);
            } catch (settingsError) {
              log.error('Failed to open pending settings', settingsError);
            }
          }, 500);
        }
      } catch (error) {
        log.error('FlowChatManager initialization failed', error);
        import('@/shared/notification-system').then(({ notificationService }) => {
          notificationService.error(t('appLayout.flowChatInitFailed'), { duration: 5000 });
        });
      }
    };

    initializeFlowChat();
  }, [
    currentWorkspace,
    currentWorkspace?.id,
    currentWorkspace?.rootPath,
    currentWorkspace?.workspaceKind,
    currentWorkspace?.connectionId,
    currentWorkspace?.sshHost,
    remoteSshFlowChatKey,
    ensureAssistantBootstrapForWorkspace,
    t,
  ]);

  React.useEffect(() => {
    if (loading || recentWorkspaces.length === 0) {
      return;
    }

    const recentProjectWorkspaces = recentWorkspaces.filter(
      workspace => workspace.workspaceKind !== WorkspaceKind.Assistant
    );
    const preloadTargetMap = new Map<string, (typeof recentWorkspaces)[number]>();
    recentProjectWorkspaces
      .slice(0, RECENT_WORKSPACE_PRELOAD_LIMIT)
      .forEach(workspace => preloadTargetMap.set(workspace.id, workspace));
    const preloadTargets = Array.from(preloadTargetMap.values());
    const preloadKey = preloadTargets
      .map(workspace => `${workspace.id}:${workspace.rootPath}:${workspace.connectionId ?? ''}:${workspace.sshHost ?? ''}`)
      .join('|');

    if (!preloadKey || recentPreloadKeyRef.current === preloadKey) {
      return;
    }
    recentPreloadKeyRef.current = preloadKey;

    let cancelled = false;
    void (async () => {
      try {
        const result = await FlowChatManager.getInstance().preloadRecentWorkspaceSessions(
          preloadTargets,
          {
            metadataLimit: RECENT_WORKSPACE_PRELOAD_LIMIT,
            warmHistoryCount: RECENT_SESSION_WARMUP_LIMIT,
          }
        );
        if (!cancelled) {
          log.info('Recent workspace session preload completed', result);
        }
      } catch (error) {
        if (!cancelled) {
          log.warn('Recent workspace session preload failed', error);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [loading, recentWorkspaces]);

  React.useEffect(() => {
    if (loading) {
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const result = await FlowChatManager.getInstance().preloadAgenticOsSessions({
          warmDispatcherCount: RECENT_DISPATCHER_WARMUP_LIMIT,
        });
        if (!cancelled) {
          log.info('Agentic OS session preload completed', result);
        }
      } catch (error) {
        if (!cancelled) {
          log.warn('Agentic OS session preload failed', error);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [loading]);

  // Save in-progress conversations on window close
  React.useEffect(() => {
    let unlistenFn: (() => void) | null = null;

    const setupWindowCloseListener = async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const currentWindow = getCurrentWindow();

        unlistenFn = await currentWindow.onCloseRequested(async (event: { preventDefault: () => void }) => {
          try {
            event.preventDefault();
            const flowChatManager = FlowChatManager.getInstance();
            await flowChatManager.saveAllInProgressTurns();
            await currentWindow.close();
          } catch (error) {
            log.error('Failed to save conversations, closing anyway', error);
            await currentWindow.close();
          }
        });
      } catch (error) {
        log.error('Failed to setup window close listener', error);
      }
    };

    setupWindowCloseListener();
    return () => { if (unlistenFn) unlistenFn(); };
  }, []);

  // Handle switch-to-files-panel event
  React.useEffect(() => {
    const handleSwitchToFilesPanel = () => {
      switchLeftPanelTab('files');
      if (state.layout.leftPanelCollapsed) toggleLeftPanel();
      if (state.layout.rightPanelCollapsed) {
        setTimeout(() => toggleRightPanel(), 100);
      }
    };

    window.addEventListener('switch-to-files-panel', handleSwitchToFilesPanel);
    return () => window.removeEventListener('switch-to-files-panel', handleSwitchToFilesPanel);
  }, [state.layout.leftPanelCollapsed, state.layout.rightPanelCollapsed, switchLeftPanelTab, toggleLeftPanel, toggleRightPanel]);

  // Toolbar send message
  React.useEffect(() => {
    const handleToolbarSendMessage = async (event: Event) => {
      const customEvent = event as CustomEvent<{ message: string; sessionId: string }>;
      const { message, sessionId } = customEvent.detail;
      if (message && sessionId) {
        try {
          const flowChatManager = FlowChatManager.getInstance();
          await flowChatManager.sendMessage(message, sessionId);
        } catch (error) {
          log.error('Failed to send toolbar message', error);
        }
      }
    };
    window.addEventListener('toolbar-send-message', handleToolbarSendMessage);
    return () => window.removeEventListener('toolbar-send-message', handleToolbarSendMessage);
  }, []);

  // Toggle left panel: mod+B (VS Code convention)
  useShortcut(
    'panel.toggleLeft',
    { key: 'B', ctrl: true, scope: 'app' },
    () => toggleLeftPanel(),
    { priority: 5, description: 'keyboard.shortcuts.panel.toggleLeft' }
  );

  // Collapse/expand both panels: mod+Shift+B
  useShortcut(
    'panel.toggleBoth',
    { key: 'B', ctrl: true, shift: true, scope: 'app' },
    () => {
      const bothCollapsed = state.layout.leftPanelCollapsed && state.layout.rightPanelCollapsed;
      if (bothCollapsed) {
        toggleLeftPanel();
        setTimeout(() => toggleRightPanel(), 50);
      } else {
        if (!state.layout.leftPanelCollapsed) toggleLeftPanel();
        if (!state.layout.rightPanelCollapsed) toggleRightPanel();
      }
    },
    { priority: 5, description: 'keyboard.shortcuts.panel.toggleBoth' }
  );

  // Toolbar cancel task
  React.useEffect(() => {
    const handleToolbarCancelTask = async () => {
      try {
        const flowChatManager = FlowChatManager.getInstance();
        await flowChatManager.cancelCurrentTask();
      } catch (error) {
        log.error('Failed to cancel toolbar task', error);
      }
    };
    window.addEventListener('toolbar-cancel-task', handleToolbarCancelTask);
    return () => window.removeEventListener('toolbar-cancel-task', handleToolbarCancelTask);
  }, []);

  // Create FlowChat session (toolbar / floating UI). detail.mode selects a fixed top-level agent.
  const handleCreateFlowChatSession = React.useCallback(async (mode?: 'code' | 'cowork' | 'design') => {
    try {
      const flowChatManager = FlowChatManager.getInstance();
      const setMode = useSessionModeStore.getState().setMode;
      if (mode === 'cowork') {
        setMode('cowork');
        await flowChatManager.createChatSession({}, 'Cowork');
      } else if (mode === 'design') {
        setMode('design');
        await flowChatManager.createChatSession({}, 'Design');
      } else {
        setMode('code');
        await flowChatManager.createChatSession({}, 'agentic');
      }
    } catch (error) {
      log.error('Failed to create FlowChat session', error);
    }
  }, []);

  React.useEffect(() => {
    const handler = (e: Event) => {
      const mode = (e as CustomEvent<{ mode?: 'code' | 'cowork' | 'design' }>).detail?.mode;
      void handleCreateFlowChatSession(
        mode === 'cowork' || mode === 'design' ? mode : 'code'
      );
    };
    window.addEventListener('toolbar-create-session', handler);
    return () => window.removeEventListener('toolbar-create-session', handler);
  }, [handleCreateFlowChatSession]);

  // Global drag-and-drop
  React.useEffect(() => {
    const handleDragStart = (e: DragEvent) => {
      if (e.dataTransfer) {
        if (e.dataTransfer.types.length === 0) e.dataTransfer.setData('text/plain', 'dragging');
        e.dataTransfer.effectAllowed = 'copy';
      }
    };
    const handleDragOver  = (e: DragEvent) => e.preventDefault();
    const handleDragEnter = (_e: DragEvent) => {};
    const handleDrop      = (e: DragEvent) => { if (!e.defaultPrevented) e.preventDefault(); };

    document.addEventListener('dragstart', handleDragStart, true);
    document.addEventListener('dragover',  handleDragOver,  true);
    document.addEventListener('dragenter', handleDragEnter, true);
    document.addEventListener('drop',      handleDrop,      true);

    return () => {
      document.removeEventListener('dragstart', handleDragStart, true);
      document.removeEventListener('dragover',  handleDragOver,  true);
      document.removeEventListener('dragenter', handleDragEnter, true);
      document.removeEventListener('drop',      handleDrop,      true);
    };
  }, []);

  const containerClassName = [
    'bitfun-app-layout',
    isMacOS ? 'bitfun-app-layout--macos' : '',
    className,
    isTransitioning ? 'bitfun-app-layout--transitioning' : '',
  ].filter(Boolean).join(' ');

  if (isToolbarMode) return <ToolbarMode />;

  return (
    <>
      <div className={containerClassName} data-testid="app-layout">
        {/* Main content — always render WorkspaceBody; WelcomeScene in viewport handles no-workspace state */}
        <main className="bitfun-app-main-workspace" data-testid="app-main-content">
          <WorkspaceBody
            onMinimize={isMacOS ? undefined : handleMinimize}
            onMaximize={handleMaximize}
            onClose={isMacOS ? undefined : handleClose}
            isMaximized={isMaximized}
            isEntering={transitionDir === 'entering'}
            isExiting={transitionDir === 'returning'}
          />
        </main>
      </div>

      {/* Dialogs (previously owned by TitleBar) */}
      <NewProjectDialog
        isOpen={showNewProjectDialog}
        onClose={() => setShowNewProjectDialog(false)}
        onConfirm={handleConfirmNewProject}
        defaultParentPath={hasWorkspace ? currentWorkspace?.rootPath : undefined}
      />
      <AboutDialog
        isOpen={showAboutDialog}
        onClose={() => setShowAboutDialog(false)}
      />
      <WorkspaceManager
        isVisible={showWorkspaceStatus}
        onClose={() => setShowWorkspaceStatus(false)}
        onWorkspaceSelect={() => {}}
      />
      <MCPInteractionDialog />
    </>
  );
};

export default AppLayout;
