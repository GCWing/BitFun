import React, { ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createLogger } from '@/shared/utils/logger';
import {
  MAIN_WINDOW_DEFAULT_SIZE,
  MAIN_WINDOW_MIN_SIZE,
  TOOLBAR_COMPACT_MIN,
  TOOLBAR_COMPACT_SIZE,
  TOOLBAR_EXPANDED_MIN,
  TOOLBAR_EXPANDED_SIZE,
  ToolbarModeContext,
  type SavedWindowState,
  type ToolbarModeContextType,
  type ToolbarModeState,
} from './ToolbarModeContext';
import { resolveToolbarWindowGeometry } from './toolbarWindowGeometry';
import {
  createWindowOps,
  type WindowOps,
} from './toolbarModeWindowOps';

const log = createLogger('ToolbarModeContext');

interface ToolbarModeProviderProps {
  children: ReactNode;
}

const isMacOSPlatform = (): boolean =>
  typeof window !== 'undefined' &&
  '__TAURI__' in window &&
  typeof navigator !== 'undefined' &&
  typeof navigator.platform === 'string' &&
  navigator.platform.toUpperCase().includes('MAC');

const setMainWindowTransientGeometry = async (transient: boolean): Promise<void> => {
  const { systemAPI } = await import('@/infrastructure/api/service-api/SystemAPI');
  await systemAPI.setMainWindowTransientGeometry(transient);
};

const restoreMainWindowFromToolbarMode = async (
  ops: WindowOps,
  saved: SavedWindowState | null,
  isMacOS: boolean,
): Promise<void> => {
  // Remove the toolbar constraint before restoring the normal bounds. The
  // standard client minimum is re-applied once restoration completes.
  await ops.setMinSize(null);

  if (isMacOS) {
    try {
      await ops.setTitleBarOverlay();
    } catch (error) {
      log.debug('Failed to restore macOS overlay title bar (early, ignored)', error);
    }
  } else {
    try {
      await ops.setDecorations(saved?.isDecorated ?? false);
    } catch (error) {
      log.debug('Failed to restore window decorations (ignored)', error);
    }
  }

  await Promise.all([
    ops.setResizable(true),
    ops.setSkipTaskbar(false),
  ]);

  if (saved) {
    await ops.setSize({ width: saved.width, height: saved.height });
    await ops.setPosition({ x: saved.x, y: saved.y });

    if (saved.isMaximized) {
      await ops.maximize();
    }
  } else {
    await ops.setSizeLogical({
      width: MAIN_WINDOW_DEFAULT_SIZE.width,
      height: MAIN_WINDOW_DEFAULT_SIZE.height,
    });
    await ops.center();
  }

  await ops.setMinSizeLogical({
    width: MAIN_WINDOW_MIN_SIZE.width,
    height: MAIN_WINDOW_MIN_SIZE.height,
  });

  if (isMacOS) {
    try {
      await ops.setTitleBarOverlay();
      await new Promise<void>((resolve) => setTimeout(resolve, 60));
      await ops.setTitleBarOverlay();
    } catch (error) {
      log.debug('Failed to re-apply macOS overlay title bar (ignored)', error);
    }
  }

  // Keep the native side in transient mode until all normal geometry has been
  // restored. Turning this off persists only the final standard-client state.
  await ops.setAlwaysOnTop(false);
  await setMainWindowTransientGeometry(false);
  await ops.setFocus();
};

