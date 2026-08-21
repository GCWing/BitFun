/**
 * Application hook.
 * Provides unified app state management and actions.
 */

import { useState, useEffect, useCallback, useSyncExternalStore } from 'react';
import {
  UseAppReturn,
  AppState,
  AgentConfig,
  ChatSession,
  TabInfo,
  PanelType
} from '../types';
import { appManager } from '../services/AppManager';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useApp');

export const useApp = (): UseAppReturn => {
  const [state, setState] = useState<AppState>(appManager.getState());

  // Listen for app state changes
  useEffect(() => {
    const unsubscribe = appManager.addEventListener(() => {
      // Update state on each event
      setState(appManager.getState());
    });

    // Sync state on initialization
    setState(appManager.getState());

    return unsubscribe;
  }, []);

  // Layout actions
  const toggleLeftPanel = useCallback(() => {
    appManager.updateLayout({
      leftPanelCollapsed: !state.layout.leftPanelCollapsed
    });
  }, [state.layout.leftPanelCollapsed]);

  const toggleRightPanel = useCallback(() => {
    const nextCollapsed = !state.layout.rightPanelCollapsed;
    appManager.updateLayout({
      rightPanelCollapsed: nextCollapsed,
      // Full-width tiled chat and the right panel are fully independent: the
      // middle column tiles over the remaining width while the right panel
      // stays whatever the user left it. Opening the right panel must NOT
      // exit full-width (that was the "two-sided trap" — the user opened the
      // panel and the full-width state was yanked away again).
    });
  }, [state.layout.rightPanelCollapsed]);

  const toggleBottomTerminalPanel = useCallback(() => {
    appManager.updateLayout({
      bottomTerminalPanelCollapsed: !state.layout.bottomTerminalPanelCollapsed
    });
  }, [state.layout.bottomTerminalPanelCollapsed]);

  const toggleChatFullWidth = useCallback(() => {
    const next = !state.layout.chatFullWidth;
    appManager.updateLayout({
      chatFullWidth: next,
      // Full-width tiled chat tiles the middle conversation column over the
      // available width — it must NOT force the right panel closed (that was
      // the "two-sided trap": entering full-width closed the panel, opening
      // the panel exited full-width). The right panel keeps whatever state the
      // user left it in; opening it while full-width is active exits to the
      // split layout via toggleRightPanel/expand-right-panel instead.
    });
  }, [state.layout.chatFullWidth]);

  const toggleChatPanel = useCallback(() => {
    const nextChatCollapsed = !state.layout.chatCollapsed;
    appManager.updateLayout({
      chatCollapsed: nextChatCollapsed,
      // Full-width tiled chat only makes sense while the chat pane is visible;
      // hide it along with the chat pane and let the right panel take over.
      chatFullWidth: nextChatCollapsed ? false : state.layout.chatFullWidth,
      // Keep behavior aligned with editor-mode layout:
      // when chat is hidden, ensure the right panel is visible to occupy center space.
      rightPanelCollapsed: nextChatCollapsed ? false : state.layout.rightPanelCollapsed
    });
  }, [state.layout.chatCollapsed, state.layout.chatFullWidth, state.layout.rightPanelCollapsed]);

  const switchLeftPanelTab = useCallback((tab: PanelType) => {
    appManager.updateLayout({
      leftPanelActiveTab: tab,
      leftPanelCollapsed: false // Auto-expand panel when switching tabs
    });
  }, []);

  const updateLeftPanelWidth = useCallback((width: number) => {
    // Clamp width: minimum 50px, no upper bound
    const MIN_WIDTH = 50;
    const clampedWidth = Math.max(MIN_WIDTH, width);
    
    appManager.updateLayout({
      leftPanelWidth: clampedWidth
    });
  }, []);

  const updateCenterPanelWidth = useCallback((width: number) => {
    // Clamp width: minimum 400px
    const MIN_WIDTH = 400;
    const clampedWidth = Math.max(MIN_WIDTH, width);
    
    appManager.updateLayout({
      centerPanelWidth: clampedWidth
    });
  }, []);

  const updateRightPanelWidth = useCallback((width: number, options?: { bypassMax?: boolean }) => {
    // Clamp to [300, 1200]: the compact minimum and the classic MAX_WIDTH cap.
    // Non-drag paths (default open, persisted-width restore, resize validation)
    // must never blow the right panel past 1200px, which would squash the chat
    // pane to its 400px minimum on open. The drag path (SessionScene
    // handleMouseDownResizer) passes bypassMax:true and is capped only by
    // SessionScene's own dynamic upper bound (container − resizer − min chat),
    // so a user's wider manual width is kept instead of being pulled back.
    const MIN_WIDTH = 300;
    const MAX_WIDTH = 1200;
    const clampedWidth = options?.bypassMax
      ? Math.max(MIN_WIDTH, width)
      : Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, width));

    appManager.updateLayout({
      rightPanelWidth: clampedWidth
    });
  }, []);

  const updateBottomTerminalPanelHeight = useCallback((height: number) => {
    const MIN_HEIGHT = 120;
    const MAX_HEIGHT = 640;
    const clampedHeight = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, height));

    appManager.updateLayout({
      bottomTerminalPanelHeight: clampedHeight
    });
  }, []);

  const toggleCenterPanel = useCallback(() => {
    appManager.updateLayout({
      centerPanelCollapsed: !state.layout.centerPanelCollapsed
    });
  }, [state.layout.centerPanelCollapsed]);

  const updateAgentConfig = useCallback(async (agentId: string, config: Partial<AgentConfig>): Promise<void> => {
    try {
      await appManager.updateAgentConfig(agentId, config);
    } catch (error) {
      log.error('Failed to update agent config', error);
      throw error;
    }
  }, []);

  // Chat actions
  const createChatSession = useCallback(async (agentId: string): Promise<ChatSession> => {
    try {
      return await appManager.createChatSession(agentId);
    } catch (error) {
      log.error('Failed to create chat session', error);
      throw error;
    }
  }, []);

  const selectChatSession = useCallback((sessionId: string) => {
    try {
      appManager.selectChatSession(sessionId);
    } catch (error) {
      log.error('Failed to select chat session', error);
    }
  }, []);

  const sendMessage = useCallback(async (content: string): Promise<void> => {
    if (!state.activeChatSession) {
      // Create a new session if there is no active session
      if (state.currentAgent) {
        const session = await createChatSession(state.currentAgent.id);
        await appManager.sendMessage(session.id, content);
      } else {
        throw new Error('No active agent or chat session');
      }
    } else {
      try {
        await appManager.sendMessage(state.activeChatSession.id, content);
      } catch (error) {
        log.error('Failed to send message', error);
        throw error;
      }
    }
  }, [state.activeChatSession, state.currentAgent, createChatSession]);

  // Extension actions
  const enableExtension = useCallback(async (extensionId: string): Promise<void> => {
    try {
      await appManager.enableExtension(extensionId);
    } catch (error) {
      log.error('Failed to enable extension', error);
      throw error;
    }
  }, []);

  const disableExtension = useCallback(async (extensionId: string): Promise<void> => {
    try {
      await appManager.disableExtension(extensionId);
    } catch (error) {
      log.error('Failed to disable extension', error);
      throw error;
    }
  }, []);

  // Tab actions
  const openTab = useCallback((tab: Omit<TabInfo, 'id'>): string => {
    try {
      return appManager.openTab(tab);
    } catch (error) {
      log.error('Failed to open tab', error);
      throw error;
    }
  }, []);

  const closeTab = useCallback((tabId: string) => {
    try {
      appManager.closeTab(tabId);
    } catch (error) {
      log.error('Failed to close tab', error);
    }
  }, []);

  const selectTab = useCallback((tabId: string) => {
    try {
      appManager.selectTab(tabId);
    } catch (error) {
      log.error('Failed to select tab', error);
    }
  }, []);

  // Utility actions
  const clearError = useCallback(() => {
    appManager.clearError();
  }, []);

  return {
    // State
    state,

    // Layout actions
    toggleLeftPanel,
    toggleCenterPanel,
    toggleRightPanel,
    toggleBottomTerminalPanel,
    toggleChatPanel,
    toggleChatFullWidth,
    switchLeftPanelTab,
    updateLeftPanelWidth,
    updateCenterPanelWidth,
    updateRightPanelWidth,
    updateBottomTerminalPanelHeight,

    updateAgentConfig,

    // Chat actions
    createChatSession,
    selectChatSession,
    sendMessage,

    // Extension actions
    enableExtension,
    disableExtension,

    // Tab actions
    openTab,
    closeTab,
    selectTab,

    // Utility actions
    clearError
  };
};

