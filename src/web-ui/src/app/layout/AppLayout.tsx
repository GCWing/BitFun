/**
 * Main application layout.
 *
 * Column structure (top to bottom):
 *   WorkspaceBody (flex:1) — contains NavBar (with WindowControls) + NavPanel + SceneArea
 *   OR StartupContent
 *
 * TitleBar removed; window controls moved to NavBar, dialogs managed here.
 */

import React, { useState, useCallback, useEffect, useMemo, useRef, useContext, lazy, Suspense } from 'react';
import { useWorkspaceContext } from '../../infrastructure/contexts/WorkspaceContext';
import { useWindowControls } from '../hooks/useWindowControls';
import { isWindowFullscreenShortcut } from '../hooks/windowFullscreenShortcut';
import { useAssistantBootstrap } from '../hooks/useAssistantBootstrap';
import { useApp } from '../hooks/useApp';
import { useSceneStore } from '../stores/sceneStore';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { FlowChatManager } from '../../flow_chat/services/FlowChatManager';
import WorkspaceBody from './WorkspaceBody';
import { useToolbarModeContext } from '../../flow_chat/components/toolbar-mode/ToolbarModeContext';
import { MCPInteractionDialog } from '../components/MCPInteractionDialog/MCPInteractionDialog';
import { workspaceAPI } from '@/infrastructure/api';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import type { CloseBehavior } from '@/infrastructure/api/service-api/SystemAPI';
import { confirmDialog } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import { DailyAppUpdateGate } from '@/infrastructure/update';
import { useI18n } from '@/infrastructure/i18n';
import { WorkspaceKind } from '@/shared/types';
import { SSHContext } from '@/features/ssh-remote/SSHRemoteContext';
import { shortcutManager, parseStoredKeybindings } from '@/infrastructure/services/ShortcutManager';
import { useSessionModeStore } from '../stores/sessionModeStore';
import { isMacOSDesktopRuntime } from '@/infrastructure/runtime';
import { flowChatSessionConfigForWorkspace } from '../utils/projectSessionWorkspace';
import { notificationService } from '@/shared/notification-system';
import './AppLayout.scss';

type TransitionDirection = 'entering' | 'returning' | null;

const log = createLogger('AppLayout');
const NewProjectDialog = lazy(() =>
  import('../components/NewProjectDialog').then(module => ({ default: module.NewProjectDialog }))
);
const ToolbarMode = lazy(() =>
  import('../../flow_chat/components/toolbar-mode/ToolbarMode').then(module => ({
    default: module.ToolbarMode,
  }))
);
const FloatingMiniChat = lazy(() =>
  import('./FloatingMiniChat').then(module => ({ default: module.FloatingMiniChat }))
);
const AboutDialog = lazy(() =>
  import('../components/AboutDialog').then(module => ({ default: module.AboutDialog }))
);
const WorkspaceManager = lazy(() => import('../../tools/workspace/components/WorkspaceManager'));

interface AppLayoutProps {
  className?: string;
}

interface AcpSessionCreationEventDetail {
  phase?: 'start' | 'finish';
  clientId?: string;
  action?: 'create' | 'restore';
  requestId?: string;
  succeeded?: boolean;
}

interface WindowModeHint {
  id: number;
  title: string;
  detail: string;
}

