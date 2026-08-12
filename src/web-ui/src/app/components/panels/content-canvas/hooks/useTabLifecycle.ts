/**
 * useTabLifecycle Hook
 * Manages tab lifecycle state transitions.
 *
 * State flow:
 * - Single click -> preview (replaces current preview tab)
 * - Double click / edit -> active
 * - Pin action -> pinned
 */

import { useCallback, useEffect } from 'react';
import {
  useCanvasStore,
  useAgentCanvasStore,
  useProjectCanvasStore,
  useGitCanvasStore,
  useBottomTerminalCanvasStore,
  GROUP_STATE_KEY,
} from '../stores';
import type { EditorGroupId, EditorGroupState, PanelContent, CreateTabEventDetail } from '../types';
import { EDITOR_GROUP_IDS, GRID_MAX_DIM } from '../types/layout';
import { TAB_EVENTS } from '../types';
import { useI18n } from '@/infrastructure/i18n';
import { drainPendingTabs } from '@/shared/services/pendingTabQueue';
import { confirmDialog } from '@/component-library/components/ConfirmDialog/confirmService';

/** Count visible (non-hidden) tabs in a canvas store editor group. */
const getVisibleTabCount = (
  state: ReturnType<typeof useAgentCanvasStore.getState>,
  groupId: EditorGroupId,
): number => {
  // Resolve through GROUP_STATE_KEY (single source of truth, shared with
  // canvasStore): covers primary/secondary/tertiary AND slot4..slot16.
  const group = state[GROUP_STATE_KEY[groupId]];
  if (!group || typeof group === 'boolean' || typeof group === 'string' || typeof group === 'number') return 0;
  const tabs = (group as { tabs?: Array<{ isHidden?: boolean }> }).tabs;
  if (!Array.isArray(tabs)) return 0;
  return tabs.filter(t => !t.isHidden).length;
};

interface UseTabLifecycleOptions {
  /** App mode / target canvas */
  mode?: 'agent' | 'project' | 'git' | 'bottom-terminal';
  /** Override the external tab creation event for specialized canvases. */
  createTabEventName?: string;
  /** Override the panel expansion event dispatched after tab activation. */
  expandPanelEventName?: string;
}

interface UseTabLifecycleReturn {
  /** Open on single click (preview mode) */
  openPreview: (content: PanelContent, groupId?: EditorGroupId) => void;

  /** Open on double click (active mode) */
  openActive: (content: PanelContent, groupId?: EditorGroupId) => void;

  /** Promote to active on content edit */
  onContentEdit: (tabId: string, groupId: EditorGroupId) => void;

  /** Toggle pin/unpin */
  togglePin: (tabId: string, groupId: EditorGroupId) => void;

  /** Dirty check before closing a tab */
  handleCloseWithDirtyCheck: (tabId: string, groupId: EditorGroupId) => Promise<boolean>;

  /** Dirty check before closing all tabs */
  handleCloseAllWithDirtyCheck: (groupId: EditorGroupId) => Promise<boolean>;
}

/**
 * Tab lifecycle management hook.
 */
