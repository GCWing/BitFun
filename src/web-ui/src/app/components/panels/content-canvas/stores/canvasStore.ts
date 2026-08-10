/**
 * Canvas Store - canvas state management.
 * Uses Zustand to manage tabs and layout state.
 */

import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import { createContext, useContext } from 'react';
import type {
  CanvasTab,
  EditorGroupId,
  EditorGroupState,
  LayoutState,
  TabState,
  PanelContent,
  ClosedTabRecord,
  SplitMode,
  AnchorPosition,
  DropPosition,
} from '../types';
import {
  createTab,
  createEditorGroupState,
  createLayoutState,
  clampSplitRatio,
  clampGrid9Ratio,
  clampAnchorSize,
  EDITOR_GROUP_IDS,
  EDITOR_GROUP_ROW,
  EDITOR_GROUP_COL,
  GRID_MAX_DIM,
} from '../types';
import { normalizePath } from '@/shared/utils/pathUtils';

// ==================== Store State Types ====================

interface CanvasStoreState {
  primaryGroup: EditorGroupState;
  secondaryGroup: EditorGroupState;
  tertiaryGroup: EditorGroupState;
  slot4Group: EditorGroupState;
  slot5Group: EditorGroupState;
  slot6Group: EditorGroupState;
  slot7Group: EditorGroupState;
  slot8Group: EditorGroupState;
  slot9Group: EditorGroupState;
  slot10Group: EditorGroupState;
  slot11Group: EditorGroupState;
  slot12Group: EditorGroupState;
  slot13Group: EditorGroupState;
  slot14Group: EditorGroupState;
  slot15Group: EditorGroupState;
  slot16Group: EditorGroupState;
  activeGroupId: EditorGroupId;
  layout: LayoutState;
  isMissionControlOpen: boolean;
  draggingTabId: string | null;
  draggingFromGroupId: EditorGroupId | null;
  closedTabs: ClosedTabRecord[];
  maxClosedTabsHistory: number;
}

/** State-field key for each editor group. Legacy 3 keep their names for
 *  backward compatibility with external consumers. Exported so lifecycle /
 *  shortcut code reads groups through the same single mapping (single source
 *  of truth for the 16 slot keys). */
export const GROUP_STATE_KEY: Record<EditorGroupId, keyof CanvasStoreState> = {
  primary: 'primaryGroup',
  secondary: 'secondaryGroup',
  tertiary: 'tertiaryGroup',
  slot4: 'slot4Group',
  slot5: 'slot5Group',
  slot6: 'slot6Group',
  slot7: 'slot7Group',
  slot8: 'slot8Group',
  slot9: 'slot9Group',
  slot10: 'slot10Group',
  slot11: 'slot11Group',
  slot12: 'slot12Group',
  slot13: 'slot13Group',
  slot14: 'slot14Group',
  slot15: 'slot15Group',
  slot16: 'slot16Group',
};

interface CanvasStoreActions {
  // ==================== Tab Operations ====================
  
  /** Add tab */
  addTab: (content: PanelContent, state?: TabState, groupId?: EditorGroupId) => void;
  
  /** Close tab; forceRemove removes terminal tab instead of hiding */
  closeTab: (tabId: string, groupId: EditorGroupId, options?: { forceRemove?: boolean }) => void;

  /** Close and remove tab by terminal sessionId (sync when left panel closes terminal) */
  closeTerminalTabBySessionId: (sessionId: string) => void;

  /** Rename terminal tab by sessionId (sync when left panel renames terminal) */
  renameTerminalTabBySessionId: (sessionId: string, newName: string) => void;
  
  /** Close all tabs */
  closeAllTabs: (groupId?: EditorGroupId) => void;
  
  /** Switch to tab */
  switchToTab: (tabId: string, groupId: EditorGroupId) => void;
  
  /** Update tab content */
  updateTabContent: (tabId: string, groupId: EditorGroupId, content: PanelContent) => void;
  
  /** Set tab dirty state */
  setTabDirty: (tabId: string, groupId: EditorGroupId, isDirty: boolean) => void;

  /** Mark whether the tab's file is missing on disk (editor-detected) */
  setTabFileDeletedFromDisk: (tabId: string, groupId: EditorGroupId, deleted: boolean) => void;
  
  /** Promote tab state (preview -> active) */
  promoteTab: (tabId: string, groupId: EditorGroupId) => void;
  
  /** Pin/unpin tab */
  togglePinTab: (tabId: string, groupId: EditorGroupId) => void;
  
  /** Find tab by metadata */
  findTabByMetadata: (metadata: Record<string, any>) => { tab: CanvasTab; groupId: EditorGroupId } | null;
  
  /** Reopen recently closed tab */
  reopenClosedTab: () => void;
  
  /** Hide tab (keep state) */
  hideTab: (tabId: string, groupId: EditorGroupId) => void;
  
  /** Show hidden tab */
  showTab: (tabId: string, groupId: EditorGroupId) => void;
  
  // ==================== Drag Operations ====================
  
  /** Start drag */
  startDrag: (tabId: string, groupId: EditorGroupId) => void;
  
  /** End drag */
  endDrag: () => void;
  
  /** Move tab to another group */
  moveTabToGroup: (tabId: string, fromGroupId: EditorGroupId, toGroupId: EditorGroupId, index?: number) => void;
  
  /** Reorder tabs */
  reorderTab: (tabId: string, groupId: EditorGroupId, newIndex: number) => void;
  
  /** Handle drop */
  handleDrop: (tabId: string, fromGroupId: EditorGroupId, toGroupId: EditorGroupId, position?: DropPosition) => void;
  
  // ==================== Layout Operations ====================
  
  /** Set split mode */
  setSplitMode: (mode: SplitMode) => void;

  /** Apply a preset grid9 template (cols×rows: 2x2 four-cell, 2x3/3x2
   *  six-cell, 3x3 nine-cell). Sets splitMode to grid9 and the active
   *  row/column counts; resets slot groups outside the template. */
  applyGrid9Template: (cols: number, rows: number) => void;

  /** Merge two grid9 cells: all tabs from `fromGroupId` move into
   *  `toGroupId`; the source cell becomes an empty drop target. This is the
   *  "merge two small windows into one" primitive for free arrangement. */
  mergeGrid9Cells: (fromGroupId: EditorGroupId, toGroupId: EditorGroupId) => void;

  /** Remove a blank grid9 cell: the grid shrinks by one column/row and the
   *  remaining cells re-tile to fill the panel; tabs in removed slots are
   *  merged into the surviving cells. */
  removeGrid9Cell: (groupId: EditorGroupId) => void;
  
  /** Set split ratio */
  setSplitRatio: (ratio: number) => void;

  /** Set secondary split ratio used by grid top row */
  setSplitRatio2: (ratio: number) => void;

  /** Set a grid9 column ratio by column index */
  setGrid9ColRatio: (col: number, ratio: number) => void;

  /** Set a grid9 row ratio by row index */
  setGrid9RowRatio: (row: number, ratio: number) => void;
  
  /** Set anchor position */
  setAnchorPosition: (position: AnchorPosition) => void;
  
  /** Set anchor size */
  setAnchorSize: (size: number) => void;
  
  /** Toggle maximize */
  toggleMaximize: () => void;
  
  /** Set active editor group */
  setActiveGroup: (groupId: EditorGroupId) => void;
  
  // ==================== Mission Control ====================
  
  /** Open mission control */
  openMissionControl: () => void;
  
  /** Close mission control */
  closeMissionControl: () => void;
  
  /** Toggle mission control */
  toggleMissionControl: () => void;
  
  // ==================== State Management ====================
  
  /** Reset state */
  reset: () => void;
  
  /** Get all tabs */
  getAllTabs: () => CanvasTab[];
}

type CanvasStore = CanvasStoreState & CanvasStoreActions;

// ==================== Initial State ====================

const initialState: CanvasStoreState = {
  primaryGroup: createEditorGroupState(),
  secondaryGroup: createEditorGroupState(),
  tertiaryGroup: createEditorGroupState(),
  slot4Group: createEditorGroupState(),
  slot5Group: createEditorGroupState(),
  slot6Group: createEditorGroupState(),
  slot7Group: createEditorGroupState(),
  slot8Group: createEditorGroupState(),
  slot9Group: createEditorGroupState(),
  slot10Group: createEditorGroupState(),
  slot11Group: createEditorGroupState(),
  slot12Group: createEditorGroupState(),
  slot13Group: createEditorGroupState(),
  slot14Group: createEditorGroupState(),
  slot15Group: createEditorGroupState(),
  slot16Group: createEditorGroupState(),
  activeGroupId: 'primary',
  layout: createLayoutState(),
  isMissionControlOpen: false,
  draggingTabId: null,
  draggingFromGroupId: null,
  closedTabs: [],
  maxClosedTabsHistory: 10,
};

const getGroup = (draft: CanvasStoreState, groupId: EditorGroupId): EditorGroupState => {
  return draft[GROUP_STATE_KEY[groupId]] as EditorGroupState;
};

