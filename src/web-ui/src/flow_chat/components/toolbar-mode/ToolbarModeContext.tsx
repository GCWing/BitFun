/**
 * Toolbar Mode context.
 * Manages global state for the single-window morph behavior.
 *
 * - Full mode: normal main window
 * - Toolbar mode: compact floating bar
 */

import React, { createContext, useContext, useState, useCallback, useRef, ReactNode } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { PhysicalSize, PhysicalPosition } from '@tauri-apps/api/dpi';
import { currentMonitor } from '@tauri-apps/api/window';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ToolbarModeContext');

// Toolbar window state for internal UI rendering.
export interface ToolbarModeState {
  sessionId: string | null;
  sessionTitle: string | null; // Current session title.
  isProcessing: boolean;
  latestContent: string;
  latestToolName: string | null;
  hasPendingConfirmation: boolean;
  pendingToolId: string | null;
  hasError: boolean;
  todoProgress: {
    completed: number;
    total: number;
    current: string;
  } | null;
}

export interface ToolbarModeContextType {
  /** Whether toolbar mode is active. */
  isToolbarMode: boolean;
  /** Whether expanded FlowChat view is active. */
  isExpanded: boolean;
  /** Whether the window is pinned. */
  isPinned: boolean;
  /** Enter toolbar mode. */
  enableToolbarMode: () => Promise<void>;
  /** Exit toolbar mode. */
  disableToolbarMode: () => Promise<void>;
  /** Toggle toolbar mode. */
  toggleToolbarMode: () => Promise<void>;
  /** Toggle expanded/compact view. */
  toggleExpanded: () => Promise<void>;
  /** Set pinned state. */
  setPinned: (pinned: boolean) => void;
  /** Toggle pinned state. */
  togglePinned: () => void;
  /** Toolbar render state. */
  toolbarState: ToolbarModeState;
  /** Update toolbar state. */
  updateToolbarState: (state: Partial<ToolbarModeState>) => void;
}

// Window size config (physical pixels).
// Compact mode uses a fixed two-row layout.
const TOOLBAR_COMPACT_SIZE = { width: 700, height: 140 };
const TOOLBAR_COMPACT_MIN = { width: 400, height: 100 };
// Expanded mode shows the full FlowChat UI.
const TOOLBAR_EXPANDED_SIZE = { width: 700, height: 1400 };
const TOOLBAR_EXPANDED_MIN = { width: 400, height: 500 };

const ToolbarModeContext = createContext<ToolbarModeContextType | undefined>(undefined);

interface ToolbarModeProviderProps {
  children: ReactNode;
}

// Saved window state for restoring full mode.
interface SavedWindowState {
  x: number;
  y: number;
  width: number;
  height: number;
  isMaximized: boolean;
  isDecorated?: boolean;
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
    todoProgress: null
  });
  
  // Persist full-mode window state for restore.
  const savedWindowStateRef = useRef<SavedWindowState | null>(null);
  
  const enableToolbarMode = useCallback(async () => {
    try {
      // Signal all native webview overlays (browser panel/scene) to hide themselves
      // immediately, before any window resize operations, to prevent them from
      // covering the toolbar UI during the transition.
      window.dispatchEvent(new CustomEvent('toolbar-mode-activating'));

      const win = getCurrentWindow();
      const isMacOS =
        typeof window !== 'undefined' &&
        '__TAURI__' in window &&
        typeof navigator !== 'undefined' &&
        typeof navigator.platform === 'string' &&
        navigator.platform.toUpperCase().includes('MAC');
      
      // Capture current window state.
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
      
      // Update state first so React renders the toolbar UI.
      setIsToolbarMode(true);
      setIsExpanded(false); // Enter compact mode by default.
      
      if (isMaximized) {
        await win.unmaximize();
      }
      
      let x = 100;
      let y = 100;
      
      const monitor = await currentMonitor();
      if (monitor) {
        const scaleFactor = await win.scaleFactor();
        const margin = Math.round(20 * scaleFactor);
        const taskbarHeight = Math.round(50 * scaleFactor); // Estimated taskbar height.
        
        x = monitor.size.width - TOOLBAR_COMPACT_SIZE.width - margin;
        y = monitor.size.height - TOOLBAR_COMPACT_SIZE.height - margin - taskbarHeight;
      }
      
      // Apply window props after toolbar UI renders.
      // macOS: avoid decorations toggles to preserve overlay title bar behavior.
      const toolbarWindowOps: Array<Promise<unknown>> = [
        win.setAlwaysOnTop(true),
        win.setSize(new PhysicalSize(TOOLBAR_COMPACT_SIZE.width, TOOLBAR_COMPACT_SIZE.height)),
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
      
      await win.setMinSize(new PhysicalSize(TOOLBAR_COMPACT_MIN.width, TOOLBAR_COMPACT_MIN.height));
      
    } catch (error) {
      log.error('Failed to enable toolbar mode', error);
      setIsToolbarMode(false);
    }
  }, []);
  
  const disableToolbarMode = useCallback(async () => {
    try {
      // Update state first so React renders the full UI.
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
        win.setSkipTaskbar(false)
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

      // macOS: re-apply overlay after resize/maximize in case it was interrupted.
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
  }, [isToolbarMode, enableToolbarMode, disableToolbarMode]);
  
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
  }, [isToolbarMode, isExpanded]);
  
  const setPinned = useCallback((pinned: boolean) => {
    setIsPinned(pinned);
  }, []);
  
  const togglePinned = useCallback(() => {
    setIsPinned(prev => !prev);
  }, []);
  
  const updateToolbarState = useCallback((updates: Partial<ToolbarModeState>) => {
    setToolbarState(prev => ({ ...prev, ...updates }));
  }, []);
  
  const value: ToolbarModeContextType = {
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
    updateToolbarState
  };
  
  return (
    <ToolbarModeContext.Provider value={value}>
      {children}
    </ToolbarModeContext.Provider>
  );
};

// Default values for calls outside the provider.
const defaultContextValue: ToolbarModeContextType = {
  isToolbarMode: false,
  isExpanded: false,
  isPinned: false,
  enableToolbarMode: async () => { log.warn('Provider not found'); },
  disableToolbarMode: async () => { log.warn('Provider not found'); },
  toggleToolbarMode: async () => { log.warn('Provider not found'); },
  toggleExpanded: async () => { log.warn('Provider not found'); },
  setPinned: () => { log.warn('Provider not found'); },
  togglePinned: () => { log.warn('Provider not found'); },
  toolbarState: {
    sessionId: null,
    sessionTitle: null,
    isProcessing: false,
    latestContent: '',
    latestToolName: null,
    hasPendingConfirmation: false,
    pendingToolId: null,
    hasError: false,
    todoProgress: null
  },
  updateToolbarState: () => { log.warn('Provider not found'); }
};

export const useToolbarModeContext = (): ToolbarModeContextType => {
  const context = useContext(ToolbarModeContext);
  if (!context) {
    log.warn('useToolbarModeContext called outside of ToolbarModeProvider, using default values');
    return defaultContextValue;
  }
  return context;
};

export default ToolbarModeContext;
