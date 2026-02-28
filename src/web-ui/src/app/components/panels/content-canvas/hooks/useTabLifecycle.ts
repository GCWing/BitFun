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
import { useCanvasStore } from '../stores';
import type { EditorGroupId, PanelContent, CreateTabEventDetail } from '../types';
import { TAB_EVENTS } from '../types';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';
import { drainPendingTabs } from '@/shared/services/pendingTabQueue';

const log = createLogger('useTabLifecycle');

interface UseTabLifecycleOptions {
  /** App mode / target canvas */
  mode?: 'agent' | 'project' | 'git';
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
  const { mode = 'agent' } = options;
  const { t } = useI18n('components');
  
  const {
    addTab,
    promoteTab,
    togglePinTab,
    findTabByMetadata,
    switchToTab,
    updateTabContent,
    closeTab,
    closeAllTabs,
    primaryGroup,
    secondaryGroup,
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
    const group = groupId === 'primary' ? primaryGroup : secondaryGroup;
    const tab = group.tabs.find(t => t.id === tabId);

    if (!tab) {
      return true;
    }

    if (tab.isDirty) {
      // Show confirmation and ensure correct return handling
      const resultPromise = window.confirm(
        t('tabs.confirmCloseWithDirty', { title: tab.title })
      );

      // Await if confirm returns a Promise
      const result = resultPromise instanceof Promise ? await resultPromise : resultPromise;

      if (!result) {
        return false;
      }
    }

    closeTab(tabId, groupId);
    return true;
  }, [primaryGroup, secondaryGroup, closeTab, t]);

  /**
   * Dirty check before closing all tabs.
   */
  const handleCloseAllWithDirtyCheck = useCallback(async (groupId: EditorGroupId): Promise<boolean> => {
    const group = groupId === 'primary' ? primaryGroup : secondaryGroup;
    const dirtyTabs = group.tabs.filter(t => t.isDirty);

    if (dirtyTabs.length === 0) {
      closeAllTabs(groupId);
      return true;
    }

    // Show confirmation with list of dirty files
    const fileList = dirtyTabs.map(t => `  - ${t.title}`).join('\n');
    const resultPromise = window.confirm(
      t('tabs.confirmCloseAllWithDirty', { count: dirtyTabs.length, fileList })
    );

    // Await if confirm returns a Promise
    const result = resultPromise instanceof Promise ? await resultPromise : resultPromise;

    if (!result) {
      return false;
    }

    closeAllTabs(groupId);
    return true;
  }, [primaryGroup, secondaryGroup, closeAllTabs, t]);

  /**
   * Listen for left-panel terminal close events to sync right-panel tabs.
   */
  useEffect(() => {
    const handleTerminalSessionDestroyed = (event: CustomEvent<{ sessionId: string }>) => {
      const { sessionId } = event.detail ?? {};
      if (sessionId) {
        useCanvasStore.getState().closeTerminalTabBySessionId(sessionId);
      }
    };
    window.addEventListener('terminal-session-destroyed', handleTerminalSessionDestroyed as EventListener);
    return () => {
      window.removeEventListener('terminal-session-destroyed', handleTerminalSessionDestroyed as EventListener);
    };
  }, []);

  /**
   * Listen for external tab creation events.
   */
  useEffect(() => {
    const eventName =
      mode === 'project'
        ? TAB_EVENTS.PROJECT_CREATE_TAB
        : mode === 'git'
          ? TAB_EVENTS.GIT_CREATE_TAB
          : TAB_EVENTS.AGENT_CREATE_TAB;

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
          
          // Trigger right panel expansion
          window.dispatchEvent(new CustomEvent(TAB_EVENTS.EXPAND_RIGHT_PANEL));
          return;
        }
      }
      
      // Determine target group: use specified group when split enabled, otherwise active group
      const groupId = (enableSplitView && targetGroup) ? targetGroup : (targetGroup || activeGroupId);
      
      // Open all tabs in active state by default (no preview replacement)
      addTab(content, 'active', groupId);
      
      // Trigger right panel expansion
      window.dispatchEvent(new CustomEvent(TAB_EVENTS.EXPAND_RIGHT_PANEL));
    };

    window.addEventListener(eventName, handleCreateTab as EventListener);

    // Drain any tab events that were enqueued before this listener was
    // registered (happens when the scene was just mounted for the first time).
    const pendingMode = mode === 'project' ? 'project' : mode === 'git' ? 'git' : 'agent';
    const pending = drainPendingTabs(pendingMode);
    pending.forEach(detail => handleCreateTab({ detail } as CustomEvent<CreateTabEventDetail>));
    
    return () => {
      window.removeEventListener(eventName, handleCreateTab as EventListener);
    };
  }, [mode, findTabByMetadata, updateTabContent, switchToTab, addTab, activeGroupId, layout.splitMode, setSplitMode]);

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
