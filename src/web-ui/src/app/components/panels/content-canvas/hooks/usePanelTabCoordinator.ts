/**
 * usePanelTabCoordinator Hook
 * Tab and panel state coordinator.
 *
 * Responsibilities:
 * 1. Watch tab count changes and manage panel expand/collapse
 * 2. Sync panel state on tab open/close
 * 3. Ensure state consistency and avoid race conditions
 */

import { useEffect, useRef, useCallback } from 'react';
import { useCanvasStore } from '../stores';
import { useApp } from '@/app/hooks/useApp';
import { TAB_EVENTS } from '../types';
import { EDITOR_GROUP_IDS } from '../types/layout';
import { loadPanelWidth, STORAGE_KEYS, RIGHT_PANEL_CONFIG } from '@/app/layout/panelConfig';
interface UsePanelTabCoordinatorOptions {
  /** Auto-collapse when all tabs are closed */
  autoCollapseOnEmpty?: boolean;
  /** Auto-expand when a tab opens */
  autoExpandOnTabOpen?: boolean;
  /** Custom collapsed state for non-right-panel canvases. */
  isCollapsed?: boolean;
  /** Custom expand behavior for non-right-panel canvases. */
  onExpand?: () => void;
  /** Custom collapse behavior for non-right-panel canvases. */
  onCollapse?: () => void;
  /** Event that requests panel expansion. Defaults to right-panel expansion. */
  expandEventName?: string;
}

/**
 * Tab and panel state coordinator.
 *
 * Design principles:
 * 1. Single source of truth: tabs from canvasStore, panels from useApp
 * 2. Reactive sync: update panel state on tab changes
 * 3. Event-driven: coordinate through unified events
 * 4. Debounce: avoid frequent state updates
 */
export const usePanelTabCoordinator = (options: UsePanelTabCoordinatorOptions = {}) => {
  const {
    autoCollapseOnEmpty = true,
    autoExpandOnTabOpen = true,
    isCollapsed,
    onExpand,
    onCollapse,
    expandEventName = TAB_EVENTS.EXPAND_RIGHT_PANEL,
  } = options;

  const {
    primaryGroup,
    secondaryGroup,
    tertiaryGroup,
    slot4Group,
    slot5Group,
    slot6Group,
    slot7Group,
    slot8Group,
    slot9Group,
    slot10Group,
    slot11Group,
    slot12Group,
    slot13Group,
    slot14Group,
    slot15Group,
    slot16Group,
  } = useCanvasStore();

  const { state, toggleRightPanel, updateRightPanelWidth } = useApp();

  // Use refs to avoid stale closures and add guards
  const rightPanelCollapsedRef = useRef(
    state?.layout?.rightPanelCollapsed ?? true
  );
  const toggleRightPanelRef = useRef(toggleRightPanel);
  const isInitializedRef = useRef(false);

  // Sync refs
  useEffect(() => {
    if (typeof isCollapsed === 'boolean') {
      rightPanelCollapsedRef.current = isCollapsed;
    } else if (state?.layout) {
      rightPanelCollapsedRef.current = state.layout.rightPanelCollapsed ?? true;
    }
    toggleRightPanelRef.current = toggleRightPanel;
    // Mark initialized
    if (!isInitializedRef.current) {
      isInitializedRef.current = true;
    }
  }, [state?.layout, toggleRightPanel, isCollapsed]);

  /**
   * Expand right panel (with debounce and state checks).
   * Set width first, then expand to avoid flicker.
   */
  const expandPanel = useCallback(() => {
    if (onExpand) {
      onExpand();
      return;
    }

    if (rightPanelCollapsedRef.current && toggleRightPanelRef.current && updateRightPanelWidth) {
      // Restore last width if available, otherwise use default
      const lastWidth = loadPanelWidth(STORAGE_KEYS.RIGHT_PANEL_LAST_WIDTH, RIGHT_PANEL_CONFIG.COMFORTABLE_DEFAULT);
      updateRightPanelWidth(lastWidth);
      
      // Expand immediately without animation (notify WorkspaceLayout)
      window.dispatchEvent(new CustomEvent('expand-right-panel-immediate', { 
        detail: { noAnimation: true } 
      }));
      
      // Use requestAnimationFrame to run on next render
      requestAnimationFrame(() => {
        if (toggleRightPanelRef.current) {
          toggleRightPanelRef.current();
        }
      });
    }
  }, [onExpand, updateRightPanelWidth]);

  /**
   * Collapse right panel (with debounce and state checks).
   */
  const collapsePanel = useCallback(() => {
    if (onCollapse) {
      onCollapse();
      return;
    }

    if (!rightPanelCollapsedRef.current && toggleRightPanelRef.current) {
      requestAnimationFrame(() => {
        if (toggleRightPanelRef.current) {
          toggleRightPanelRef.current();
        }
      });
    }
  }, [onCollapse]);

  /**
   * Watch tab count changes and manage panel state.
   */
  useEffect(() => {
    // Wait until initialization completes
    if (!isInitializedRef.current) {
      return;
    }

    // Count visible tabs across all 16 editor groups (legacy primary/secondary/
    // tertiary plus the grid9 extension slots slot4..slot16). Counting only the
    // first three groups let grid9 windows be wrongly auto-collapsed while
    // their tabs were still open (d7-P1-1).
    const groupStates = {
      primary: primaryGroup,
      secondary: secondaryGroup,
      tertiary: tertiaryGroup,
      slot4: slot4Group,
      slot5: slot5Group,
      slot6: slot6Group,
      slot7: slot7Group,
      slot8: slot8Group,
      slot9: slot9Group,
      slot10: slot10Group,
      slot11: slot11Group,
      slot12: slot12Group,
      slot13: slot13Group,
      slot14: slot14Group,
      slot15: slot15Group,
      slot16: slot16Group,
    };
    const visibleCount = EDITOR_GROUP_IDS.reduce(
      (sum, gid) => sum + groupStates[gid].tabs.filter((tab) => !tab.isHidden).length,
      0,
    );
    
    const isCollapsed = rightPanelCollapsedRef.current;

    // Auto-collapse when all tabs are closed
    if (visibleCount === 0 && autoCollapseOnEmpty && !isCollapsed) {
      collapsePanel();
    }
    // Auto-expand when tabs exist
    else if (visibleCount > 0 && autoExpandOnTabOpen && isCollapsed) {
      expandPanel();
    }
  }, [
    primaryGroup.tabs,
    secondaryGroup.tabs,
    tertiaryGroup.tabs,
    slot4Group.tabs,
    slot5Group.tabs,
    slot6Group.tabs,
    slot7Group.tabs,
    slot8Group.tabs,
    slot9Group.tabs,
    slot10Group.tabs,
    slot11Group.tabs,
    slot12Group.tabs,
    slot13Group.tabs,
    slot14Group.tabs,
    slot15Group.tabs,
    slot16Group.tabs,
    autoCollapseOnEmpty,
    autoExpandOnTabOpen,
    expandPanel,
    collapsePanel,
  ]);

  /**
   * Listen for expand-right-panel events.
   * Backup mechanism to ensure panel expands for all tab-open paths.
   */
  useEffect(() => {
    const handleExpandRightPanel = () => {
      if (autoExpandOnTabOpen) {
        expandPanel();
      }
    };

    window.addEventListener(expandEventName, handleExpandRightPanel);

    return () => {
      window.removeEventListener(expandEventName, handleExpandRightPanel);
    };
  }, [autoExpandOnTabOpen, expandPanel, expandEventName]);

  return {
    // Utilities
    expandPanel,
    collapsePanel,
  };
};

export default usePanelTabCoordinator;