// Layout helper hook
export const useLayout = () => {
  const { state, toggleLeftPanel, toggleRightPanel, toggleBottomTerminalPanel, toggleChatPanel, switchLeftPanelTab, updateLeftPanelWidth } = useApp();
  
  return {
    layout: state.layout,
    toggleLeftPanel,
    toggleRightPanel,
    toggleBottomTerminalPanel,
    toggleChatPanel,
    switchLeftPanelTab,
    updateLeftPanelWidth
  };
};

// Chat helper hook
export const useChat = () => {
  const { state, createChatSession, selectChatSession, sendMessage } = useApp();
  
  return {
    sessions: state.chatSessions,
    activeSession: state.activeChatSession,
    currentAgent: state.currentAgent,
    createSession: createChatSession,
    selectSession: selectChatSession,
    sendMessage
  };
};

// Tab helper hook
export const useTabs = () => {
  const { state, openTab, closeTab, selectTab } = useApp();
  
  return {
    tabs: state.layout.rightPanelTabs,
    activeTabId: state.layout.rightPanelActiveTabId,
    openTab,
    closeTab,
    selectTab
  };
};

// ─── Fine-grained layout subscription ─────────────────────────────────────
// useApp() re-renders on every AppState change (any panel drag, chat session
// update, agent change, …). Hot paths such as ChatPane and BtwSessionPanel
// only need a single boolean; subscribing through useSyncExternalStore keeps
// the component mounted without re-rendering when unrelated state changes.
// getSnapshot returns a primitive boolean, so React's Object.is comparison
// short-circuits re-renders unless chatFullWidth actually flips.
export function useChatFullWidth(): boolean {
  return useSyncExternalStore(
    (callback) => appManager.addEventListener(() => callback()),
    () => appManager.getState().layout.chatFullWidth,
    () => false, // SSR / non-browser snapshot
  );
}