export const ToolbarModeProvider: React.FC<ToolbarModeProviderProps> = ({ children }) => {
  const [isToolbarMode, setIsToolbarMode] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [isPinned, setIsPinned] = useState(false);
  const [toolbarState, setToolbarState] = useState<ToolbarModeState>({
    sessionId: null,
    sessionTitle: null,
    isProcessing: false,
    latestContent: '',
    latestToolName: null,
    hasPendingConfirmation: false,
    pendingToolId: null,
    hasError: false,
    todoProgress: null,
  });

  const savedWindowStateRef = useRef<SavedWindowState | null>(null);

  const enableToolbarMode = useCallback(async () => {
    const ops = createWindowOps();
    const isMacOS = isMacOSPlatform();

    try {
      window.dispatchEvent(new CustomEvent('toolbar-mode-activating'));

      const [position, size, isMaximized, isDecorated] = await Promise.all([
        ops.outerPosition(),
        // setSize restores the inner size, so capture the matching metric.
        // Using outerSize here grows decorated windows on every mode round-trip.
        ops.innerSize(),
        ops.isMaximized(),
        ops.isDecorated(),
      ]);

      savedWindowStateRef.current = {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        isMaximized,
        isDecorated,
      };

      await import('./ToolbarMode');

      // Persist the current normal bounds before any compact-window mutation.
      await setMainWindowTransientGeometry(true);
      setIsToolbarMode(true);
      setIsExpanded(true);

      if (isMaximized) {
        await ops.unmaximize();
      }

      const monitor = await ops.currentMonitor();
      const geometry = resolveToolbarWindowGeometry({
        monitor,
        targetSize: TOOLBAR_EXPANDED_SIZE,
        minSize: TOOLBAR_EXPANDED_MIN,
      });
      await ops.setMinSize({ width: geometry.minWidth, height: geometry.minHeight });
      await ops.setAlwaysOnTop(true);

      const toolbarWindowOps: Array<Promise<unknown>> = [
        ops.setSize({ width: geometry.width, height: geometry.height }),
        ops.setPosition({ x: geometry.x, y: geometry.y }),
        ops.setResizable(true),
        ops.setSkipTaskbar(true),
      ];
      if (!isMacOS) {
        toolbarWindowOps.push(ops.setDecorations(false));
      } else {
        try {
          await ops.setTitleBarOverlay();
        } catch {
          // ignore macOS title bar style on platforms without it
        }
      }
      await Promise.all(toolbarWindowOps);
    } catch (error) {
      log.error('Failed to enable toolbar mode', error);
      setIsToolbarMode(false);
      setIsExpanded(false);
      try {
        await restoreMainWindowFromToolbarMode(ops, savedWindowStateRef.current, isMacOS);
        savedWindowStateRef.current = null;
      } catch (restoreError) {
        // The native transient flag intentionally remains active if rollback
        // cannot finish, so a partial floating geometry is never persisted.
        log.error('Failed to restore main window after toolbar mode activation error', restoreError);
      }
    }
  }, []);

  const disableToolbarMode = useCallback(async () => {
    const ops = createWindowOps();
    const isMacOS = isMacOSPlatform();

    try {
      setIsToolbarMode(false);
      setIsExpanded(false);
      await restoreMainWindowFromToolbarMode(ops, savedWindowStateRef.current, isMacOS);
      savedWindowStateRef.current = null;
    } catch (error) {
      log.error('Failed to disable toolbar mode', error);
    }
  }, []);

  const toggleToolbarMode = useCallback(async () => {
    if (isToolbarMode) {
      await disableToolbarMode();
    } else {
      await enableToolbarMode();
    }
  }, [disableToolbarMode, enableToolbarMode, isToolbarMode]);

  const toggleExpanded = useCallback(async () => {
    if (!isToolbarMode) return;

    const newIsExpanded = !isExpanded;

    try {
      const ops = createWindowOps();
      const targetSize = newIsExpanded ? TOOLBAR_EXPANDED_SIZE : TOOLBAR_COMPACT_SIZE;
      const minSize = newIsExpanded ? TOOLBAR_EXPANDED_MIN : TOOLBAR_COMPACT_MIN;
      const currentPosition = await ops.outerPosition();
      const currentSize = await ops.outerSize();
      const monitor = await ops.currentMonitor();
      const geometry = resolveToolbarWindowGeometry({
        monitor,
        targetSize,
        minSize,
        anchor: {
          x: currentPosition.x,
          y: currentPosition.y,
          width: currentSize.width,
          height: currentSize.height,
        },
        fallbackPosition: {
          x: currentPosition.x,
          y: Math.max(0, currentPosition.y + currentSize.height - targetSize.height),
        },
      });
      setIsExpanded(newIsExpanded);

      await ops.setMinSize({ width: geometry.minWidth, height: geometry.minHeight });
      await ops.setSize({ width: geometry.width, height: geometry.height });
      await ops.setPosition({ x: geometry.x, y: geometry.y });
    } catch (error) {
      log.error('Failed to toggle expanded state', { newIsExpanded, error });
    }
  }, [isExpanded, isToolbarMode]);

  const setPinned = useCallback((pinned: boolean) => {
    setIsPinned(pinned);
  }, []);

  const togglePinned = useCallback(() => {
    setIsPinned((prev) => !prev);
  }, []);

  const updateToolbarState = useCallback((updates: Partial<ToolbarModeState>) => {
    setToolbarState((prev) => ({ ...prev, ...updates }));
  }, []);

  useEffect(() => {
    return () => {
      // No background timers to clean up here; window state is restored by user actions.
    };
  }, []);

  const value: ToolbarModeContextType = useMemo(() => ({
    isToolbarMode,
    isExpanded,
    isPinned,
    enableToolbarMode,
    disableToolbarMode,
    toggleToolbarMode,
    toggleExpanded,
    setPinned,
    togglePinned,
    toolbarState,
    updateToolbarState,
  }), [
    isToolbarMode,
    isExpanded,
    isPinned,
    enableToolbarMode,
    disableToolbarMode,
    toggleToolbarMode,
    toggleExpanded,
    setPinned,
    togglePinned,
    toolbarState,
    updateToolbarState,
  ]);

  return <ToolbarModeContext.Provider value={value}>{children}</ToolbarModeContext.Provider>;
};
