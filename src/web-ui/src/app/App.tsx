import { lazy, Suspense, useEffect, useCallback, useState, useRef } from 'react';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { useHasDismissibleLayer } from '@/infrastructure/hooks/useDismissibleLayer';
import { dismissibleLayerManager } from '@/infrastructure/services/DismissibleLayerManager';
import { ChatProvider } from '../infrastructure/contexts/ChatProvider';
import { ViewModeProvider } from '../infrastructure/contexts/ViewModeProvider';
import { SSHRemoteProvider } from '../features/ssh-remote';
import { ContextMenuRenderer } from '../shared/context-menu-system/components/ContextMenuRenderer';
import { NotificationContainer, NotificationCenter, notificationService } from '../shared/notification-system';
import { AnnouncementProvider } from '../shared/announcement-system';
import { ConfirmDialogRenderer } from '../component-library';
import { createLogger } from '@/shared/utils/logger';
import { startupTrace } from '@/shared/utils/startupTrace';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { useWorkspaceContext } from '../infrastructure/contexts/WorkspaceContext';
import { useGlobalSceneShortcuts } from './hooks/useGlobalSceneShortcuts';
import { useDebugInspector } from '@/infrastructure/debug/useDebugInspector';
import { useI18n } from '@/infrastructure/i18n';
import { scheduleDeferredStartupSystems } from './startup/deferredStartupSystems';
import {
  getStartupOverlayElapsedMs,
  hideStartupOverlay,
  isStartupOverlayPresent,
} from './startup/startupOverlay';
import { ToolbarModeProvider } from '../flow_chat/components/toolbar-mode';

const log = createLogger('App');

function isBackgroundTaskCancelledError(error: unknown): boolean {
  return error instanceof Error && error.name === 'BackgroundTaskCancelledError';
}

interface AppLayoutStartupGateProps {
  onReady: () => void;
}

const LazyAppLayout = lazy(async () => {
  startupTrace.markPhase('app_layout_import_start');
  try {
    const module = await import('./layout/AppLayout');
    startupTrace.markPhase('app_layout_import_end');
    return {
      default: function AppLayoutStartupGate({ onReady }: AppLayoutStartupGateProps) {
        useEffect(() => {
          startupTrace.markPhase('app_layout_ready');
          onReady();
        }, [onReady]);

        return <module.default />;
      },
    };
  } catch (error) {
    startupTrace.markPhase('app_layout_import_failed');
    throw error;
  }
});

/**
 * BitFun main application component.
 *
 * Unified architecture:
 * - Use a single AppLayout component
 * - AppLayout switches content based on workspace presence
 * - Without a workspace: show startup content (branding + actions)
 * - With a workspace: show workspace panels
 * - Header is always present; elements toggle by state
 */
// Minimum time (ms) the splash is shown, so the animation is never a flash.
const MIN_SPLASH_MS = 900;