const getVisibleTabs = (group: EditorGroupState) => group.tabs.filter(t => !t.isHidden);
const getVisibleCount = (group: EditorGroupState) => getVisibleTabs(group).length;

const ensureValidActiveTab = (group: EditorGroupState) => {
  const visibleTabs = getVisibleTabs(group);
  if (visibleTabs.length === 0) {
    group.activeTabId = null;
  } else if (group.activeTabId === null || !visibleTabs.find(t => t.id === group.activeTabId)) {
    group.activeTabId = visibleTabs[0]?.id || null;
  }
};

const keepPinnedTabsOnly = (group: EditorGroupState) => {
  group.tabs = group.tabs.filter(tab => tab.state === 'pinned');
  ensureValidActiveTab(group);
};

const getPinnedBoundary = (group: EditorGroupState) => {
  const firstUnpinnedIndex = group.tabs.findIndex(tab => tab.state !== 'pinned');
  return firstUnpinnedIndex === -1 ? group.tabs.length : firstUnpinnedIndex;
};

const insertTabRespectingPinnedBoundary = (group: EditorGroupState, tab: CanvasTab) => {
  const insertIndex = getPinnedBoundary(group);
  group.tabs.splice(insertIndex, 0, tab);
};

/**
 * Reset grid9 column/row ratios to equal shares. Templates always tile evenly
 * (d7-P2-7): applying a template resets the ratios and clears the user-adjust
 * flag. Cell add/remove operations keep user-adjusted ratios (see
 * preserveGrid9RatiosOnAxisChange below) instead of wiping them.
 */
const resetGrid9Ratios = (layout: LayoutState) => {
  for (let i = 0; i < GRID_MAX_DIM; i++) {
    layout.grid9Cols[i] = 1 / GRID_MAX_DIM;
    layout.grid9Rows[i] = 1 / GRID_MAX_DIM;
  }
  layout.grid9RatiosUserAdjusted = false;
};

/**
 * Keep user-adjusted grid9 ratios when the axis count changes (edge-drop
 * growth, blank-cell removal, trailing-row downgrade): if the user resized
 * columns/rows via SplitHandle, their shares are preserved and only the new
 * active axes are normalized to the equal share; otherwise the ratios are
 * reset to even tiles so the remaining cells always fill the panel
 * (d7-P2-7).
 */
const preserveGrid9RatiosOnAxisChange = (layout: LayoutState, cols: number, rows: number) => {
  if (layout.grid9RatiosUserAdjusted) {
    // Keep user shares for the active axes; extend any inactive axis to the
    // equal share so newly grown cells tile evenly.
    for (let c = 0; c < GRID_MAX_DIM; c++) {
      if (c >= cols && layout.grid9Cols[c] <= 0) {
        layout.grid9Cols[c] = 1 / GRID_MAX_DIM;
      }
    }
    for (let r = 0; r < GRID_MAX_DIM; r++) {
      if (r >= rows && layout.grid9Rows[r] <= 0) {
        layout.grid9Rows[r] = 1 / GRID_MAX_DIM;
      }
    }
    return;
  }
  resetGrid9Ratios(layout);
};

// ==================== Store Creation ====================

