/**
 * useLayoutState Hook
 * Layout state management.
 */

import { useCallback } from 'react';
import { useCanvasStore } from '../stores';
import type { SplitMode, AnchorPosition } from '../types';
import { LAYOUT_CONFIG } from '../types';
interface UseLayoutStateReturn {
  // State
  splitMode: SplitMode;
  splitRatio: number;
  anchorPosition: AnchorPosition;
  anchorSize: number;
  isMaximized: boolean;
  
  // Actions
  setSplitMode: (mode: SplitMode) => void;
  setSplitRatio: (ratio: number) => void;
  setAnchorPosition: (position: AnchorPosition) => void;
  setAnchorSize: (size: number) => void;
  toggleMaximize: () => void;
  
  // Helpers
  enableHorizontalSplit: () => void;
  enableVerticalSplit: () => void;
  disableSplit: () => void;
  toggleSplit: () => void;
  showAnchor: (position?: AnchorPosition) => void;
  hideAnchor: () => void;
  toggleAnchor: () => void;
  resetLayout: () => void;
}

/**
 * Layout state management hook.
 */
export const useLayoutState = (): UseLayoutStateReturn => {
  const {
    layout,
    setSplitMode,
    setSplitRatio,
    setAnchorPosition,
    setAnchorSize,
    toggleMaximize,
  } = useCanvasStore();

  // Enable horizontal split
  const enableHorizontalSplit = useCallback(() => {
    setSplitMode('horizontal');
  }, [setSplitMode]);

  // Enable vertical split
  const enableVerticalSplit = useCallback(() => {
    setSplitMode('vertical');
  }, [setSplitMode]);

  // Disable split
  const disableSplit = useCallback(() => {
    setSplitMode('none');
  }, [setSplitMode]);

  // Toggle split
  const toggleSplit = useCallback(() => {
    if (layout.splitMode === 'none') {
      setSplitMode('horizontal');
    } else {
      setSplitMode('none');
    }
  }, [layout.splitMode, setSplitMode]);

  // Show anchor
  const showAnchor = useCallback((position: AnchorPosition = 'bottom') => {
    setAnchorPosition(position);
  }, [setAnchorPosition]);

  // Hide anchor
  const hideAnchor = useCallback(() => {
    setAnchorPosition('hidden');
  }, [setAnchorPosition]);

  // Toggle anchor
  const toggleAnchor = useCallback(() => {
    if (layout.anchorPosition === 'hidden') {
      setAnchorPosition('bottom');
    } else {
      setAnchorPosition('hidden');
    }
  }, [layout.anchorPosition, setAnchorPosition]);

  // Reset layout
  const resetLayout = useCallback(() => {
    setSplitMode('none');
    setSplitRatio(LAYOUT_CONFIG.DEFAULT_SPLIT_RATIO);
    setAnchorPosition('hidden');
    setAnchorSize(LAYOUT_CONFIG.DEFAULT_ANCHOR_SIZE);
  }, [setSplitMode, setSplitRatio, setAnchorPosition, setAnchorSize]);

  return {
    // State
    splitMode: layout.splitMode,
    splitRatio: layout.splitRatio,
    anchorPosition: layout.anchorPosition,
    anchorSize: layout.anchorSize,
    isMaximized: layout.isMaximized,
    
    // Actions
    setSplitMode,
    setSplitRatio,
    setAnchorPosition,
    setAnchorSize,
    toggleMaximize,
    
    // Helpers
    enableHorizontalSplit,
    enableVerticalSplit,
    disableSplit,
    toggleSplit,
    showAnchor,
    hideAnchor,
    toggleAnchor,
    resetLayout,
  };
};

export default useLayoutState;