function App() {
  const { t } = useI18n('settings/basics');

  // Workspace loading state — drives splash exit timing
  const { loading: workspaceLoading } = useWorkspaceContext();

  const [startupOverlayVisible, setStartupOverlayVisible] = useState(isStartupOverlayPresent);
  const hasAppDismissibleLayer = useHasDismissibleLayer('app');
  const mainWindowShownRef = useRef(false);
  const interactiveShellReadyRef = useRef(false);
  const [interactiveShellReady, setInteractiveShellReady] = useState(false);
  const [appLayoutReady, setAppLayoutReady] = useState(false);
  const handleAppLayoutReady = useCallback(() => {
    startupTrace.markPhase('app_layout_ready_state_update_requested');
    setAppLayoutReady(true);
  }, []);

  // Once the workspace finishes loading, wait for the remaining min-display
  // time and then begin the exit animation.
  useEffect(() => {
    if (workspaceLoading || !appLayoutReady) return;
    const elapsed = getStartupOverlayElapsedMs();
    const remaining = Math.max(0, MIN_SPLASH_MS - elapsed);
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void hideStartupOverlay().then(() => {
        if (!cancelled) {
          setStartupOverlayVisible(false);
        }
      });
    }, remaining);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [workspaceLoading, appLayoutReady]);

  const showMainWindow = useCallback(async (reason: string) => {
    if (mainWindowShownRef.current) {
      return;
    }
    mainWindowShownRef.current = true;

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('show_main_window');
      log.debug('Main window shown', { reason });
      startupTrace.markPhase('main_window_shown', { reason });
      window.dispatchEvent(new CustomEvent('bitfun:main-window-shown', { detail: { reason } }));
    } catch (error: any) {
      log.error('Failed to show main window', error);

      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const mainWindow = getCurrentWindow();
        await mainWindow.show();
        await mainWindow.setFocus();
        log.debug('Main window shown via fallback', { reason });
        startupTrace.markPhase('main_window_shown_fallback', { reason });
        window.dispatchEvent(new CustomEvent('bitfun:main-window-shown', { detail: { reason } }));
      } catch (fallbackError) {
        log.error('Fallback window show failed', fallbackError);
        mainWindowShownRef.current = false;
      }
    }
  }, []);

  const verifyMainWindowVisible = useCallback(async (reason: string) => {
    if (!isTauriRuntime()) {
      void showMainWindow(reason);
      return;
    }

    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const mainWindow = getCurrentWindow();
      if (await mainWindow.isVisible()) {
        return;
      }

      log.warn('Main window is not visible after native startup show, retrying', { reason });
      mainWindowShownRef.current = false;
      await showMainWindow(reason);
    } catch (error) {
      log.warn('Failed to verify main window visibility after native startup show', { reason, error });
    }
  }, [showMainWindow]);

  // Desktop shows the startup splash from the native window creation path.
  // Mark it here so deferred work can wait until the first visible shell exists.
  useEffect(() => {
    startupTrace.markPhase('app_effect_mounted');
    if (isTauriRuntime()) {
      mainWindowShownRef.current = true;
      startupTrace.markPhase('main_window_shown', { reason: 'startup-native' });
      window.dispatchEvent(new CustomEvent('bitfun:main-window-shown', {
        detail: { reason: 'startup-native' },
      }));
      return;
    }
    void showMainWindow('startup-overlay');
  }, [showMainWindow]);

  useEffect(() => {
    startupTrace.markPhase('interactive_shell_ready_gate_check', {
      workspaceLoading,
      appLayoutReady,
      alreadyReady: interactiveShellReadyRef.current,
    });
    if (workspaceLoading || !appLayoutReady || interactiveShellReadyRef.current) {
      return;
    }
    interactiveShellReadyRef.current = true;
    startupTrace.markPhase('interactive_shell_ready');
    window.dispatchEvent(new CustomEvent('bitfun:interactive-shell-ready', {
      detail: { reason: 'workspace-and-layout-ready' },
    }));
    setInteractiveShellReady(true);
  }, [workspaceLoading, appLayoutReady]);

  // If the early reveal path fails, keep the old post-splash show as a retry.
  useEffect(() => {
    if (startupOverlayVisible) {
      return;
    }

    const timer = window.setTimeout(() => {
      void verifyMainWindowVisible('startup-complete');
    }, 50);

    return () => window.clearTimeout(timer);
  }, [startupOverlayVisible, verifyMainWindowVisible]);

  // Safety net: if startup gets stuck, reveal the window so the user can see errors.
  useEffect(() => {
    const timer = window.setTimeout(() => {
      void verifyMainWindowVisible('startup-watchdog');
    }, 10000);

    return () => window.clearTimeout(timer);
  }, [verifyMainWindowVisible]);

  // Non-critical systems are delayed until the shell is interactive.
  useEffect(() => {
    if (!interactiveShellReady) {
      return;
    }

    log.info('Application interactive, scheduling deferred systems');
    const startupSystemsHandle = scheduleDeferredStartupSystems();
    startupSystemsHandle.promise.catch(error => {
      if (!isBackgroundTaskCancelledError(error)) {
        log.warn('Deferred startup systems task failed', error);
      }
    });

    return () => startupSystemsHandle.cancel();
  }, [interactiveShellReady]);

  useEffect(() => {
    if (!interactiveShellReady || startupOverlayVisible) {
      return;
    }

    let disposed = false;
    let editorWarmupHandle: { promise: Promise<void>; cancel: () => void } | null = null;

    void import('@/tools/editor/services/MonacoStartupWarmup')
      .then(({ scheduleMonacoStartupWarmup }) => {
        if (disposed) {
          return;
        }
        editorWarmupHandle = scheduleMonacoStartupWarmup();
        editorWarmupHandle.promise.catch(error => {
          if (!disposed && !isBackgroundTaskCancelledError(error)) {
            log.warn('Editor startup warmup task failed', error);
          }
        });
      })
      .catch(error => {
        if (!disposed) {
          log.warn('Failed to schedule editor startup warmup', error);
        }
      });

    return () => {
      disposed = true;
      editorWarmupHandle?.cancel();
    };
  }, [interactiveShellReady, startupOverlayVisible]);

  useEffect(() => {
    if (!isTauriRuntime() || !interactiveShellReady) return;

    let disposed = false;
    let startupSyncHandle: { promise: Promise<void>; cancel: () => void } | null = null;
    let removeSettingsListener: (() => void) | null = null;

    void (async () => {
      const [
        { aiExperienceConfigService },
        { syncAgentCompanionDesktopWindow },
        { buildAgentCompanionActivity },
        { emitAgentCompanionActivity },
        { backgroundTaskScheduler },
      ] = await Promise.all([
        import('@/infrastructure/config/services/AIExperienceConfigService'),
        import('@/infrastructure/config/services/AgentCompanionWindowService'),
        import('@/flow_chat/utils/agentCompanionActivity'),
        import('@/flow_chat/services/AgentCompanionActivityBridge'),
        import('@/shared/utils/backgroundTaskScheduler'),
      ]);

      if (disposed) {
        return;
      }

      const emitCurrentAgentCompanionActivity = () => {
        if (disposed) {
          return;
        }
        void emitAgentCompanionActivity(buildAgentCompanionActivity());
      };

      const settings = await aiExperienceConfigService.getSettingsAsync();
      if (disposed) {
        return;
      }

      startupTrace.markPhase('agent_companion_sync_scheduled', {
        source: 'startup_idle',
      });
      startupSyncHandle = backgroundTaskScheduler.schedule(async signal => {
        if (signal.aborted || disposed) {
          return;
        }
        startupTrace.markPhase('agent_companion_sync_start', {
          source: 'startup_idle',
        });
        await syncAgentCompanionDesktopWindow(settings);
        if (signal.aborted || disposed) {
          return;
        }
        emitCurrentAgentCompanionActivity();
        window.setTimeout(emitCurrentAgentCompanionActivity, 250);
        startupTrace.markPhase('agent_companion_sync_end', {
          source: 'startup_idle',
        });
      }, {
        idle: true,
        inFlightKey: 'agent-companion:startup-sync',
        priority: 'low',
      });

      startupSyncHandle.promise.catch(error => {
        if (!disposed && !isBackgroundTaskCancelledError(error)) {
          log.warn('Initial Agent companion sync task failed', error);
        }
      });

      removeSettingsListener = aiExperienceConfigService.addChangeListener(settings => {
        void syncAgentCompanionDesktopWindow(settings).then(() => {
          emitCurrentAgentCompanionActivity();
          window.setTimeout(emitCurrentAgentCompanionActivity, 250);
        });
      });
    })().catch(error => {
      if (!disposed) {
        log.warn('Failed to initialize Agent companion startup sync', error);
      }
    });

    return () => {
      disposed = true;
      startupSyncHandle?.cancel();
      removeSettingsListener?.();
    };
  }, [interactiveShellReady]);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | null = null;

    void Promise.all([
      import('@/flow_chat/utils/agentCompanionActivity'),
      import('@/flow_chat/services/AgentCompanionActivityBridge'),
    ])
      .then(([{ subscribeAgentCompanionActivity }, { emitAgentCompanionActivity }]) => {
        if (disposed) {
          return;
        }
        unsubscribe = subscribeAgentCompanionActivity(activity => {
          void emitAgentCompanionActivity(activity);
        });
      })
      .catch(error => {
        if (!disposed) {
          log.warn('Failed to subscribe Agent companion activity bridge', error);
        }
      });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void import('@tauri-apps/api/event')
      .then(({ listen }) => listen<{ sessionId?: string }>(
        'agent-companion://open-session',
        async event => {
          const sessionId = event.payload?.sessionId;
          if (!sessionId) return;

          const { openAgentCompanionSession } = await import('./services/openAgentCompanionSession');
          await openAgentCompanionSession(sessionId);

          try {
            const { invoke } = await import('@tauri-apps/api/core');
            await invoke('show_main_window');
          } catch (error) {
            log.warn('Failed to show main window from Agent companion bubble', {
              sessionId,
              error,
            });
          }
        },
      ))
      .then(removeListener => {
        unlisten = removeListener;
      })
      .catch(error => {
        log.warn('Failed to listen for Agent companion session open events', error);
      });

    return () => {
      unlisten?.();
    };
  }, []);

  // Block browser-native Ctrl+F (find bar) and Ctrl+R (hard reload).
  // On macOS the equivalent modifiers are Cmd+F / Cmd+R.
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const primary = e.ctrlKey || e.metaKey;
      if (!primary) return;
      const key = e.key.toLowerCase();
      if (key === 'f' || key === 'r') {
        e.preventDefault();
        e.stopPropagation();
      }
    };
    window.addEventListener('keydown', handleKeyDown, { capture: true });
    return () => window.removeEventListener('keydown', handleKeyDown, { capture: true });
  }, []);

  // Escape closes preview overlay (registered via ShortcutManager)
  useShortcut(
    'app.closePreview',
    { key: 'Escape', scope: 'app', allowInInput: true },
    () => {
      dismissibleLayerManager.dismissTop('app');
    },
    {
      enabled: hasAppDismissibleLayer,
      priority: 1,
      description: 'keyboard.shortcuts.app.closePreview',
    }
  );

  // Top SceneBar: Mod+Alt+1..9 / Mod+Alt+PageUp/PageDown
  useGlobalSceneShortcuts();

  // Debug inspector shortcuts (desktop devtools only)
  useDebugInspector();

  useEffect(() => {
    if (!isTauriRuntime() || !interactiveShellReady) {
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const { configAPI, workspaceAPI } = await import('@/infrastructure/api');
        const runtimeInfo = await configAPI.getRuntimeLoggingInfo();
        if (cancelled || !runtimeInfo.previousUnexpectedExit?.detected) {
          return;
        }
        const recoveryKey = `bitfun:unexpected-exit-notice:${runtimeInfo.previousUnexpectedExit.sessionLogDir || 'unknown'}`;
        if (sessionStorage.getItem(recoveryKey) === 'shown') {
          return;
        }
        sessionStorage.setItem(recoveryKey, 'shown');

        notificationService.warning(t('logging.startupRecovery.message'), {
          title: t('logging.startupRecovery.title'),
          duration: 0,
          actions: [
            {
              label: t('logging.actions.exportDiagnostics'),
              variant: 'primary',
              onClick: () => {
                void (async () => {
                  try {
                    const result = await configAPI.exportDiagnosticsBundle();
                    notificationService.success(t('logging.messages.diagnosticsExported'), { duration: 3000 });
                    await workspaceAPI.revealInExplorer(result.bundlePath);
                  } catch (error) {
                    log.error('Failed to export diagnostics bundle from startup notification', error);
                    notificationService.error(t('logging.messages.diagnosticsExportFailed'), { duration: 5000 });
                  }
                })();
              },
            },
            {
              label: t('logging.actions.openLoggingSettings'),
              onClick: () => {
                void import('@/shared/services/ide-control').then(({ quickActions }) => {
                  quickActions.openSettings('basics');
                });
              },
            },
          ],
          metadata: {
            source: 'startup-crash-diagnostics',
            sessionLogDir: runtimeInfo.previousUnexpectedExit.sessionLogDir,
            crashReportPath: runtimeInfo.previousUnexpectedExit.crashReportPath,
          },
        });
      } catch (error) {
        log.warn('Failed to check previous unexpected exit status', error);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [interactiveShellReady, t]);

  // Unified layout via a single AppLayout
  return (
    <ChatProvider>
      <ViewModeProvider defaultMode="coder">
        <SSHRemoteProvider>
          <ToolbarModeProvider>
            {/* Unified app layout with startup/workspace modes */}
            <Suspense fallback={null}>
              <LazyAppLayout onReady={handleAppLayoutReady} />
            </Suspense>

            {/* Context menu renderer */}
            <ContextMenuRenderer />

            {/* Notification system */}
            <NotificationContainer />
            <NotificationCenter />

            {/* Confirm dialog */}
            <ConfirmDialogRenderer />

            {/* Announcement / feature-demo / tips system */}
            <AnnouncementProvider />

          </ToolbarModeProvider>
        </SSHRemoteProvider>
      </ViewModeProvider>
    </ChatProvider>
  );
}

export default App;