export const useTabLifecycle = (options: UseTabLifecycleOptions = {}): UseTabLifecycleReturn => {
  const {
    mode = 'agent',
    createTabEventName,
    expandPanelEventName = TAB_EVENTS.EXPAND_RIGHT_PANEL,
  } = options;
  const { t } = useI18n('components');
  const canvasStoreApi =
    mode === 'project'
      ? useProjectCanvasStore
      : mode === 'git'
        ? useGitCanvasStore
        : mode === 'bottom-terminal'
          ? useBottomTerminalCanvasStore
          : useAgentCanvasStore;
  
  const {
    addTab,
    promoteTab,
    togglePinTab,
    findTabByMetadata,
    switchToTab,
    updateTabContent,
    closeTab,
    closeAllTabs,
    activeGroupId,
    layout,
    setSplitMode,
  } = useCanvasStore();

  /**
   * Open in preview mode (replaces current preview tab).
   */
  const openPreview = useCallback((content: PanelContent, groupId?: EditorGroupId) => {
    const targetGroupId = groupId || activeGroupId;
    
    // Check for existing tab with same content
    if (content.metadata?.duplicateCheckKey) {
      const existing = findTabByMetadata({ duplicateCheckKey: content.metadata.duplicateCheckKey });
      if (existing) {
        // Switch to existing tab
        switchToTab(existing.tab.id, existing.groupId);
        return;
      }
    }
    
    // Add preview tab (auto-replaces current preview tab)
    addTab(content, 'preview', targetGroupId);
  }, [activeGroupId, findTabByMetadata, switchToTab, addTab]);

  /**
   * Open directly in active state.
   */
  const openActive = useCallback((content: PanelContent, groupId?: EditorGroupId) => {
    const targetGroupId = groupId || activeGroupId;
    
    // Check for existing tab with same content
    if (content.metadata?.duplicateCheckKey) {
      const existing = findTabByMetadata({ duplicateCheckKey: content.metadata.duplicateCheckKey });
      if (existing) {
        // Switch to existing tab and ensure active state
        switchToTab(existing.tab.id, existing.groupId);
        if (existing.tab.state === 'preview') {
          promoteTab(existing.tab.id, existing.groupId);
        }
        return;
      }
    }
    
    // Add active tab
    addTab(content, 'active', targetGroupId);
  }, [activeGroupId, findTabByMetadata, switchToTab, promoteTab, addTab]);

  /**
   * Promote to active on edit.
   */
  const onContentEdit = useCallback((tabId: string, groupId: EditorGroupId) => {
    promoteTab(tabId, groupId);
  }, [promoteTab]);

  /**
   * Toggle pin/unpin.
   */
  const togglePin = useCallback((tabId: string, groupId: EditorGroupId) => {
    togglePinTab(tabId, groupId);
  }, [togglePinTab]);

  /**
   * Dirty check before closing a tab.
   */
  const handleCloseWithDirtyCheck = useCallback(async (tabId: string, groupId: EditorGroupId): Promise<boolean> => {
    // Generic mapping through GROUP_STATE_KEY (single source of truth shared
    // with canvasStore): resolves primary/secondary/tertiary AND slot4..slot16,
    // so tabs in any of the 16 grid9 cells can be closed. The old ternary only
    // decoded the legacy 3 groups, silently no-op'ing slot closes.
    const state = canvasStoreApi.getState();
    const group = state[GROUP_STATE_KEY[groupId]] as EditorGroupState;
    const tab = group.tabs.find(t => t.id === tabId);

    if (!tab) {
      return true;
    }

    if (tab.isDirty) {
      const result = await confirmDialog({
        title: t('tabs.unsaved'),
        message: t('tabs.confirmCloseWithDirty', { title: tab.title }),
        type: 'warning',
        confirmDanger: true,
      });

      if (!result) {
        return false;
      }
    }

    closeTab(tabId, groupId);
    return true;
  }, [canvasStoreApi, closeTab, t]);

  /**
   * Dirty check before closing all tabs.
   */
  const handleCloseAllWithDirtyCheck = useCallback(async (groupId: EditorGroupId): Promise<boolean> => {
    // Same generic mapping as handleCloseWithDirtyCheck: covers slot4..slot16
    // so "close all" works in every expanded grid9 cell.
    const state = canvasStoreApi.getState();
    const group = state[GROUP_STATE_KEY[groupId]] as EditorGroupState;
    const closableTabs = group.tabs.filter(t => t.state !== 'pinned');
    const dirtyTabs = closableTabs.filter(t => t.isDirty);

    if (closableTabs.length === 0 || dirtyTabs.length === 0) {
      closeAllTabs(groupId);
      return true;
    }

    const fileList = dirtyTabs.map(t => `  - ${t.title}`).join('\n');
    const result = await confirmDialog({
      title: t('tabs.unsaved'),
      message: t('tabs.confirmCloseAllWithDirty', { count: dirtyTabs.length, fileList }),
      type: 'warning',
      confirmDanger: true,
      preview: fileList,
    });

    if (!result) {
      return false;
    }

    closeAllTabs(groupId);
    return true;
  }, [canvasStoreApi, closeAllTabs, t]);

  /**
   * Listen for left-panel terminal close events to sync right-panel tabs.
   */
  useEffect(() => {
    const store = mode === 'project' ? useProjectCanvasStore
                : mode === 'git' ? useGitCanvasStore
                : mode === 'bottom-terminal' ? useBottomTerminalCanvasStore
                : useAgentCanvasStore;
    
    const handleTerminalSessionDestroyed = (event: CustomEvent<{ sessionId: string }>) => {
      const { sessionId } = event.detail ?? {};
      if (sessionId) {
        store.getState().closeTerminalTabBySessionId(sessionId);
      }
    };
    window.addEventListener('terminal-session-destroyed', handleTerminalSessionDestroyed as EventListener);
    return () => {
      window.removeEventListener('terminal-session-destroyed', handleTerminalSessionDestroyed as EventListener);
    };
  }, [mode]);

  /**
   * Listen for left-panel terminal rename events to sync right-panel tabs.
   */
  useEffect(() => {
    const store = mode === 'project' ? useProjectCanvasStore
                : mode === 'git' ? useGitCanvasStore
                : mode === 'bottom-terminal' ? useBottomTerminalCanvasStore
                : useAgentCanvasStore;
    
    const handleTerminalSessionRenamed = (event: CustomEvent<{ sessionId: string; newName: string }>) => {
      const { sessionId, newName } = event.detail ?? {};
      if (sessionId && newName) {
        store.getState().renameTerminalTabBySessionId(sessionId, newName);
      }
    };
    window.addEventListener('terminal-session-renamed', handleTerminalSessionRenamed as EventListener);
    return () => {
      window.removeEventListener('terminal-session-renamed', handleTerminalSessionRenamed as EventListener);
    };
  }, [mode]);

  /**
   * Listen for external tab creation events.
   */
  useEffect(() => {
    const eventName = createTabEventName ??
      (mode === 'project'
        ? TAB_EVENTS.PROJECT_CREATE_TAB
        : mode === 'git'
          ? TAB_EVENTS.GIT_CREATE_TAB
          : mode === 'bottom-terminal'
            ? TAB_EVENTS.BOTTOM_TERMINAL_CREATE_TAB
            : TAB_EVENTS.AGENT_CREATE_TAB);

    const handleCreateTab = (event: CustomEvent<CreateTabEventDetail>) => {
      const {
        type,
        title,
        data,
        metadata,
        checkDuplicate,
        duplicateCheckKey,
        replaceExisting,
        targetGroup,
        enableSplitView,
      } = event.detail;

      const content: PanelContent = {
        type,
        title,
        data,
        metadata: { ...metadata, duplicateCheckKey },
      };

      // If split view is enabled, switch to vertical split first (top/bottom)
      if (enableSplitView && layout.splitMode === 'none') {
        setSplitMode('vertical');
      }
      
      // Check duplicates
      if (checkDuplicate && duplicateCheckKey) {
        const existing = findTabByMetadata({ duplicateCheckKey });
        if (existing) {
          const hasJumpInfo = data?.jumpToRange || data?.jumpToLine || data?.jumpToColumn;

          if (replaceExisting || hasJumpInfo) {
            // Update content
            updateTabContent(existing.tab.id, existing.groupId, content);
          }
          
          // Switch to existing tab
          switchToTab(existing.tab.id, existing.groupId);
          
          window.dispatchEvent(new CustomEvent(expandPanelEventName));
          return;
        }
      }
      
      // Determine target group: use specified group when split enabled, otherwise active group
      // btw-session tabs (subagent side-threads / review windows) prefer an empty
      // grid9 cell over stacking into an existing window: in grid9 mode emptied
      // slots persist as drop targets, so a newly opened subagent fills a blank
      // window first (r < grid9RowsCount && c < grid9ColsCount keeps the search
      // inside the active frame). Non-grid9 modes auto-merge empty groups, so the
      // fallback stays the plain target/active group.
      let groupId = (enableSplitView && targetGroup) ? targetGroup : (targetGroup || activeGroupId);
      if (type === 'btw-session' && layout.splitMode === 'grid9') {
        const canvasState = canvasStoreApi.getState();
        const { grid9ColsCount, grid9RowsCount } = canvasState.layout;
        const firstEmpty = EDITOR_GROUP_IDS.find((gid, idx) => {
          const row = Math.floor(idx / GRID_MAX_DIM);
          const col = idx % GRID_MAX_DIM;
          if (row >= grid9RowsCount || col >= grid9ColsCount) return false;
          return getVisibleTabCount(canvasState, gid) === 0;
        });
        if (firstEmpty) groupId = firstEmpty;
      }

      // Open all tabs in active state by default (no preview replacement)
      addTab(content, 'active', groupId);
      
      window.dispatchEvent(new CustomEvent(expandPanelEventName));
    };

    window.addEventListener(eventName, handleCreateTab as EventListener);

    // Drain any tab events that were enqueued before this listener was
    // registered (happens when the scene was just mounted for the first time).
    if (mode !== 'bottom-terminal') {
      const pendingMode = mode === 'project' ? 'project' : mode === 'git' ? 'git' : 'agent';
      const pending = drainPendingTabs(pendingMode);
      pending.forEach(detail => handleCreateTab({ detail } as CustomEvent<CreateTabEventDetail>));
    }
    
    return () => {
      window.removeEventListener(eventName, handleCreateTab as EventListener);
    };
  }, [mode, createTabEventName, expandPanelEventName, findTabByMetadata, updateTabContent, switchToTab, addTab, activeGroupId, layout.splitMode, setSplitMode, canvasStoreApi]);

  return {
    openPreview,
    openActive,
    onContentEdit,
    togglePin,
    handleCloseWithDirtyCheck,
    handleCloseAllWithDirtyCheck,
  };
};

export default useTabLifecycle;
