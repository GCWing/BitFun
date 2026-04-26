import { useCallback, useRef, useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useWorkspaceContext } from '../../infrastructure/contexts/WorkspaceContext';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { sendDebugProbe } from '@/shared/utils/debugProbe';
import { nowMs } from '@/shared/utils/timing';
import { useI18n } from '@/infrastructure/i18n';
import { isMacOSDesktopRuntime, supportsNativeWindowControls } from '@/infrastructure/runtime';

const log = createLogger('useWindowControls');

const formatErrorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

/**
 * Window controls hook.
 * Manages minimize, maximize, close, and related actions.
 */
export const useWindowControls = (options?: { isToolbarMode?: boolean }) => {
  const { t } = useI18n('errors');
  const isToolbarMode = options?.isToolbarMode ?? false;
  const canUseNativeWindowControls = supportsNativeWindowControls();
  const { hasWorkspace, closeWorkspace } = useWorkspaceContext();
  
  // Maximized state
  const [isMaximized, setIsMaximized] = useState(false);
  
  // Debounce guard to prevent rapid toggles
  const isMaximizeInProgress = useRef(false);
  
  // Skip state updates during manual operations
  const shouldSkipStateUpdate = useRef(false);

  // Listen for window state changes
  useEffect(() => {
    if (!canUseNativeWindowControls) return;

    let unlistenResized: (() => void) | undefined;
    
    // Debounce timer
    let resizeTimer: NodeJS.Timeout | null = null;

    const isMacOSDesktop = isMacOSDesktopRuntime();

    const restoreMacOSOverlayTitlebar = async (appWindow: any) => {
      if (!isMacOSDesktop || isToolbarMode) return;
      try {
        if (typeof appWindow.setTitleBarStyle === 'function') {
          await appWindow.setTitleBarStyle('overlay');
        }
      } catch {
        // Ignore failures during window animation/state changes
      }
    };
    
    // Helper to update window state (minimize API calls)
    const updateWindowState = async (appWindow: any, skipVisibilityCheck = false) => {
      // Skip auto updates while maximizing to avoid duplicates
      if (shouldSkipStateUpdate.current) {
        return;
      }
      
      try {
        // Skip visibility check when not required
        if (skipVisibilityCheck) {
          const maximized = await appWindow.isMaximized();
          setIsMaximized(maximized);
          return;
        }
        
        // Skip if window is not visible (minimized)
        const isVisible = await appWindow.isVisible();
        if (!isVisible) {
          return;
        }
        
        const maximized = await appWindow.isMaximized();
        setIsMaximized(maximized);
      } catch (_error) {
        // Ignore errors to avoid noise when minimized
      }
    };
    
    // Update state when window regains focus.
    // Note: Tauri may not expose onFocus; use page visibility as a fallback.
    const handleVisibilityChange = async () => {
      // Skip visibility handling while maximizing
      if (shouldSkipStateUpdate.current) {
        return;
      }
      
      if (document.visibilityState === 'visible') {
        sendDebugProbe(
          'useWindowControls.ts:handleVisibilityChange',
          'Window became visible',
          {
            isToolbarMode,
          }
        );
        try {
          const appWindow = getCurrentWindow();
          // Delay update until window fully restores
          setTimeout(async () => {
            const startedAt = nowMs();
            try {
              await updateWindowState(appWindow);
              await restoreMacOSOverlayTitlebar(appWindow);
              sendDebugProbe(
                'useWindowControls.ts:handleVisibilityChange',
                'Window restore sync completed',
                {
                  isToolbarMode,
                },
                { startedAt }
              );
            } catch (error) {
              sendDebugProbe(
                'useWindowControls.ts:handleVisibilityChange',
                'Window restore sync failed',
                {
                  error: formatErrorMessage(error),
                  isToolbarMode,
                }
              );
            }
          }, 300);
        } catch (error) {
          sendDebugProbe(
            'useWindowControls.ts:handleVisibilityChange',
            'Window restore setup failed',
            {
              error: formatErrorMessage(error),
              isToolbarMode,
            }
          );
        }
      }
    };
    
    const setupListener = async () => {
      try {
        const appWindow = getCurrentWindow();

        // Get initial state (skip visibility check so we still sync
        // when the window is maximized before it becomes visible)
        await updateWindowState(appWindow, true);
        await restoreMacOSOverlayTitlebar(appWindow);
        
        // Listen for resize (with debounce and visibility checks)
        unlistenResized = await appWindow.onResized(async () => {
          // Skip resize handling while maximizing
          if (shouldSkipStateUpdate.current) {
            return;
          }
          
          // Clear previous timer
          if (resizeTimer) {
            clearTimeout(resizeTimer);
          }
          
          // Debounce: delay to avoid frequent calls (300ms covers maximize/restore)
          resizeTimer = setTimeout(async () => {
            await updateWindowState(appWindow);
            await restoreMacOSOverlayTitlebar(appWindow);
          }, 300); // 300ms debounce covers window change duration
        });
        
        // Add page visibility listener
        document.addEventListener('visibilitychange', handleVisibilityChange);
      } catch (error) {
        log.error('Failed to setup window state listener', error);
      }
    };
    
    setupListener();
    
    return () => {
      if (resizeTimer) {
        clearTimeout(resizeTimer);
      }
      if (unlistenResized) {
        unlistenResized();
      }
      // Remove page visibility listener
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, [canUseNativeWindowControls, isToolbarMode]);

  // Window control handlers
  const handleMinimize = useCallback(async () => {
    if (!canUseNativeWindowControls) return;

    // Save active element to restore focus after window restore
    const activeElement = document.activeElement as HTMLElement;
    const wasInputFocused = activeElement && (
      activeElement.classList.contains('rich-text-input') ||
      activeElement.closest('.rich-text-input') !== null ||
      activeElement.isContentEditable
    );
    
    try {
      const appWindow = getCurrentWindow();
      await appWindow.minimize();
      
      // Ensure input is usable after restore
      // Listen for restore
      const handleWindowRestore = async () => {
        setTimeout(() => {
          // Ensure contentEditable is set correctly
          const chatInputs = document.querySelectorAll('.rich-text-input[contenteditable]');
          chatInputs.forEach((input) => {
            const element = input as HTMLElement;
            if (element.getAttribute('contenteditable') !== 'true') {
              element.setAttribute('contenteditable', 'true');
            }
          });
          
          // Restore focus if input was focused
          if (wasInputFocused && activeElement && activeElement.isConnected) {
            try {
              const rect = activeElement.getBoundingClientRect();
              if (rect.width > 0 && rect.height > 0) {
                activeElement.focus();
              }
            } catch (_error) {
              // Ignore focus restore failures
            }
          }
        }, 100);
        
        // Run once
        window.removeEventListener('focus', handleWindowRestore);
      };
      
      // Listen for restore
      window.addEventListener('focus', handleWindowRestore, { once: true });
    } catch (error) {
      log.error('Failed to minimize window', error);
      // Avoid error toast when minimized to prevent UI blockage
    }
  }, [canUseNativeWindowControls]);

  const handleMaximize = useCallback(async () => {
    if (!canUseNativeWindowControls) return;

    // Debounce: ignore while in progress
    if (isMaximizeInProgress.current) {
      return;
    }
    
    // Save active element to restore focus after window change
    const activeElement = document.activeElement as HTMLElement;
    const wasInputFocused = activeElement && (
      activeElement.classList.contains('rich-text-input') ||
      activeElement.closest('.rich-text-input') !== null ||
      activeElement.isContentEditable
    );
    
    try {
      isMaximizeInProgress.current = true;
      // Skip auto updates to avoid duplicate state changes
      shouldSkipStateUpdate.current = true;
      
      const appWindow = getCurrentWindow();
      
      // Optimization: skip isVisible check; query maximized directly.
      // If minimized, user restores via taskbar instead of double-clicking header.
      // Check current state to avoid duplicate toggles.
      let currentMaximized = false;
      try {
        currentMaximized = await appWindow.isMaximized();
      } catch (error) {
        log.warn('Failed to get maximized state, assuming not maximized', error);
        currentMaximized = false;
      }
      // Use requestAnimationFrame to avoid blocking UI updates
      const updateState = (newState: boolean) => {
        requestAnimationFrame(() => {
          setIsMaximized(newState);
        });
      };
      
      // Toggle maximize/restore
      if (currentMaximized) {
        await appWindow.unmaximize();
        updateState(false);
      } else {
        await appWindow.maximize();
        updateState(true);
      }
      
      // Delay DOM work to avoid blocking UI rendering
      requestAnimationFrame(() => {
        setTimeout(() => {
          // Ensure contentEditable is set correctly
          const chatInputs = document.querySelectorAll('.rich-text-input[contenteditable]');
          chatInputs.forEach((input) => {
            const element = input as HTMLElement;
            // Ensure contentEditable is set correctly
            if (element.getAttribute('contenteditable') !== 'true') {
              element.setAttribute('contenteditable', 'true');
            }
          });
          
          // Restore focus if input was focused (best-effort)
          if (wasInputFocused && activeElement && activeElement.isConnected) {
            try {
              // Restore only if element is still present and visible
              const rect = activeElement.getBoundingClientRect();
              if (rect.width > 0 && rect.height > 0) {
                activeElement.focus();
              }
            } catch (_error) {
              // Ignore focus restore failures
            }
          }
        }, 50); // Reduced delay from 100ms to 50ms
      });
    } catch (error) {
      log.error('Failed to toggle maximize window', error);
      notificationService.error(t('window.maximizeFailed', { error: formatErrorMessage(error) }));
    } finally {
      // Reduce final delay: 200ms is sufficient for window updates
      setTimeout(() => {
        isMaximizeInProgress.current = false;
        shouldSkipStateUpdate.current = false;
      }, 200);
    }
  }, [canUseNativeWindowControls, t]);

  const handleClose = useCallback(async () => {
    if (!canUseNativeWindowControls) return;

    try {
      const appWindow = getCurrentWindow();
      await appWindow.close();
    } catch (error) {
      log.error('Failed to close window', error);
      notificationService.error(t('window.closeFailed', { error: formatErrorMessage(error) }));
    }
  }, [canUseNativeWindowControls, t]);

  // Home button: reset to startup page
  const handleHomeClick = useCallback(async () => {
    try {
      // 1) Close current workspace (triggers state update)
      if (hasWorkspace) {
        await closeWorkspace();
      }
      
      // 2) Dispatch preview close event
      window.dispatchEvent(new CustomEvent('closePreview'));
    } catch (error) {
      log.error('Failed to return to startup page', error);
    }
  }, [hasWorkspace, closeWorkspace]);

  return {
    handleMinimize,
    handleMaximize,
    handleClose,
    handleHomeClick,
    isMaximized,
    canUseNativeWindowControls
  };
};
