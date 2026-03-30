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
import { loadPanelWidth, STORAGE_KEYS, RIGHT_PANEL_CONFIG } from '@/app/layout/panelConfig';
interface UsePanelTabCoordinatorOptions {
  /** Auto-collapse when all tabs are closed */
  autoCollapseOnEmpty?: boolean;
  /** Auto-expand when a tab opens */
  autoExpandOnTabOpen?: boolean;
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
  } = options;

  const {
    primaryGroup,
    secondaryGroup,
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
    if (state?.layout) {
      rightPanelCollapsedRef.current = state.layout.rightPanelCollapsed ?? true;
    }
    toggleRightPanelRef.current = toggleRightPanel;
    // Mark initialized
    if (!isInitializedRef.current) {
      isInitializedRef.current = true;
    }
  }, [state?.layout, toggleRightPanel]);

  /**
   * Expand right panel (with debounce and state checks).
   * Set width first, then expand to avoid flicker.
   */
  const expandPanel = useCallback(() => {
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
  }, [updateRightPanelWidth]);

  /**
   * Collapse right panel (with debounce and state checks).
   */
  const collapsePanel = useCallback(() => {
    if (!rightPanelCollapsedRef.current && toggleRightPanelRef.current) {
      requestAnimationFrame(() => {
        if (toggleRightPanelRef.current) {
          toggleRightPanelRef.current();
        }
      });
    }
  }, []);

  /**
   * Watch tab count changes and manage panel state.
   */
  useEffect(() => {
    // Wait until initialization completes
    if (!isInitializedRef.current) {
      return;
    }

    // Count visible tabs
    const primaryVisible = primaryGroup.tabs.filter(t => !t.isHidden).length;
    const secondaryVisible = secondaryGroup.tabs.filter(t => !t.isHidden).length;
    const visibleCount = primaryVisible + secondaryVisible;
    
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

    window.addEventListener(TAB_EVENTS.EXPAND_RIGHT_PANEL, handleExpandRightPanel);

    return () => {
      window.removeEventListener(TAB_EVENTS.EXPAND_RIGHT_PANEL, handleExpandRightPanel);
    };
  }, [autoExpandOnTabOpen, expandPanel]);

  return {
    // Utilities
    expandPanel,
    collapsePanel,
  };
};

export default usePanelTabCoordinator;
