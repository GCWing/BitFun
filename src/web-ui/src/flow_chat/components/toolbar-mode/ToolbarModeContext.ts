/**
 * Toolbar Mode context.
 * Manages global state for the single-window morph behavior.
 *
 * - Full mode: normal main window
 * - Toolbar mode: compact floating bar
 */
import { createContext, useContext } from 'react';
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

export const TOOLBAR_COMPACT_SIZE = { width: 700, height: 140 };
export const TOOLBAR_COMPACT_MIN = { width: 400, height: 100 };
export const TOOLBAR_EXPANDED_SIZE = { width: 700, height: 1400 };
export const TOOLBAR_EXPANDED_MIN = { width: 400, height: 500 };

export const ToolbarModeContext = createContext<ToolbarModeContextType | undefined>(undefined);

// Saved window state for restoring full mode.
export interface SavedWindowState {
  x: number;
  y: number;
  width: number;
  height: number;
  isMaximized: boolean;
  isDecorated?: boolean;
}

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
