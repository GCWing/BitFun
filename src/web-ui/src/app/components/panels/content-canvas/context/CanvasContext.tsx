/**
 * CanvasContext - canvas global context.
 * Provides access to tabs and layout state to reduce props drilling.
 */

import React, { createContext, useContext, useMemo, ReactNode } from 'react';
import type {
  CanvasTab,
  EditorGroupId,
  EditorGroupState,
  LayoutState,
  TabDragPayload,
  PanelContent,
  TabState,
  DropPosition,
  AnchorPosition,
} from '../types';

// ==================== Operations Interfaces ====================

/**
 * Tab operations.
 */
export interface TabOperations {
  /** Add tab */
  addTab: (content: PanelContent, state?: TabState, groupId?: EditorGroupId) => void;
  /** Close tab */
  closeTab: (tabId: string, groupId: EditorGroupId) => Promise<void>;
  /** Close all tabs */
  closeAllTabs: (groupId?: EditorGroupId) => void;
  /** Switch to tab */
  switchToTab: (tabId: string, groupId: EditorGroupId) => void;
  /** Update tab content */
  updateTabContent: (tabId: string, groupId: EditorGroupId, content: PanelContent) => void;
  /** Set tab dirty state */
  setTabDirty: (tabId: string, groupId: EditorGroupId, isDirty: boolean) => void;

  /** File missing on disk (for tab chrome) */
  setTabFileDeletedFromDisk: (tabId: string, groupId: EditorGroupId, deleted: boolean) => void;
  /** Promote tab state (preview -> active) */
  promoteTab: (tabId: string, groupId: EditorGroupId) => void;
  /** Pin/unpin tab */
  togglePinTab: (tabId: string, groupId: EditorGroupId) => void;
  /** Find tab by metadata */
  findTabByMetadata: (metadata: Record<string, any>) => { tab: CanvasTab; groupId: EditorGroupId } | null;
  /** Reopen recently closed tab */
  reopenClosedTab: () => void;
}

/**
 * Drag operations.
 */
export interface DragOperations {
  /** Start drag */
  onDragStart: (payload: TabDragPayload) => void;
  /** End drag */
  onDragEnd: () => void;
  /** Drop tab */
  onDrop: (tabId: string, sourceGroupId: EditorGroupId, targetGroupId: EditorGroupId, position?: DropPosition) => void;
  /** Reorder tab */
  reorderTab: (tabId: string, groupId: EditorGroupId, newIndex: number) => void;
  /** Current dragging tab ID */
  draggingTabId: string | null;
  /** Drag source group ID */
  draggingFromGroupId: EditorGroupId | null;
}

/**
 * Layout operations.
 */
export interface LayoutOperations {
  /** Set split mode */
  setSplitMode: (mode: 'none' | 'horizontal' | 'vertical') => void;
  /** Set split ratio */
  setSplitRatio: (ratio: number) => void;
  /** Set anchor position */
  setAnchorPosition: (position: AnchorPosition) => void;
  /** Set anchor size */
  setAnchorSize: (size: number) => void;
  /** Toggle maximize */
  toggleMaximize: () => void;
  /** Set active editor group */
  setActiveGroup: (groupId: EditorGroupId) => void;
}

/**
 * Mission control operations.
 */
export interface MissionControlOperations {
  /** Open mission control */
  openMissionControl: () => void;
  /** Close mission control */
  closeMissionControl: () => void;
  /** Toggle mission control */
  toggleMissionControl: () => void;
}

// ==================== Context Value Types ====================

export interface CanvasContextValue {
  // State
  primaryGroup: EditorGroupState;
  secondaryGroup: EditorGroupState;
  activeGroupId: EditorGroupId;
  layout: LayoutState;
  isMissionControlOpen: boolean;
  workspacePath?: string;
  
  // Operations
  tabOps: TabOperations;
  dragOps: DragOperations;
  layoutOps: LayoutOperations;
  missionControlOps: MissionControlOperations;
  
  // Panel interactions
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
  onBeforeClose?: (content: PanelContent | null) => Promise<boolean>;
}

// ==================== Context Creation ====================

const CanvasContext = createContext<CanvasContextValue | null>(null);

// ==================== Provider Props ====================

export interface CanvasProviderProps {
  children: ReactNode;
  value: CanvasContextValue;
}

// ==================== Provider Component ====================

export const CanvasProvider: React.FC<CanvasProviderProps> = ({
  children,
  value,
}) => {
  // Memoize value to avoid unnecessary re-renders
  const memoizedValue = useMemo(() => value, [
    value.primaryGroup,
    value.secondaryGroup,
    value.activeGroupId,
    value.layout,
    value.isMissionControlOpen,
    value.workspacePath,
    value.tabOps,
    value.dragOps,
    value.layoutOps,
    value.missionControlOps,
    value.onInteraction,
    value.onBeforeClose,
  ]);

  return (
    <CanvasContext.Provider value={memoizedValue}>
      {children}
    </CanvasContext.Provider>
  );
};

// ==================== Hooks ====================

/**
 * Get full Canvas context.
 */
export const useCanvas = (): CanvasContextValue => {
  const context = useContext(CanvasContext);
  if (!context) {
    throw new Error('useCanvas must be used within a CanvasProvider');
  }
  return context;
};

/**
 * Get state for a specific editor group only.
 */
export const useEditorGroup = (groupId: EditorGroupId) => {
  const { primaryGroup, secondaryGroup, activeGroupId, tabOps, dragOps } = useCanvas();
  
  const group = groupId === 'primary' ? primaryGroup : secondaryGroup;
  const isActive = activeGroupId === groupId;
  
  return {
    group,
    isActive,
    tabs: group.tabs,
    activeTabId: group.activeTabId,
    tabOps,
    dragOps,
  };
};

/**
 * Get layout state only.
 */
export const useCanvasLayout = () => {
  const { layout, layoutOps } = useCanvas();
  return { layout, ...layoutOps };
};

/**
 * Get tab operations only.
 */
export const useTabActions = () => {
  const { tabOps } = useCanvas();
  return tabOps;
};

/**
 * Get drag state and operations only.
 */
export const useDragState = () => {
  const { dragOps } = useCanvas();
  return dragOps;
};

/**
 * Get mission control state and operations only.
 */
export const useMissionControl = () => {
  const { isMissionControlOpen, missionControlOps } = useCanvas();
  return { isOpen: isMissionControlOpen, ...missionControlOps };
};

export default CanvasContext;
