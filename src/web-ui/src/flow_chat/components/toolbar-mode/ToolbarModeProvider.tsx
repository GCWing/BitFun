import React, { ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { currentMonitor, getCurrentWindow } from '@tauri-apps/api/window';
import { PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi';
import { createLogger } from '@/shared/utils/logger';
import {
  TOOLBAR_COMPACT_MIN,
  TOOLBAR_COMPACT_SIZE,
  TOOLBAR_EXPANDED_MIN,
  TOOLBAR_EXPANDED_SIZE,
  ToolbarModeContext,
  type SavedWindowState,
  type ToolbarModeContextType,
  type ToolbarModeState,
} from './ToolbarModeContext';

const log = createLogger('ToolbarModeContext');

interface ToolbarModeProviderProps {
  children: ReactNode;
}

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
    try {
      window.dispatchEvent(new CustomEvent('toolbar-mode-activating'));

      const win = getCurrentWindow();
      const isMacOS =
        typeof window !== 'undefined' &&
        '__TAURI__' in window &&
        typeof navigator !== 'undefined' &&
        typeof navigator.platform === 'string' &&
        navigator.platform.toUpperCase().includes('MAC');

      const [position, size, isMaximized, isDecorated] = await Promise.all([
        win.outerPosition(),
        win.outerSize(),
        win.isMaximized(),
        (async () => {
          try {
            if (typeof (win as any).isDecorated === 'function') {
              return await (win as any).isDecorated();
            }
          } catch {
          }
          return undefined;
        })(),
      ]);

      savedWindowStateRef.current = {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        isMaximized,
        isDecorated,
      };

      setIsToolbarMode(true);
      setIsExpanded(true);

      if (isMaximized) {
        await win.unmaximize();
      }

      let x = 100;
      let y = 100;

      const monitor = await currentMonitor();
      if (monitor) {
        const scaleFactor = await win.scaleFactor();
        const margin = Math.round(20 * scaleFactor);
        const taskbarHeight = Math.round(50 * scaleFactor);

        x = monitor.size.width - TOOLBAR_EXPANDED_SIZE.width - margin;
        y = monitor.size.height - TOOLBAR_EXPANDED_SIZE.height - margin - taskbarHeight;
      }

      const toolbarWindowOps: Array<Promise<unknown>> = [
        win.setAlwaysOnTop(true),
        win.setSize(new PhysicalSize(TOOLBAR_EXPANDED_SIZE.width, TOOLBAR_EXPANDED_SIZE.height)),
        win.setPosition(new PhysicalPosition(x, y)),
        win.setResizable(true),
        win.setSkipTaskbar(true),
      ];
      if (!isMacOS) {
        toolbarWindowOps.push(win.setDecorations(false));
      } else {
        try {
          await win.setTitleBarStyle('overlay');
        } catch {
        }
      }
      await Promise.all(toolbarWindowOps);

      await win.setMinSize(new PhysicalSize(TOOLBAR_EXPANDED_MIN.width, TOOLBAR_EXPANDED_MIN.height));
    } catch (error) {
      log.error('Failed to enable toolbar mode', error);
      setIsToolbarMode(false);
    }
  }, []);

  const disableToolbarMode = useCallback(async () => {
    try {
      setIsToolbarMode(false);
      setIsExpanded(false);

      const win = getCurrentWindow();
      const isMacOS =
        typeof window !== 'undefined' &&
        '__TAURI__' in window &&
        typeof navigator !== 'undefined' &&
        typeof navigator.platform === 'string' &&
        navigator.platform.toUpperCase().includes('MAC');
      const saved = savedWindowStateRef.current;

      await win.setMinSize(null);

      if (isMacOS) {
        try {
          await win.setTitleBarStyle('overlay');
        } catch (error) {
          log.debug('Failed to restore macOS overlay title bar (early, ignored)', error);
        }
      } else {
        try {
          const targetDecorations = saved?.isDecorated ?? false;
          await win.setDecorations(targetDecorations);
        } catch (error) {
          log.debug('Failed to restore window decorations (ignored)', error);
        }
      }

      await Promise.all([
        win.setAlwaysOnTop(false),
        win.setResizable(true),
        win.setSkipTaskbar(false),
      ]);

      if (saved) {
        await win.setSize(new PhysicalSize(saved.width, saved.height));
        await win.setPosition(new PhysicalPosition(saved.x, saved.y));

        if (saved.isMaximized) {
          await win.maximize();
        }
      } else {
        await win.setSize(new PhysicalSize(1200, 800));
        await win.center();
      }

      if (isMacOS) {
        try {
          await win.setTitleBarStyle('overlay');
          await new Promise<void>((resolve) => setTimeout(resolve, 60));
          await win.setTitleBarStyle('overlay');
        } catch (error) {
          log.debug('Failed to re-apply macOS overlay title bar (ignored)', error);
        }
      }

      await win.setFocus();
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
      const win = getCurrentWindow();
      const targetSize = newIsExpanded ? TOOLBAR_EXPANDED_SIZE : TOOLBAR_COMPACT_SIZE;
      const minSize = newIsExpanded ? TOOLBAR_EXPANDED_MIN : TOOLBAR_COMPACT_MIN;
      const currentPosition = await win.outerPosition();
      const currentSize = await win.outerSize();
      const heightDiff = targetSize.height - currentSize.height;
      const newY = currentPosition.y - heightDiff;

      setIsExpanded(newIsExpanded);

      await win.setMinSize(new PhysicalSize(minSize.width, minSize.height));
      await win.setSize(new PhysicalSize(targetSize.width, targetSize.height));
      await win.setPosition(new PhysicalPosition(currentPosition.x, Math.max(0, newY)));
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