const AppLayout: React.FC<AppLayoutProps> = ({ className = '' }) => {
  const { t } = useI18n('components');
  const { t: tCommon } = useI18n('common');
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
  const isMacOS = useMemo(() => {
    return isMacOSDesktopRuntime();
  }, []);

  const {
    handleMinimize,
    handleMaximize,
    handleToggleFullscreen,
    handleClose,
    isMaximized,
    isFullscreen,
    canUseNativeWindowControls,
  } =
    useWindowControls({ isToolbarMode });

  const { state, switchLeftPanelTab, toggleLeftPanel, toggleRightPanel } = useApp();
  const [windowModeHint, setWindowModeHint] = useState<WindowModeHint | null>(null);
  const windowModeHintTimerRef = useRef<number | null>(null);

  const showWindowFullscreenHint = useCallback((enteredFullscreen: boolean) => {
    if (windowModeHintTimerRef.current) {
      window.clearTimeout(windowModeHintTimerRef.current);
    }

    const shortcut = isMacOS ? 'Control+Command+F' : 'F11';
    setWindowModeHint({
      id: Date.now(),
      title: t(enteredFullscreen
        ? 'appLayout.windowFullscreenEntered'
        : 'appLayout.windowFullscreenExited'),
      detail: t(enteredFullscreen
        ? 'appLayout.windowFullscreenExitHint'
        : 'appLayout.windowFullscreenEnterHint', { shortcut }),
    });

    windowModeHintTimerRef.current = window.setTimeout(() => {
      setWindowModeHint(null);
      windowModeHintTimerRef.current = null;
    }, 2200);
  }, [isMacOS, t]);

  useEffect(() => {
    return () => {
      if (windowModeHintTimerRef.current) {
        window.clearTimeout(windowModeHintTimerRef.current);
      }
    };
  }, []);

  // ── Load user keybinding overrides from config on startup ────────────────
  useEffect(() => {
    const load = async () => {
      try {
        const raw = await configManager.getOptionalConfig('app.keybindings');
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

  useEffect(() => {
    if (!canUseNativeWindowControls || isToolbarMode) return;

    const handleSystemFullscreenShortcut = (event: KeyboardEvent) => {
      if (!isWindowFullscreenShortcut(event)) return;

      // OS fullscreen is a platform window command, not the app's maximize
      // shortcut and not an internal panel fullscreen action. Use a raw
      // listener because ShortcutManager intentionally maps Ctrl to Cmd on
      // macOS for "mod" shortcuts, while system fullscreen requires the exact
      // Control+Command+F chord.
      event.preventDefault();
      event.stopPropagation();
      void handleToggleFullscreen().then((enteredFullscreen) => {
        if (typeof enteredFullscreen === 'boolean') {
          showWindowFullscreenHint(enteredFullscreen);
        }
      });
    };

    window.addEventListener('keydown', handleSystemFullscreenShortcut, { capture: true });
    return () => {
      window.removeEventListener('keydown', handleSystemFullscreenShortcut, { capture: true });
    };
  }, [canUseNativeWindowControls, handleToggleFullscreen, isToolbarMode, showWindowFullscreenHint]);
  const activeSceneId = useSceneStore(s => s.activeTabId);
  const isAgentScene = activeSceneId === 'session';
  const isWelcomeScene = activeSceneId === 'welcome';

  const isTransitioning = false;
  const transitionDir: TransitionDirection = null;

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
      const { pickWorkspaceDirectory } = await import(
        '@/infrastructure/peer-device/pickWorkspaceDirectory'
      );
      const selected = await pickWorkspaceDirectory({
        title: t('header.selectProjectDirectory'),
      });

      if (selected) {
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
    window.addEventListener('nav:open-project', onOpenProject);
    window.addEventListener('nav:new-project', onNewProject);
    return () => {
      window.removeEventListener('nav:open-project', onOpenProject);
      window.removeEventListener('nav:new-project', onNewProject);
    };
  }, [handleNewProject, handleOpenProject]);

  // macOS native menubar events (previously in TitleBar)
  useEffect(() => {
    if (!isMacOS) return;
    let unlistenFns: Array<() => void> = [];
    void (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        const { pickWorkspaceDirectory } = await import(
          '@/infrastructure/peer-device/pickWorkspaceDirectory'
        );
        unlistenFns.push(await listen('bitfun_menu_open_project', async () => {
          try {
            const selected = await pickWorkspaceDirectory({
              title: t('header.selectProjectDirectory'),
            });
            if (selected) await openWorkspace(selected);
          } catch {}
        }));
        unlistenFns.push(await listen('bitfun_menu_new_project', () => handleNewProject()));
        unlistenFns.push(await listen('bitfun_menu_about', () => handleShowAbout()));
      } catch {}
    })();
    return () => { unlistenFns.forEach(fn => fn()); unlistenFns = []; };
  }, [isMacOS, openWorkspace, handleNewProject, handleShowAbout, t]);

  // Initialize FlowChatManager
  React.useEffect(() => {
    let cancelled = false;
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
        if (cancelled) {
          return;
        }

        let sessionId: string | undefined;
        const { flowChatStore } = await import('@/flow_chat/store/FlowChatStore');
        if (cancelled) {
          return;
        }
        if (!hasHistoricalSessions) {
          const initialSessionMode =
            currentWorkspace.workspaceKind === WorkspaceKind.Assistant
              ? 'Claw'
              : explicitPreferredMode || 'agentic';
          sessionId = await flowChatManager.createChatSession(
            flowChatSessionConfigForWorkspace(currentWorkspace),
            initialSessionMode,
          );
          if (cancelled) {
            return;
          }
        }

        const activeSessionId = sessionId || flowChatStore.getState().activeSessionId;
        if (currentWorkspace.workspaceKind === WorkspaceKind.Assistant && activeSessionId) {
          ensureAssistantBootstrapForWorkspace(currentWorkspace, activeSessionId);
        }

        const pendingDescription = sessionStorage.getItem('pendingProjectDescription');
        if (pendingDescription && pendingDescription.trim()) {
          sessionStorage.removeItem('pendingProjectDescription');

          setTimeout(async () => {
            if (cancelled) {
              return;
            }
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
            if (cancelled) {
              return;
            }
            try {
              const { quickActions } = await import('@/shared/services/ide-control');
              await quickActions.openSettings(pendingSettings);
            } catch (settingsError) {
              log.error('Failed to open pending settings', settingsError);
            }
          }, 500);
        }
      } catch (error) {
        if (cancelled) {
          return;
        }
        log.error('FlowChatManager initialization failed', error);
        import('@/shared/notification-system').then(({ notificationService }) => {
          notificationService.error(t('appLayout.flowChatInitFailed'), { duration: 5000 });
        });
      }
    };

    initializeFlowChat();
    return () => {
      cancelled = true;
    };
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

  // When the user hides the main window (tray / macOS dock), the app keeps running.
  // `saveAllInProgressTurns` settles in-flight dialog turns for disk persistence, which
  // clears Agent companion desktop bubbles until the next chat update—so only run it
  // immediately before we actually exit the process.
  React.useEffect(() => {
    let unlistenFn: (() => void) | null = null;
    let handlingClose = false;

    const setupWindowCloseListener = async () => {
      if (!canUseNativeWindowControls) return;

      try {
        // Both macOS and Windows/Linux: Rust intercepts the native close request
        // and emits this event. We decide hide vs quit; persist interrupted turns only on quit.
        const [{ listen }, { invoke }] = await Promise.all([
          import('@tauri-apps/api/event'),
          import('@tauri-apps/api/core'),
        ]);

        const persistInterruptedTurnsForExit = async () => {
          try {
            const flowChatManager = FlowChatManager.getInstance();
            await flowChatManager.saveAllInProgressTurns();
          } catch (error) {
            log.error('Failed to save conversations before quit', error);
          }
        };

        unlistenFn = await listen('bitfun_main_window_close_requested', async () => {
          if (handlingClose) return;
          handlingClose = true;

          if (isMacOS) {
            // macOS always hides to keep the app alive in the dock.
            try {
              await invoke('hide_main_window_after_close_request');
            } catch (error) {
              log.error('Failed to hide main window after close request', error);
            }
            handlingClose = false;
            return;
          }

          // Windows / Linux: read the user's close-button preference.
          let behavior: CloseBehavior = 'minimize_to_tray';
          try {
            behavior = (await configManager.getConfig<CloseBehavior>('app.close_button_behavior')) ?? 'minimize_to_tray';
          } catch {
            // Fall back to minimize_to_tray if config cannot be read.
          }

          try {
            if (behavior === 'minimize_to_tray') {
              await systemAPI.minimizeToTray();
            } else if (behavior === 'ask') {
              const shouldQuit = await confirmDialog({
                title: tCommon('closeDialog.title'),
                message: tCommon('closeDialog.message'),
                confirmText: tCommon('closeDialog.quit'),
                cancelText: tCommon('closeDialog.minimizeToTray'),
                showCancel: true,
              });
              if (shouldQuit) {
                await persistInterruptedTurnsForExit();
                await systemAPI.quitApp();
              } else {
                await systemAPI.minimizeToTray();
              }
            } else {
              // quit
              await persistInterruptedTurnsForExit();
              await systemAPI.quitApp();
            }
          } catch (error) {
            log.error('Failed to handle close request', { behavior, error });
            try {
              await persistInterruptedTurnsForExit();
              await systemAPI.quitApp();
            } catch { /* ignore */ }
          } finally {
            handlingClose = false;
          }
        });
      } catch (error) {
        log.error('Failed to setup window close listener', error);
      }
    };

    setupWindowCloseListener();
    return () => { if (unlistenFn) unlistenFn(); };
  }, [canUseNativeWindowControls, isMacOS, tCommon]);

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

  // Create FlowChat session (toolbar / floating UI). detail.mode: 'cowork' → Cowork, else code (agentic).
  const handleCreateFlowChatSession = React.useCallback(async (mode?: 'code' | 'cowork') => {
    try {
      if (!currentWorkspace?.rootPath) {
        log.warn('Cannot create FlowChat session without an active workspace');
        return;
      }
      const flowChatManager = FlowChatManager.getInstance();
      const setMode = useSessionModeStore.getState().setMode;
      const sessionConfig = flowChatSessionConfigForWorkspace(currentWorkspace);
      if (mode === 'cowork') {
        setMode('cowork');
        await flowChatManager.createChatSession(sessionConfig, 'Cowork');
      } else {
        setMode('code');
        await flowChatManager.createChatSession(sessionConfig, 'agentic');
      }
    } catch (error) {
      log.error('Failed to create FlowChat session', error);
    }
  }, [currentWorkspace]);

  React.useEffect(() => {
    const handler = (e: Event) => {
      const mode = (e as CustomEvent<{ mode?: 'code' | 'cowork' }>).detail?.mode;
      void handleCreateFlowChatSession(mode === 'cowork' ? 'cowork' : 'code');
    };
    window.addEventListener('toolbar-create-session', handler);
    return () => window.removeEventListener('toolbar-create-session', handler);
  }, [handleCreateFlowChatSession]);

  React.useEffect(() => {
    const handler = (e: Event) => {
      const clientId = (e as CustomEvent<{ clientId?: string }>).detail?.clientId?.trim();
      if (!clientId) return;
      const config = currentWorkspace
        ? {
            workspacePath: currentWorkspace.rootPath,
            ...(currentWorkspace.workspaceKind === WorkspaceKind.Remote && currentWorkspace.connectionId
              ? { remoteConnectionId: currentWorkspace.connectionId }
              : {}),
            ...(currentWorkspace.workspaceKind === WorkspaceKind.Remote && currentWorkspace.sshHost
              ? { remoteSshHost: currentWorkspace.sshHost }
              : {}),
          }
        : {};
      void FlowChatManager.getInstance()
        .createAcpChatSession(clientId, config)
        .catch(error => log.error('Failed to create ACP FlowChat session', error));
    };
    window.addEventListener('bitfun:create-acp-session', handler);
    return () => window.removeEventListener('bitfun:create-acp-session', handler);
  }, [currentWorkspace]);

  React.useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<AcpSessionCreationEventDetail>).detail;
      const clientId = detail?.clientId?.trim() || 'ACP';
      const action = detail?.action === 'restore' ? 'restore' : 'create';
      const id = detail?.requestId?.trim() || `${action}:${clientId}`;
      if (detail?.phase === 'start') {
        notificationService.silent({
          title: clientId,
          message: tCommon('nav.workspaces.startingAcpSession'),
          type: 'info',
          metadata: { source: 'acp-session', clientId, action, requestId: id, phase: 'start' },
        });
      } else if (detail?.phase === 'finish') {
        const succeeded = detail.succeeded !== false;
        notificationService.silent({
          title: clientId,
          message: succeeded
            ? tCommon('nav.workspaces.acpSessionStarted')
            : tCommon('nav.workspaces.acpSessionStartFailed'),
          type: succeeded ? 'success' : 'error',
          metadata: { source: 'acp-session', clientId, action, requestId: id, phase: 'finish', succeeded },
        });
      }
    };
    window.addEventListener('bitfun:acp-session-creation', handler);
    return () => window.removeEventListener('bitfun:acp-session-creation', handler);
  }, [tCommon]);

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
    isFullscreen ? 'bitfun-app-layout--window-fullscreen' : '',
    isTransitioning ? 'bitfun-app-layout--transitioning' : '',
  ].filter(Boolean).join(' ');

  if (isToolbarMode) {
    return (
      <>
        <DailyAppUpdateGate />
        <Suspense fallback={null}>
          <ToolbarMode />
        </Suspense>
      </>
    );
  }

  return (
    <>
      <DailyAppUpdateGate />
      <div className={containerClassName} data-testid="app-layout">
        {windowModeHint && (
          <div
            key={windowModeHint.id}
            className="bitfun-window-mode-hint"
            role="status"
            aria-live="polite"
          >
            <span className="bitfun-window-mode-hint__title">{windowModeHint.title}</span>
            <span className="bitfun-window-mode-hint__detail">{windowModeHint.detail}</span>
          </div>
        )}

        {/* Main content — always render WorkspaceBody; WelcomeScene in viewport handles no-workspace state */}
        <main className="bitfun-app-main-workspace" data-testid="app-main-content">
          <WorkspaceBody
            onMinimize={canUseNativeWindowControls && !isMacOS ? handleMinimize : undefined}
            onMaximize={canUseNativeWindowControls ? handleMaximize : undefined}
            onClose={canUseNativeWindowControls && !isMacOS ? handleClose : undefined}
            isMaximized={isMaximized}
            isEntering={transitionDir === 'entering'}
            isExiting={transitionDir === 'returning'}
          />
        </main>

        {/* Non-agent scenes: floating mini chat button */}
        {!isWelcomeScene && !isAgentScene && (
          <Suspense fallback={null}>
            <FloatingMiniChat />
          </Suspense>
        )}
      </div>

      {/* Dialogs (previously owned by TitleBar) */}
      {showNewProjectDialog && (
        <Suspense fallback={null}>
          <NewProjectDialog
            isOpen={showNewProjectDialog}
            onClose={() => setShowNewProjectDialog(false)}
            onConfirm={handleConfirmNewProject}
            defaultParentPath={hasWorkspace ? currentWorkspace?.rootPath : undefined}
          />
        </Suspense>
      )}
      {showAboutDialog && (
        <Suspense fallback={null}>
          <AboutDialog
            isOpen={showAboutDialog}
            onClose={() => setShowAboutDialog(false)}
          />
        </Suspense>
      )}
      {showWorkspaceStatus && (
        <Suspense fallback={null}>
          <WorkspaceManager
            isVisible={showWorkspaceStatus}
            onClose={() => setShowWorkspaceStatus(false)}
            onWorkspaceSelect={() => {}}
          />
        </Suspense>
      )}
      <MCPInteractionDialog />
    </>
  );
};

export default AppLayout;