const createCanvasStoreHook = () => create<CanvasStore>()(
  immer((set, get) => ({
      ...initialState,
      
      // ==================== Tab Operations ====================
      
      addTab: (content, state = 'preview', groupId) => {
        set((draft) => {
          let targetGroupId = groupId || draft.activeGroupId;
          
          // Adjust target group based on splitMode to ensure visibility
          if (draft.layout.splitMode === 'none') {
            // Single-column mode: use primary group only
            targetGroupId = 'primary';
            draft.activeGroupId = 'primary';
          } else if (draft.layout.splitMode === 'horizontal' || draft.layout.splitMode === 'vertical') {
            // Two-column mode: use primary or secondary (not tertiary)
            if (targetGroupId === 'tertiary') {
              targetGroupId = draft.activeGroupId === 'primary' ? 'primary' : 'secondary';
              draft.activeGroupId = targetGroupId;
            }
          }
          // Grid / grid9 mode: all group slots are allowed
          
          const group = getGroup(draft, targetGroupId);
          
          if (state === 'preview') {
            const previewIndex = group.tabs.findIndex(
              t => t.state === 'preview' && !t.isHidden
            );
            if (previewIndex !== -1) {
              group.tabs.splice(previewIndex, 1);
            }
          }
          
          const newTab = createTab(content, state);
          insertTabRespectingPinnedBoundary(group, newTab);
          group.activeTabId = newTab.id;
          draft.activeGroupId = targetGroupId;
        });
      },
      
      closeTab: (tabId, groupId, options) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tabIndex = group.tabs.findIndex(t => t.id === tabId);
          
          if (tabIndex === -1) return;
          
          const tab = group.tabs[tabIndex];
          const forceRemove = options?.forceRemove === true;

          // For terminal tabs without force remove, hide instead of delete for reactivation
          if (tab.content.type === 'terminal' && !forceRemove) {
            tab.isHidden = true;
            
            // If closing active tab, switch to next visible tab
            if (group.activeTabId === tabId) {
              const visibleTabs = group.tabs.filter(t => !t.isHidden);
              group.activeTabId = visibleTabs[0]?.id || null;
            }
            return;
          }
          
          // Skip history when terminal is force-removed
          if (!(tab.content.type === 'terminal' && forceRemove)) {
            // Record in close history
            draft.closedTabs.unshift({
              tab: { ...tab },
              closedAt: Date.now(),
              groupId,
              index: tabIndex,
            });
            // Limit history size
            if (draft.closedTabs.length > draft.maxClosedTabsHistory) {
              draft.closedTabs.pop();
            }
          }
          
          // Remove tab
          group.tabs.splice(tabIndex, 1);
          
          // If closing active tab, switch to adjacent tab
          if (group.activeTabId === tabId) {
            const visibleTabs = group.tabs.filter(t => !t.isHidden);
            if (visibleTabs.length > 0) {
              const nextIndex = Math.min(tabIndex, visibleTabs.length - 1);
              group.activeTabId = visibleTabs[nextIndex]?.id || null;
            } else {
              group.activeTabId = null;
            }
          }
          
          // Helper: merge tabs from multiple groups into primary
          const mergeGroupsToPrimary = (sourceGroups: EditorGroupId[]) => {
            const allTabs: CanvasTab[] = [];
            let activeTabId: string | null = null;

            const currentActiveGroupId = draft.activeGroupId;
            if (sourceGroups.includes(currentActiveGroupId)) {
              const currentGroup = getGroup(draft, currentActiveGroupId);
              const visibleTabs = getVisibleTabs(currentGroup);
              if (currentGroup.activeTabId && visibleTabs.find(t => t.id === currentGroup.activeTabId)) {
                activeTabId = currentGroup.activeTabId;
              }
            }

            for (const sourceGroupId of sourceGroups) {
              const sourceGroup = getGroup(draft, sourceGroupId);
              const visibleTabs = getVisibleTabs(sourceGroup);
              allTabs.push(...visibleTabs);

              if (!activeTabId && sourceGroup.activeTabId && visibleTabs.find(t => t.id === sourceGroup.activeTabId)) {
                activeTabId = sourceGroup.activeTabId;
              }
            }

            draft.primaryGroup.tabs = allTabs;
            draft.primaryGroup.activeTabId = activeTabId || (allTabs.length > 0 ? allTabs[0].id : null);

            draft.secondaryGroup = createEditorGroupState();
            draft.tertiaryGroup = createEditorGroupState();
          };

          // Auto-merge empty editor groups
          if (draft.layout.splitMode === 'grid9') {
            // grid9: all 9 slots stay visible; emptied slots remain as drop
            // targets so the user's free-form placement is preserved.
            for (const gid of EDITOR_GROUP_IDS) {
              ensureValidActiveTab(getGroup(draft, gid));
            }
            // Downgrade: shrink the column/row counts while their trailing
            // activated slots are empty (columns/rows are independent).
            let cols = draft.layout.grid9ColsCount;
            while (cols > 1) {
              const trailingColHasTabs = Array.from({ length: GRID_MAX_DIM }, (_, row) =>
                getVisibleCount(getGroup(draft, EDITOR_GROUP_IDS[row * GRID_MAX_DIM + (cols - 1)])) > 0
              ).some(Boolean);
              if (trailingColHasTabs) break;
              cols -= 1;
            }
            let rows = draft.layout.grid9RowsCount;
            while (rows > 1) {
              const trailingRowHasTabs = Array.from({ length: GRID_MAX_DIM }, (_, col) =>
                getVisibleCount(getGroup(draft, EDITOR_GROUP_IDS[(rows - 1) * GRID_MAX_DIM + col])) > 0
              ).some(Boolean);
              if (trailingRowHasTabs) break;
              rows -= 1;
            }
            draft.layout.grid9ColsCount = cols;
            draft.layout.grid9RowsCount = rows;
            preserveGrid9RatiosOnAxisChange(draft.layout, cols, rows);
            if (getVisibleCount(getGroup(draft, draft.activeGroupId)) === 0) {
              const firstNonEmpty = EDITOR_GROUP_IDS.find(
                gid => getVisibleCount(getGroup(draft, gid)) > 0
              );
              draft.activeGroupId = firstNonEmpty ?? 'primary';
            }
          } else if (draft.layout.splitMode === 'grid') {
            const pCount = getVisibleCount(draft.primaryGroup);
            const sCount = getVisibleCount(draft.secondaryGroup);
            const tCount = getVisibleCount(draft.tertiaryGroup);

            if (tCount === 0 && pCount > 0 && sCount > 0) {
              // Tertiary empty; primary + secondary have tabs -> downgrade to horizontal
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'horizontal';
              if (draft.activeGroupId === 'tertiary') {
                draft.activeGroupId = 'primary';
                ensureValidActiveTab(draft.primaryGroup);
              }
            } else if (tCount === 0 && (pCount === 0 || sCount === 0)) {
              // Tertiary empty and primary/secondary missing -> merge remaining to primary
              const remainingGroups: EditorGroupId[] = [];
              if (pCount > 0) remainingGroups.push('primary');
              if (sCount > 0) remainingGroups.push('secondary');

              if (remainingGroups.length > 0) {
                mergeGroupsToPrimary(remainingGroups);
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
              } else {
                draft.primaryGroup = createEditorGroupState();
                draft.secondaryGroup = createEditorGroupState();
                draft.tertiaryGroup = createEditorGroupState();
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
              }
            } else if (pCount === 0 && sCount === 0 && tCount > 0) {
              // Primary + secondary empty; tertiary has tabs -> merge to primary
              mergeGroupsToPrimary(['tertiary']);
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            } else if (pCount === 0 && sCount > 0) {
              // Primary empty; secondary and tertiary have tabs -> downgrade to vertical
              const sTabs = getVisibleTabs(draft.secondaryGroup);
              const tTabs = getVisibleTabs(draft.tertiaryGroup);

              draft.primaryGroup.tabs = sTabs;
              draft.primaryGroup.activeTabId = draft.secondaryGroup.activeTabId &&
                sTabs.find(t => t.id === draft.secondaryGroup.activeTabId)
                  ? draft.secondaryGroup.activeTabId
                  : (sTabs[0]?.id || null);

              draft.secondaryGroup.tabs = tTabs;
              draft.secondaryGroup.activeTabId = draft.tertiaryGroup.activeTabId &&
                tTabs.find(t => t.id === draft.tertiaryGroup.activeTabId)
                  ? draft.tertiaryGroup.activeTabId
                  : (tTabs[0]?.id || null);

              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'vertical';

              if (draft.activeGroupId === 'secondary') {
                draft.activeGroupId = 'primary';
              } else if (draft.activeGroupId === 'tertiary') {
                draft.activeGroupId = 'secondary';
              }
            } else if (sCount === 0 && pCount > 0) {
              // Secondary empty; primary and tertiary have tabs -> downgrade to vertical
              const tTabs = getVisibleTabs(draft.tertiaryGroup);
              draft.secondaryGroup.tabs = tTabs;
              draft.secondaryGroup.activeTabId = draft.tertiaryGroup.activeTabId &&
                tTabs.find(t => t.id === draft.tertiaryGroup.activeTabId)
                  ? draft.tertiaryGroup.activeTabId
                  : (tTabs[0]?.id || null);

              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'vertical';

              if (draft.activeGroupId === 'tertiary') {
                draft.activeGroupId = 'secondary';
              }
            }

            ensureValidActiveTab(draft.primaryGroup);
            ensureValidActiveTab(draft.secondaryGroup);
            ensureValidActiveTab(draft.tertiaryGroup);
          }
          else if (draft.layout.splitMode === 'horizontal' || draft.layout.splitMode === 'vertical') {
            const pCount = getVisibleCount(draft.primaryGroup);
            const sCount = getVisibleCount(draft.secondaryGroup);
            if (sCount === 0 && pCount > 0) {
              // Secondary empty; primary has tabs -> merge to single column
              draft.secondaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
              ensureValidActiveTab(draft.primaryGroup);
            } else if (pCount === 0 && sCount > 0) {
              // Primary empty; secondary has tabs -> merge to primary
              mergeGroupsToPrimary(['secondary']);
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            } else if (pCount === 0 && sCount === 0) {
              // Both groups are empty
              draft.primaryGroup = createEditorGroupState();
              draft.secondaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            }
          }
          
          // Final check: ensure activeGroupId points to a group with tabs
          if (draft.layout.splitMode === 'grid9') {
            if (getVisibleCount(getGroup(draft, draft.activeGroupId)) === 0) {
              const firstNonEmpty = EDITOR_GROUP_IDS.find(
                gid => getVisibleCount(getGroup(draft, gid)) > 0
              );
              draft.activeGroupId = firstNonEmpty ?? 'primary';
            }
          } else {
            const finalPCount = getVisibleCount(draft.primaryGroup);
            const finalSCount = getVisibleCount(draft.secondaryGroup);
            const finalTCount = getVisibleCount(draft.tertiaryGroup);
            
            if (draft.activeGroupId === 'primary' && finalPCount === 0) {
              // Primary empty; switch to group with tabs
              if (finalSCount > 0) {
                draft.activeGroupId = 'secondary';
              } else if (finalTCount > 0) {
                draft.activeGroupId = 'tertiary';
              }
            } else if (draft.activeGroupId === 'secondary' && finalSCount === 0) {
              // Secondary empty; switch to group with tabs
              if (finalPCount > 0) {
                draft.activeGroupId = 'primary';
              } else if (finalTCount > 0) {
                draft.activeGroupId = 'tertiary';
              }
            } else if (draft.activeGroupId === 'tertiary' && finalTCount === 0) {
              // Tertiary empty; switch to group with tabs
              if (finalPCount > 0) {
                draft.activeGroupId = 'primary';
              } else if (finalSCount > 0) {
                draft.activeGroupId = 'secondary';
              }
            }
          }
        });
      },

      closeTerminalTabBySessionId: (sessionId) => {
        const state = get();
        const result = state.findTabByMetadata({ sessionId });
        if (!result || result.tab.content.type !== 'terminal') return;
        state.closeTab(result.tab.id, result.groupId, { forceRemove: true });
      },

      renameTerminalTabBySessionId: (sessionId, newName) => {
        const result = get().findTabByMetadata({ sessionId });
        if (!result || result.tab.content.type !== 'terminal') return;
        
        set((draft) => {
          const group = getGroup(draft, result.groupId);
          const tab = group.tabs.find(t => t.id === result.tab.id);
          if (tab) {
            const displayTitle = newName.length > 20 ? `${newName.slice(0, 20)}...` : newName;
            tab.title = displayTitle;
            tab.content.title = displayTitle;
            tab.content.data = { ...tab.content.data, sessionName: newName };
          }
        });
      },
      
      closeAllTabs: (groupId) => {
        set((draft) => {
          if (groupId) {
            const group = getGroup(draft, groupId);
            keepPinnedTabsOnly(group);

            const pCount = draft.primaryGroup.tabs.filter(t => !t.isHidden).length;
            const sCount = draft.secondaryGroup.tabs.filter(t => !t.isHidden).length;

            if (draft.layout.splitMode === 'grid9') {
              // grid9: closing one slot keeps all 9 slots (free-form placement
              // is preserved). The emptied slot stays as a drop target.
              ensureValidActiveTab(group);
              if (getVisibleCount(getGroup(draft, draft.activeGroupId)) === 0) {
                const firstNonEmpty = EDITOR_GROUP_IDS.find(
                  gid => getVisibleCount(getGroup(draft, gid)) > 0
                );
                draft.activeGroupId = firstNonEmpty ?? 'primary';
              }
            } else if (draft.layout.splitMode === 'grid') {
              if (groupId === 'tertiary') {
                if (pCount > 0 && sCount > 0) {
                  draft.layout.splitMode = 'horizontal';
                  draft.activeGroupId = 'primary';
                } else if (pCount > 0 || sCount > 0) {
                  draft.primaryGroup = pCount > 0 ? draft.primaryGroup : draft.secondaryGroup;
                  draft.secondaryGroup = createEditorGroupState();
                  draft.tertiaryGroup = createEditorGroupState();
                  draft.layout.splitMode = 'none';
                  draft.activeGroupId = 'primary';
                } else {
                  draft.layout.splitMode = 'none';
                  draft.activeGroupId = 'primary';
                }
              } else {
                // Closing primary or secondary
                const tCount = draft.tertiaryGroup.tabs.filter(t => !t.isHidden).length;
                
                if (groupId === 'primary') {
                  // Closing primary; remaining secondary and/or tertiary
                  if (sCount > 0 && tCount > 0) {
                    // Secondary + tertiary remain -> downgrade to vertical
                    draft.primaryGroup = { ...draft.secondaryGroup };
                    draft.secondaryGroup = { ...draft.tertiaryGroup };
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'vertical';
                    draft.activeGroupId = 'primary';
                  } else if (sCount > 0) {
                    // Only secondary remains
                    draft.primaryGroup = { ...draft.secondaryGroup };
                    draft.secondaryGroup = createEditorGroupState();
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  } else if (tCount > 0) {
                    // Only tertiary remains
                    draft.primaryGroup = { ...draft.tertiaryGroup };
                    draft.secondaryGroup = createEditorGroupState();
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  } else {
                    // All empty
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  }
                } else if (groupId === 'secondary') {
                  // Closing secondary; remaining primary and/or tertiary
                  if (pCount > 0 && tCount > 0) {
                    // Primary + tertiary remain -> downgrade to vertical
                    draft.secondaryGroup = { ...draft.tertiaryGroup };
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'vertical';
                    draft.activeGroupId = 'primary';
                  } else if (pCount > 0) {
                    // Only primary remains
                    draft.secondaryGroup = createEditorGroupState();
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  } else if (tCount > 0) {
                    // Only tertiary remains
                    draft.primaryGroup = { ...draft.tertiaryGroup };
                    draft.secondaryGroup = createEditorGroupState();
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  } else {
                    // All empty
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  }
                }
              }
            } else if (draft.layout.splitMode === 'horizontal' || draft.layout.splitMode === 'vertical') {
              // Handle horizontal/vertical split mode
              if (groupId === 'secondary' && pCount > 0) {
                // Close secondary; primary has tabs -> merge to single column
                draft.secondaryGroup = createEditorGroupState();
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
                // Ensure primary has a valid activeTabId
                const visibleTabs = draft.primaryGroup.tabs.filter(t => !t.isHidden);
                if (visibleTabs.length > 0 && (!draft.primaryGroup.activeTabId || !visibleTabs.find(t => t.id === draft.primaryGroup.activeTabId))) {
                  draft.primaryGroup.activeTabId = visibleTabs[0].id;
                }
              } else if (groupId === 'primary' && sCount > 0) {
                // Close primary; secondary has tabs -> move to primary
                draft.primaryGroup = { ...draft.secondaryGroup };
                draft.secondaryGroup = createEditorGroupState();
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
              } else {
                // Both groups empty or closing the only group with tabs
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
              }
            }
          } else {
            keepPinnedTabsOnly(draft.primaryGroup);
            keepPinnedTabsOnly(draft.secondaryGroup);
            keepPinnedTabsOnly(draft.tertiaryGroup);
            for (const gid of EDITOR_GROUP_IDS) {
              if (gid === 'primary' || gid === 'secondary' || gid === 'tertiary') continue;
              keepPinnedTabsOnly(getGroup(draft, gid));
            }

            const pCount = getVisibleCount(draft.primaryGroup);
            const sCount = getVisibleCount(draft.secondaryGroup);
            const tCount = getVisibleCount(draft.tertiaryGroup);

            if (pCount === 0 && sCount === 0 && tCount === 0) {
              // p/s/t are empty, but slot groups may still hold pinned tabs
              // (kept by keepPinnedTabsOnly above). Collect them into primary
              // before resetting every group, so pinned tabs are never lost.
              const pinnedTabs = EDITOR_GROUP_IDS.flatMap(gid => {
                if (gid === 'primary') return [];
                return getGroup(draft, gid).tabs.filter(t => t.state === 'pinned');
              });
              draft.primaryGroup = createEditorGroupState();
              draft.primaryGroup.tabs = pinnedTabs;
              draft.primaryGroup.activeTabId = pinnedTabs[0]?.id || null;
              draft.secondaryGroup = createEditorGroupState();
              draft.tertiaryGroup = createEditorGroupState();
              for (const gid of EDITOR_GROUP_IDS) {
                if (gid === 'primary' || gid === 'secondary' || gid === 'tertiary') continue;
                (draft as any)[GROUP_STATE_KEY[gid]] = createEditorGroupState();
              }
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            } else if (draft.layout.splitMode === 'grid9') {
              // grid9: all 9 slots persist; just re-validate active tab ids
              for (const gid of EDITOR_GROUP_IDS) {
                ensureValidActiveTab(getGroup(draft, gid));
              }
              if (getVisibleCount(getGroup(draft, draft.activeGroupId)) === 0) {
                const firstNonEmpty = EDITOR_GROUP_IDS.find(
                  gid => getVisibleCount(getGroup(draft, gid)) > 0
                );
                draft.activeGroupId = firstNonEmpty ?? 'primary';
              }
            } else if (draft.layout.splitMode === 'grid') {
              if (pCount > 0 && sCount > 0 && tCount > 0) {
                ensureValidActiveTab(draft.primaryGroup);
                ensureValidActiveTab(draft.secondaryGroup);
                ensureValidActiveTab(draft.tertiaryGroup);
              } else {
                const remainingGroups: EditorGroupState[] = [];
                if (pCount > 0) remainingGroups.push(draft.primaryGroup);
                if (sCount > 0) remainingGroups.push(draft.secondaryGroup);
                if (tCount > 0) remainingGroups.push(draft.tertiaryGroup);

                draft.primaryGroup = remainingGroups[0] ? { ...remainingGroups[0] } : createEditorGroupState();
                draft.secondaryGroup = remainingGroups[1] ? { ...remainingGroups[1] } : createEditorGroupState();
                draft.tertiaryGroup = remainingGroups[2] ? { ...remainingGroups[2] } : createEditorGroupState();
                draft.layout.splitMode = remainingGroups.length >= 3 ? 'grid' : remainingGroups.length === 2 ? 'horizontal' : 'none';
                draft.activeGroupId = 'primary';
                ensureValidActiveTab(draft.primaryGroup);
                ensureValidActiveTab(draft.secondaryGroup);
                ensureValidActiveTab(draft.tertiaryGroup);
              }
            } else if (draft.layout.splitMode === 'horizontal' || draft.layout.splitMode === 'vertical') {
              if (pCount > 0 && sCount > 0) {
                ensureValidActiveTab(draft.primaryGroup);
                ensureValidActiveTab(draft.secondaryGroup);
              } else if (pCount > 0) {
                draft.secondaryGroup = createEditorGroupState();
                draft.tertiaryGroup = createEditorGroupState();
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
                ensureValidActiveTab(draft.primaryGroup);
              } else if (sCount > 0) {
                draft.primaryGroup = { ...draft.secondaryGroup };
                draft.secondaryGroup = createEditorGroupState();
                draft.tertiaryGroup = createEditorGroupState();
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
                ensureValidActiveTab(draft.primaryGroup);
              }
            } else {
              ensureValidActiveTab(draft.primaryGroup);
              draft.activeGroupId = 'primary';
            }
          }
        });
      },
      
      switchToTab: (tabId, groupId) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (!tab) return;
          
          // Unhide if the tab is hidden
          if (tab.isHidden) {
            tab.isHidden = false;
          }
          
          // Update last accessed time
          tab.lastAccessedAt = Date.now();
          
          group.activeTabId = tabId;
          draft.activeGroupId = groupId;
        });
      },
      
      updateTabContent: (tabId, groupId, content) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab) {
            tab.content = content;
            tab.title = content.title || tab.title;
          }
        });
      },
      
      setTabDirty: (tabId, groupId, isDirty) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab) {
            tab.isDirty = isDirty;
          }
        });
      },

      setTabFileDeletedFromDisk: (tabId, groupId, deleted) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          if (tab) {
            tab.fileDeletedFromDisk = deleted;
          }
        });
      },
      
      promoteTab: (tabId, groupId) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab && tab.state === 'preview') {
            tab.state = 'active';
          }
        });
      },
      
      togglePinTab: (tabId, groupId) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab) {
            if (tab.state === 'pinned') {
              tab.state = 'active';
            } else {
              tab.state = 'pinned';
            }

            const tabIndex = group.tabs.findIndex(t => t.id === tabId);
            if (tabIndex !== -1) {
              const [movedTab] = group.tabs.splice(tabIndex, 1);
              insertTabRespectingPinnedBoundary(group, movedTab);
            }
          }
        });
      },
      
      findTabByMetadata: (metadata) => {
        const state = get();
        const groups: { id: EditorGroupId; group: EditorGroupState }[] =
          EDITOR_GROUP_IDS.map(id => ({ id, group: getGroup(state, id) }));
        
        for (const { id, group } of groups) {
          const tab = group.tabs.find(t => {
            if (!t.content.metadata) return false;
            return Object.keys(metadata).every(key => {
              const metadataValue = metadata[key];
              const tabValue = t.content.metadata?.[key];
              if (key === 'duplicateCheckKey' && typeof metadataValue === 'string' && typeof tabValue === 'string') {
                return normalizePath(metadataValue) === normalizePath(tabValue);
              }
              return tabValue === metadataValue;
            });
          });
          if (tab) {
            return { tab, groupId: id };
          }
        }
        return null;
      },
      
      reopenClosedTab: () => {
        set((draft) => {
          const record = draft.closedTabs.shift();
          if (record) {
            const group = getGroup(draft, record.groupId);
            
            // Restore tab to its original position
            const insertIndex = Math.min(record.index, group.tabs.length);
            group.tabs.splice(insertIndex, 0, {
              ...record.tab,
              lastAccessedAt: Date.now(),
            });
            group.activeTabId = record.tab.id;
            draft.activeGroupId = record.groupId;
          }
        });
      },
      
      hideTab: (tabId, groupId) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab) {
            tab.isHidden = true;
            
            if (group.activeTabId === tabId) {
              const visibleTabs = group.tabs.filter(t => !t.isHidden);
              group.activeTabId = visibleTabs[0]?.id || null;
            }
          }
        });
      },
      
      showTab: (tabId, groupId) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab) {
            tab.isHidden = false;
            group.activeTabId = tabId;
          }
        });
      },
      
      // ==================== Drag Operations ====================
      
      startDrag: (tabId, groupId) => {
        set((draft) => {
          draft.draggingTabId = tabId;
          draft.draggingFromGroupId = groupId;
        });
      },
      
      endDrag: () => {
        set((draft) => {
          draft.draggingTabId = null;
          draft.draggingFromGroupId = null;
        });
      },
      
      moveTabToGroup: (tabId, fromGroupId, toGroupId, index) => {
        if (fromGroupId === toGroupId) return;
        
        set((draft) => {
          const fromGroup = getGroup(draft, fromGroupId);
          const toGroup = getGroup(draft, toGroupId);
          
          const tabIndex = fromGroup.tabs.findIndex(t => t.id === tabId);
          if (tabIndex === -1) return;
          
          const [tab] = fromGroup.tabs.splice(tabIndex, 1);
          
          // Add to target group
          const insertIndex = index !== undefined ? Math.min(index, toGroup.tabs.length) : 0;
          toGroup.tabs.splice(insertIndex, 0, tab);
          toGroup.activeTabId = tab.id;
          
          // Update active tab in source group
          if (fromGroup.activeTabId === tabId) {
            const visibleTabs = fromGroup.tabs.filter(t => !t.isHidden);
            fromGroup.activeTabId = visibleTabs[Math.min(tabIndex, visibleTabs.length - 1)]?.id || null;
          }
          
          // If single-column, enable split
          if (draft.layout.splitMode === 'none') {
            draft.layout.splitMode = 'horizontal';
          }
          
          draft.activeGroupId = toGroupId;
        });
      },
      
      reorderTab: (tabId, groupId, newIndex) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tabIndex = group.tabs.findIndex(t => t.id === tabId);
          
          if (tabIndex === -1 || tabIndex === newIndex) return;
          
          const [tab] = group.tabs.splice(tabIndex, 1);
          const pinnedBoundary = getPinnedBoundary(group);
          const targetIndex = tab.state === 'pinned'
            ? Math.max(0, Math.min(newIndex, pinnedBoundary))
            : Math.max(pinnedBoundary, Math.min(newIndex, group.tabs.length));
          group.tabs.splice(targetIndex, 0, tab);
        });
      },
      
      handleDrop: (tabId, fromGroupId, toGroupId, position) => {
        set((draft) => {
          const fromGroup = getGroup(draft, fromGroupId);
          const tabIndex = fromGroup.tabs.findIndex(t => t.id === tabId);
          if (tabIndex === -1) return;

          const [tab] = fromGroup.tabs.splice(tabIndex, 1);

          if (fromGroup.activeTabId === tabId) {
            const visible = fromGroup.tabs.filter(t => !t.isHidden);
            fromGroup.activeTabId = visible[Math.min(tabIndex, visible.length - 1)]?.id || null;
          }

          const { splitMode } = draft.layout;

          if (splitMode === 'none') {
            if (position === 'center') {
              // Original semantics: dropping into the center of the single
              // column just places the tab in the target group (no split
              // upgrade). Keeps the 1-3 dynamic chain intact.
              const targetGroup = getGroup(draft, toGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = toGroupId;
            } else if (position === 'left' || position === 'right') {
              draft.layout.splitMode = 'horizontal';
              if (position === 'left') {
                draft.secondaryGroup.tabs = [...draft.primaryGroup.tabs];
                draft.secondaryGroup.activeTabId = draft.primaryGroup.activeTabId;
                draft.primaryGroup.tabs = [tab];
                draft.primaryGroup.activeTabId = tab.id;
              } else {
                draft.secondaryGroup.tabs = [tab];
                draft.secondaryGroup.activeTabId = tab.id;
              }
              draft.activeGroupId = position === 'left' ? 'primary' : 'secondary';
            } else if (position === 'top' || position === 'bottom') {
              draft.layout.splitMode = 'vertical';
              if (position === 'top') {
                draft.secondaryGroup.tabs = [...draft.primaryGroup.tabs];
                draft.secondaryGroup.activeTabId = draft.primaryGroup.activeTabId;
                draft.primaryGroup.tabs = [tab];
                draft.primaryGroup.activeTabId = tab.id;
              } else {
                draft.secondaryGroup.tabs = [tab];
                draft.secondaryGroup.activeTabId = tab.id;
              }
              draft.activeGroupId = position === 'top' ? 'primary' : 'secondary';
            }
          } else if (splitMode === 'horizontal') {
            if (position === 'bottom') {
              draft.layout.splitMode = 'grid';
              draft.tertiaryGroup.tabs = [tab];
              draft.tertiaryGroup.activeTabId = tab.id;
              draft.activeGroupId = 'tertiary';
            } else if (position === 'top') {
              draft.layout.splitMode = 'grid';
              draft.tertiaryGroup.tabs = [...draft.primaryGroup.tabs, ...draft.secondaryGroup.tabs];
              draft.tertiaryGroup.activeTabId = draft.primaryGroup.activeTabId || draft.secondaryGroup.activeTabId;
              draft.primaryGroup.tabs = [tab];
              draft.primaryGroup.activeTabId = tab.id;
              draft.secondaryGroup = createEditorGroupState();
              draft.activeGroupId = 'primary';
            } else if (position === 'center') {
              const targetGroup = getGroup(draft, toGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = toGroupId;
            } else if (position === 'left' || position === 'right') {
              // Horizontal (2-row) split: dropping on the left/right edge
              // always grows into the grid by adding a column — rows stay
              // as-is, the new column appears on that side. "Drag top/bottom
              // first, then drag left/right" composes freely. The old
              // fromGroupId !== primary/secondary guard was unreachable
              // (horizontal renders only primary/secondary), so it never
              // upgraded — now it always does.
              draft.layout.splitMode = 'grid9';
              draft.layout.grid9ColsCount = 2;
              draft.layout.grid9RowsCount = 2;
              resetGrid9Ratios(draft.layout);
              const targetCol = position === 'left' ? 0 : 1;
              const targetRow = toGroupId === 'secondary' ? 1 : 0;
              const slotId = EDITOR_GROUP_IDS[targetRow * GRID_MAX_DIM + targetCol];
              const slotGroup = getGroup(draft, slotId);
              slotGroup.tabs.unshift(tab);
              slotGroup.activeTabId = tab.id;
              draft.activeGroupId = slotId;
            } else {
              const targetGroupId = position === 'left' ? 'primary' : 'secondary';
              const targetGroup = getGroup(draft, targetGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = targetGroupId;
            }
          } else if (splitMode === 'vertical') {
            if (position === 'center') {
              const targetGroup = getGroup(draft, toGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = toGroupId;
            } else {
              const targetGroupId = position === 'top' ? 'primary' : 'secondary';
              const targetGroup = getGroup(draft, targetGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = targetGroupId;
            }
          } else if (splitMode === 'grid') {
            if (position === 'bottom' && toGroupId === 'tertiary') {
              // Expand the 3-pane (left/right/bottom) into the grid: the
              // dragged tab opens row 1 (rowsCount grows to 2), keeping the
              // existing 2 columns. Rows/columns stay independent. The new
              // cell below tertiary is row1 col1 (slot6 in 4x4 row-major) —
              // computed from the grid geometry, never hardcoded, so it stays
              // correct if GRID_MAX_DIM or the slot layout changes.
              draft.layout.splitMode = 'grid9';
              draft.layout.grid9ColsCount = 2;
              draft.layout.grid9RowsCount = 2;
              resetGrid9Ratios(draft.layout);
              const slotId = EDITOR_GROUP_IDS[1 * GRID_MAX_DIM + 1];
              const slotGroup = getGroup(draft, slotId);
              slotGroup.tabs = [tab];
              slotGroup.activeTabId = tab.id;
              draft.activeGroupId = slotId;
            } else if (position === 'center') {
              const targetGroup = getGroup(draft, toGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = toGroupId;
            }
          } else if (splitMode === 'grid9') {
            // grid9 with independent rows/columns (grid9ColsCount ×
            // grid9RowsCount, each 1..GRID_MAX_DIM). Edge drops grow the
            // corresponding axis; the center drop places the tab into the
            // target slot. Row/col of the target slot (4x4, row-major).
            const targetRow = EDITOR_GROUP_ROW[toGroupId];
            const targetCol = EDITOR_GROUP_COL[toGroupId];
            if (position === 'left' || position === 'right') {
              // Grow the column count toward GRID_MAX_DIM (left/right both add
              // a column) and place the tab in the newly added column at the
              // target row.
              if (draft.layout.grid9ColsCount < GRID_MAX_DIM) {
                draft.layout.grid9ColsCount += 1;
              }
              preserveGrid9RatiosOnAxisChange(
                draft.layout,
                draft.layout.grid9ColsCount,
                draft.layout.grid9RowsCount,
              );
              const newCol = Math.min(draft.layout.grid9ColsCount - 1, GRID_MAX_DIM - 1);
              const slotId = EDITOR_GROUP_IDS[targetRow * GRID_MAX_DIM + newCol];
              const slotGroup = getGroup(draft, slotId);
              slotGroup.tabs.unshift(tab);
              slotGroup.activeTabId = tab.id;
              draft.activeGroupId = slotId;
            } else if (position === 'top' || position === 'bottom') {
              // Grow the row count toward GRID_MAX_DIM (top/bottom both add a
              // row) and place the tab in the newly added row at the target
              // column.
              if (draft.layout.grid9RowsCount < GRID_MAX_DIM) {
                draft.layout.grid9RowsCount += 1;
              }
              preserveGrid9RatiosOnAxisChange(
                draft.layout,
                draft.layout.grid9ColsCount,
                draft.layout.grid9RowsCount,
              );
              const newRow = Math.min(draft.layout.grid9RowsCount - 1, GRID_MAX_DIM - 1);
              const slotId = EDITOR_GROUP_IDS[newRow * GRID_MAX_DIM + targetCol];
              const slotGroup = getGroup(draft, slotId);
              slotGroup.tabs.unshift(tab);
              slotGroup.activeTabId = tab.id;
              draft.activeGroupId = slotId;
            } else {
              // center: place into the target slot (activate it if the slot is
              // outside the current rows/cols — grows that axis implicitly).
              if (targetRow >= draft.layout.grid9RowsCount) {
                draft.layout.grid9RowsCount = targetRow + 1;
              }
              if (targetCol >= draft.layout.grid9ColsCount) {
                draft.layout.grid9ColsCount = targetCol + 1;
              }
              preserveGrid9RatiosOnAxisChange(
                draft.layout,
                draft.layout.grid9ColsCount,
                draft.layout.grid9RowsCount,
              );
              const targetGroup = getGroup(draft, toGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = toGroupId;
            }
          }

          // Auto-merge empty editor groups
          const getVisibleCount = (g: EditorGroupState) => g.tabs.filter(t => !t.isHidden).length;
          const primaryCount = getVisibleCount(draft.primaryGroup);
          const secondaryCount = getVisibleCount(draft.secondaryGroup);
          const tertiaryCount = getVisibleCount(draft.tertiaryGroup);

          if (draft.layout.splitMode === 'grid9') {
            // grid9 keeps all 9 slots; no auto-merge/downgrade. Just re-validate
            // active tab ids and keep activeGroupId on a non-empty group.
            for (const gid of EDITOR_GROUP_IDS) {
              ensureValidActiveTab(getGroup(draft, gid));
            }
            // Downgrade: shrink the column/row counts when trailing slots
            // emptied by the move (rows/columns are independent).
            let cols = draft.layout.grid9ColsCount;
            while (cols > 1) {
              const trailingColHasTabs = Array.from({ length: GRID_MAX_DIM }, (_, row) =>
                getVisibleCount(getGroup(draft, EDITOR_GROUP_IDS[row * GRID_MAX_DIM + (cols - 1)])) > 0
              ).some(Boolean);
              if (trailingColHasTabs) break;
              cols -= 1;
            }
            let rows = draft.layout.grid9RowsCount;
            while (rows > 1) {
              const trailingRowHasTabs = Array.from({ length: GRID_MAX_DIM }, (_, col) =>
                getVisibleCount(getGroup(draft, EDITOR_GROUP_IDS[(rows - 1) * GRID_MAX_DIM + col])) > 0
              ).some(Boolean);
              if (trailingRowHasTabs) break;
              rows -= 1;
            }
            draft.layout.grid9ColsCount = cols;
            draft.layout.grid9RowsCount = rows;
            preserveGrid9RatiosOnAxisChange(draft.layout, cols, rows);
            if (getVisibleCount(getGroup(draft, draft.activeGroupId)) === 0) {
              const firstNonEmpty = EDITOR_GROUP_IDS.find(
                gid => getVisibleCount(getGroup(draft, gid)) > 0
              );
              draft.activeGroupId = firstNonEmpty ?? 'primary';
            }
            return;
          }

          if (draft.layout.splitMode === 'grid') {
            let gridHandled = false;
            
            if (tertiaryCount === 0) {
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'horizontal';
              gridHandled = true;
            }
            if (primaryCount === 0 && secondaryCount === 0) {
              draft.primaryGroup = { ...draft.tertiaryGroup };
              draft.secondaryGroup = createEditorGroupState();
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
              gridHandled = true;
            }
            // FIX: handle primary empty while secondary and tertiary have tabs
            if (primaryCount === 0 && secondaryCount > 0 && tertiaryCount > 0) {
              // Move secondary -> primary (top), tertiary -> secondary (bottom), downgrade to vertical
              // Tabs are dropped to "bottom", so final layout should be vertical
              draft.primaryGroup = { ...draft.secondaryGroup };
              draft.secondaryGroup = { ...draft.tertiaryGroup };
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'vertical';
              // If active group is tertiary, update to secondary
              if (draft.activeGroupId === 'tertiary') {
                draft.activeGroupId = 'secondary';
              }
              gridHandled = true;
            }
            // FIX: handle secondary empty while primary and tertiary have tabs
            if (secondaryCount === 0 && primaryCount > 0 && tertiaryCount > 0) {
              // Move tertiary -> secondary, downgrade to vertical
              // Primary (top-left) and tertiary (bottom) are vertical
              draft.secondaryGroup = { ...draft.tertiaryGroup };
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'vertical';
              // If active group is tertiary, update to secondary
              if (draft.activeGroupId === 'tertiary') {
                draft.activeGroupId = 'secondary';
              }
              gridHandled = true;
            }
            
            // If grid handling finished, skip horizontal/vertical checks
            if (gridHandled) {
              return;
            }
          }

          if (draft.layout.splitMode === 'horizontal' || draft.layout.splitMode === 'vertical') {
            if (secondaryCount === 0) {
              draft.secondaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            } else if (primaryCount === 0) {
              draft.primaryGroup = { ...draft.secondaryGroup };
              draft.secondaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            }
          }
        });
      },
      
      // ==================== Layout Operations ====================
      
      setSplitMode: (mode) => {
        set((draft) => {
          if (mode === 'none' && draft.layout.splitMode !== 'none') {
            const allTabs = EDITOR_GROUP_IDS.flatMap(gid =>
              getGroup(draft, gid).tabs
            );
            draft.primaryGroup.tabs = allTabs;
            draft.primaryGroup.activeTabId =
              draft.primaryGroup.activeTabId ||
              draft.secondaryGroup.activeTabId ||
              draft.tertiaryGroup.activeTabId;
            for (const gid of EDITOR_GROUP_IDS) {
              if (gid !== 'primary') {
                (draft as any)[GROUP_STATE_KEY[gid]] = createEditorGroupState();
              }
            }
            draft.activeGroupId = 'primary';
          }
          draft.layout.splitMode = mode;
        });
      },

      // ==================== Grid9 templates ====================

      /**
       * Apply a preset grid9 template: 2x2 (four-cell), 2x3 / 3x2 (six-cell),
       * 3x3 (nine-cell) or 4x4 (sixteen-cell). Sets splitMode to grid9 and the
       * active row/column counts; the EditorArea renders exactly rows×cols
       * active cells. Existing tabs stay in place; empty slots render as drop
       * targets.
       */
      applyGrid9Template: (cols, rows) => {
        set((draft) => {
          const c = Math.min(GRID_MAX_DIM, Math.max(1, Math.round(cols)));
          const r = Math.min(GRID_MAX_DIM, Math.max(1, Math.round(rows)));
          draft.layout.splitMode = 'grid9';
          draft.layout.grid9ColsCount = c;
          draft.layout.grid9RowsCount = r;
          // A template always tiles evenly: reset any leftover ratios and the
          // user-adjust flag (d7-P2-7 keeps templates as the explicit "re-tile"
          // control; cell add/remove below preserves user-adjusted shares).
          resetGrid9Ratios(draft.layout);
          // Move tabs from slots outside the new template into the primary
          // group (first valid slot) instead of silently discarding them.
          const orphanedTabs: EditorGroupState['tabs'] = [];
          EDITOR_GROUP_IDS.forEach((gid, idx) => {
            const row = Math.floor(idx / GRID_MAX_DIM);
            const col = idx % GRID_MAX_DIM;
            if (row >= r || col >= c) {
              const slot = getGroup(draft, gid);
              if (slot.tabs.length > 0) {
                orphanedTabs.push(...slot.tabs);
                if (slot.activeTabId && orphanedTabs.some(t => t.id === slot.activeTabId)) {
                  draft.primaryGroup.activeTabId = slot.activeTabId;
                }
              }
              (draft as any)[GROUP_STATE_KEY[gid]] = createEditorGroupState();
            }
          });
          if (orphanedTabs.length > 0) {
            draft.primaryGroup.tabs = [...draft.primaryGroup.tabs, ...orphanedTabs];
          }
          // H1: ensure activeGroupId points at a slot inside the new template.
          const activeIdx = EDITOR_GROUP_IDS.indexOf(draft.activeGroupId);
          const activeRow = Math.floor(activeIdx / GRID_MAX_DIM);
          const activeCol = activeIdx % GRID_MAX_DIM;
          if (
            !draft.activeGroupId ||
            activeIdx < 0 ||
            activeRow >= r ||
            activeCol >= c ||
            (draft as any)[GROUP_STATE_KEY[draft.activeGroupId]] === undefined
          ) {
            draft.activeGroupId = 'primary';
          }
          if (draft.primaryGroup.tabs.length > 0 && !draft.primaryGroup.activeTabId) {
            draft.primaryGroup.activeTabId = draft.primaryGroup.tabs[0].id;
          }
        });
      },

      /**
       * Merge two grid9 cells: all tabs from `fromGroupId` move into
       * `toGroupId` (kept at the end), and `fromGroupId` is emptied. This is
       * the "merge two small windows into one big window" primitive that, with
       * the free split/drop creation, gives fully free arrangement. The grid
       * dimensions are kept as-is; the emptied cell simply becomes an empty
       * drop target again.
       */
      mergeGrid9Cells: (fromGroupId, toGroupId) => {
        set((draft) => {
          if (fromGroupId === toGroupId) return;
          const from = getGroup(draft, fromGroupId);
          const to = getGroup(draft, toGroupId);
          if (from.tabs.length === 0) return;
          // Move all tabs (visible first, then hidden) into the target.
          const moved = [...from.tabs];
          to.tabs = [...to.tabs, ...moved];
          if (from.tabs.some(t => t.id === from.activeTabId)) {
            to.activeTabId = from.activeTabId;
          }
          from.tabs = [];
          from.activeTabId = null;
          draft.activeGroupId = toGroupId;
        });
      },

      /**
       * Remove a blank grid9 cell: the grid shrinks by one column (preferred)
       * or one row so the remaining cells re-tile to fill the panel (the
       * user's "delete an empty cell, the rest adapt and fill"). The removed
       * cell's column/row is removed — tabs in it are merged into the left
       * neighbour (or, for the first column, the right neighbour) so no tab
       * is ever dropped and no surviving layout is destroyed: columns/rows
       * right of (or below) the removed one shift in to fill the gap.
       */
      removeGrid9Cell: (groupId) => {
        set((draft) => {
          const idx = EDITOR_GROUP_IDS.indexOf(groupId);
          if (idx < 0) return;
          const row = Math.floor(idx / GRID_MAX_DIM);
          const col = idx % GRID_MAX_DIM;
          const cols = draft.layout.grid9ColsCount;
          const rows = draft.layout.grid9RowsCount;
          // Only a 1×1 grid cannot shrink any further (matches canRemoveCell).
          if (draft.layout.splitMode !== 'grid9' || (cols <= 1 && rows <= 1)) return;

          const moveAllTabs = (fromGid: EditorGroupId, toGid: EditorGroupId) => {
            const from = getGroup(draft, fromGid);
            const to = getGroup(draft, toGid);
            if (from.tabs.length === 0) return;
            if (from.tabs.some(t => t.id === from.activeTabId)) {
              to.activeTabId = from.activeTabId;
            }
            to.tabs = [...to.tabs, ...from.tabs];
            from.tabs = [];
            from.activeTabId = null;
          };
          const resetGroup = (gid: EditorGroupId) => {
            (draft as any)[GROUP_STATE_KEY[gid]] = createEditorGroupState();
          };

          if (cols > 1) {
            // Remove column `col` (keep rows).
            const mergeTargetCol = col > 0 ? col - 1 : 1;
            for (let r = 0; r < rows; r++) {
              moveAllTabs(EDITOR_GROUP_IDS[r * GRID_MAX_DIM + col], EDITOR_GROUP_IDS[r * GRID_MAX_DIM + mergeTargetCol]);
            }
            // Shift columns right of the removed one left by one.
            for (let r = 0; r < rows; r++) {
              for (let c = col === 0 ? 0 : col; c < cols - 1; c++) {
                moveAllTabs(EDITOR_GROUP_IDS[r * GRID_MAX_DIM + c + 1], EDITOR_GROUP_IDS[r * GRID_MAX_DIM + c]);
              }
              resetGroup(EDITOR_GROUP_IDS[r * GRID_MAX_DIM + cols - 1]);
            }
            draft.layout.grid9ColsCount = cols - 1;
          } else {
            // Remove row `row` (keep columns).
            const mergeTargetRow = row > 0 ? row - 1 : 1;
            for (let c = 0; c < cols; c++) {
              moveAllTabs(EDITOR_GROUP_IDS[row * GRID_MAX_DIM + c], EDITOR_GROUP_IDS[mergeTargetRow * GRID_MAX_DIM + c]);
            }
            // Shift rows below the removed one up by one.
            for (let c = 0; c < cols; c++) {
              for (let r = row === 0 ? 0 : row; r < rows - 1; r++) {
                moveAllTabs(EDITOR_GROUP_IDS[(r + 1) * GRID_MAX_DIM + c], EDITOR_GROUP_IDS[r * GRID_MAX_DIM + c]);
              }
              resetGroup(EDITOR_GROUP_IDS[(rows - 1) * GRID_MAX_DIM + c]);
            }
            draft.layout.grid9RowsCount = rows - 1;
          }

          // Reset any slot outside the new template (defensive, layout was
          // already shifted above).
          const newCols = draft.layout.grid9ColsCount;
          const newRows = draft.layout.grid9RowsCount;
          // Keep user-adjusted ratios after the shrink (d7-P2-7); fall back to
          // even tiles when the user never resized.
          preserveGrid9RatiosOnAxisChange(draft.layout, newCols, newRows);
          for (let r = 0; r < GRID_MAX_DIM; r++) {
            for (let c = 0; c < GRID_MAX_DIM; c++) {
              if (r >= newRows || c >= newCols) {
                resetGroup(EDITOR_GROUP_IDS[r * GRID_MAX_DIM + c]);
              }
            }
          }
          // H1: keep activeGroupId inside the new template.
          const activeIdx = EDITOR_GROUP_IDS.indexOf(draft.activeGroupId);
          const ar = Math.floor(activeIdx / GRID_MAX_DIM);
          const ac = activeIdx % GRID_MAX_DIM;
          if (activeIdx < 0 || ar >= newRows || ac >= newCols) {
            draft.activeGroupId = 'primary';
          }
          if (draft.primaryGroup.tabs.length > 0 && !draft.primaryGroup.activeTabId) {
            draft.primaryGroup.activeTabId = draft.primaryGroup.tabs[0].id;
          }
        });
      },
      
      setSplitRatio: (ratio) => {
        set((draft) => {
          draft.layout.splitRatio = clampSplitRatio(ratio);
        });
      },

      setSplitRatio2: (ratio) => {
        set((draft) => {
          draft.layout.splitRatio2 = clampSplitRatio(ratio);
        });
      },

      setGrid9ColRatio: (col, ratio) => {
        set((draft) => {
          if (col >= 0 && col < GRID_MAX_DIM) {
            draft.layout.grid9Cols[col] = clampGrid9Ratio(ratio);
            draft.layout.grid9RatiosUserAdjusted = true;
          }
        });
      },

      setGrid9RowRatio: (row, ratio) => {
        set((draft) => {
          if (row >= 0 && row < GRID_MAX_DIM) {
            draft.layout.grid9Rows[row] = clampGrid9Ratio(ratio);
            draft.layout.grid9RatiosUserAdjusted = true;
          }
        });
      },
      
      setAnchorPosition: (position) => {
        set((draft) => {
          draft.layout.anchorPosition = position;
        });
      },
      
      setAnchorSize: (size) => {
        set((draft) => {
          draft.layout.anchorSize = clampAnchorSize(size);
        });
      },
      
      toggleMaximize: () => {
        set((draft) => {
          draft.layout.isMaximized = !draft.layout.isMaximized;
        });
      },
      
      setActiveGroup: (groupId) => {
        set((draft) => {
          draft.activeGroupId = groupId;
        });
      },
      
      // ==================== Mission Control ====================
      
      openMissionControl: () => {
        set((draft) => {
          draft.isMissionControlOpen = true;
        });
      },
      
      closeMissionControl: () => {
        set((draft) => {
          draft.isMissionControlOpen = false;
        });
      },
      
      toggleMissionControl: () => {
        set((draft) => {
          draft.isMissionControlOpen = !draft.isMissionControlOpen;
        });
      },
      
      // ==================== State Management ====================
      
      reset: () => {
        set(initialState);
      },
      
      getAllTabs: () => {
        const state = get();
        return EDITOR_GROUP_IDS.flatMap(gid => getGroup(state, gid).tabs);
      },
    }))
);

export type CanvasStoreMode = 'agent' | 'project' | 'git' | 'panel-view' | 'bottom-terminal';

/**
 * Selects which canvas store instance is used by the current subtree.
 * Defaults to 'agent' to preserve existing behavior in AI Agent scene.
 */
export const CanvasStoreModeContext = createContext<CanvasStoreMode>('agent');

export const useAgentCanvasStore = createCanvasStoreHook();
export const useProjectCanvasStore = createCanvasStoreHook();
export const useGitCanvasStore = createCanvasStoreHook();
export const usePanelViewCanvasStore = createCanvasStoreHook();
export const useBottomTerminalCanvasStore = createCanvasStoreHook();

// ==================== Agent canvas: per-workspace snapshots (AuxPane / Session scene) ====================
// Switching active workspace saves the current agent canvas under the previous workspace id and restores
// the snapshot for the next id, so remote/local tabs coexist across workspace switches.

const AGENT_CANVAS_SNAPSHOT_MAX = 12;
const agentWorkspaceSnapshots = new Map<string, CanvasStoreState>();
const agentSnapshotLruOrder: string[] = [];
/** Dedupes React Strict Mode double-invoke when `prev` is null (ref reset on remount). */
let lastAgentCanvasSwitchTargetKey: string | null = null;

function normalizeAgentWorkspaceKey(id: string | null | undefined): string {
  return id ?? '__none__';
}

function extractAgentPersistableState(state: CanvasStore): CanvasStoreState {
  return {
    primaryGroup: state.primaryGroup,
    secondaryGroup: state.secondaryGroup,
    tertiaryGroup: state.tertiaryGroup,
    slot4Group: state.slot4Group,
    slot5Group: state.slot5Group,
    slot6Group: state.slot6Group,
    slot7Group: state.slot7Group,
    slot8Group: state.slot8Group,
    slot9Group: state.slot9Group,
    slot10Group: state.slot10Group,
    slot11Group: state.slot11Group,
    slot12Group: state.slot12Group,
    slot13Group: state.slot13Group,
    slot14Group: state.slot14Group,
    slot15Group: state.slot15Group,
    slot16Group: state.slot16Group,
    activeGroupId: state.activeGroupId,
    layout: state.layout,
    isMissionControlOpen: state.isMissionControlOpen,
    draggingTabId: state.draggingTabId,
    draggingFromGroupId: state.draggingFromGroupId,
    closedTabs: state.closedTabs,
    maxClosedTabsHistory: state.maxClosedTabsHistory,
  };
}

function rememberAgentSnapshot(key: string, snapshot: CanvasStoreState): void {
  const clone = structuredClone(snapshot);
  clone.draggingTabId = null;
  clone.draggingFromGroupId = null;
  agentWorkspaceSnapshots.set(key, clone);
  const idx = agentSnapshotLruOrder.indexOf(key);
  if (idx >= 0) agentSnapshotLruOrder.splice(idx, 1);
  agentSnapshotLruOrder.push(key);
  while (agentWorkspaceSnapshots.size > AGENT_CANVAS_SNAPSHOT_MAX) {
    const evict = agentSnapshotLruOrder.shift();
    if (!evict) break;
    agentWorkspaceSnapshots.delete(evict);
  }
}

function applyEmptyAgentCanvas(): void {
  useAgentCanvasStore.setState({
    ...initialState,
    activeGroupId: 'primary',
    layout: createLayoutState(),
    isMissionControlOpen: false,
    draggingTabId: null,
    draggingFromGroupId: null,
    closedTabs: [],
  });
}

/** Clear agent canvas workspace snapshots when entering/exiting Peer Device Mode. */
export function clearAgentCanvasForPeerSwitch(): void {
  agentWorkspaceSnapshots.clear();
  agentSnapshotLruOrder.length = 0;
  lastAgentCanvasSwitchTargetKey = null;
  applyEmptyAgentCanvas();
  useProjectCanvasStore.getState().reset();
  useGitCanvasStore.getState().reset();
  usePanelViewCanvasStore.getState().reset();
  useBottomTerminalCanvasStore.getState().reset();
}

/**
 * Save the current agent canvas under `prevWorkspaceId` (unless first mount) and restore the snapshot
 * for `nextWorkspaceId` (or empty canvas if none). Capture target snapshot before LRU eviction.
 */
export function switchAgentCanvasWorkspace(
  prevWorkspaceId: string | null | undefined,
  nextWorkspaceId: string | null | undefined
): void {
  const from =
    prevWorkspaceId === null || prevWorkspaceId === undefined
      ? null
      : normalizeAgentWorkspaceKey(prevWorkspaceId);
  const to = normalizeAgentWorkspaceKey(nextWorkspaceId);

  if (from === null && lastAgentCanvasSwitchTargetKey === to) {
    return;
  }

  const rawNext = agentWorkspaceSnapshots.get(to);
  const nextSnapshotClone = rawNext ? structuredClone(rawNext) : null;

  if (from !== null) {
    const current = extractAgentPersistableState(useAgentCanvasStore.getState() as CanvasStore);
    rememberAgentSnapshot(from, current);
  }

  if (nextSnapshotClone) {
    useAgentCanvasStore.setState({
      primaryGroup: nextSnapshotClone.primaryGroup,
      secondaryGroup: nextSnapshotClone.secondaryGroup,
      tertiaryGroup: nextSnapshotClone.tertiaryGroup,
      slot4Group: nextSnapshotClone.slot4Group,
      slot5Group: nextSnapshotClone.slot5Group,
      slot6Group: nextSnapshotClone.slot6Group,
      slot7Group: nextSnapshotClone.slot7Group,
      slot8Group: nextSnapshotClone.slot8Group,
      slot9Group: nextSnapshotClone.slot9Group,
      slot10Group: nextSnapshotClone.slot10Group,
      slot11Group: nextSnapshotClone.slot11Group,
      slot12Group: nextSnapshotClone.slot12Group,
      slot13Group: nextSnapshotClone.slot13Group,
      slot14Group: nextSnapshotClone.slot14Group,
      slot15Group: nextSnapshotClone.slot15Group,
      slot16Group: nextSnapshotClone.slot16Group,
      activeGroupId: nextSnapshotClone.activeGroupId,
      layout: nextSnapshotClone.layout,
      isMissionControlOpen: false,
      draggingTabId: null,
      draggingFromGroupId: null,
      closedTabs: nextSnapshotClone.closedTabs,
      maxClosedTabsHistory: nextSnapshotClone.maxClosedTabsHistory,
    });
  } else {
    applyEmptyAgentCanvas();
  }

  lastAgentCanvasSwitchTargetKey = to;
}

/** Drop cached canvas for a closed workspace (does not touch the live canvas unless user switches back). */
export function removeAgentCanvasSnapshot(workspaceId: string): void {
  const key = normalizeAgentWorkspaceKey(workspaceId);
  agentWorkspaceSnapshots.delete(key);
  const idx = agentSnapshotLruOrder.indexOf(key);
  if (idx >= 0) agentSnapshotLruOrder.splice(idx, 1);
}

const selectWholeCanvasStore = (state: CanvasStore) => state;

export function useCanvasStore(): CanvasStore;
export function useCanvasStore<T>(selector: (state: CanvasStore) => T): T;
export function useCanvasStore<T>(selector?: (state: CanvasStore) => T): T | CanvasStore {
  const mode = useContext(CanvasStoreModeContext);
  const resolvedSelector = (selector ?? selectWholeCanvasStore) as (state: CanvasStore) => T | CanvasStore;

  // Keep hook order stable across mode switches by subscribing to each scoped store.
  const agentValue = useAgentCanvasStore(resolvedSelector);
  const projectValue = useProjectCanvasStore(resolvedSelector);
  const gitValue = useGitCanvasStore(resolvedSelector);
  const panelViewValue = usePanelViewCanvasStore(resolvedSelector);
  const bottomTerminalValue = useBottomTerminalCanvasStore(resolvedSelector);

  if (mode === 'project') return projectValue;
  if (mode === 'git') return gitValue;
  if (mode === 'panel-view') return panelViewValue;
  if (mode === 'bottom-terminal') return bottomTerminalValue;
  return agentValue;
}

// ==================== Selector Hooks ====================

/**
 * Get tabs for a specific editor group.
 */
export const useGroupTabs = (groupId: EditorGroupId) => {
  return useCanvasStore((state) =>
    getGroup(state, groupId).tabs
  );
};

/**
 * Get active tab ID for a specific editor group.
 */
export const useActiveTabId = (groupId: EditorGroupId) => {
  return useCanvasStore((state) =>
    getGroup(state, groupId).activeTabId
  );
};

/**
 * Get layout state.
 */
export const useLayout = () => {
  return useCanvasStore((state) => state.layout);
};

/**
 * Get drag state.
 */
export const useDragging = () => {
  return useCanvasStore((state) => ({
    draggingTabId: state.draggingTabId,
    draggingFromGroupId: state.draggingFromGroupId,
  }));
};
