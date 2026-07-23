/**
 * Layout-related type definitions.
 * ContentCanvas layout system.
 */

import type { EditorGroupState } from './tab';

/**
 * Split modes
 * - none: single column
 * - horizontal: left/right
 * - vertical: top/bottom
 * - grid: top left/right + bottom (T layout)
 */
export type SplitMode = 'none' | 'horizontal' | 'vertical' | 'grid';

export type AnchorPosition = 'bottom' | 'right' | 'hidden';

export type DropPosition = 'left' | 'right' | 'top' | 'bottom' | 'center';

export type EditorGroupId = 'primary' | 'secondary' | 'tertiary';

export interface LayoutState {
  splitMode: SplitMode;
  /** Primary split ratio: left/top in 2-pane; top/bottom or left/right in 3-pane */
  splitRatio: number;
  /** Secondary split ratio: grid-top left/right or grid-bottom left/right */
  splitRatio2: number;
  anchorPosition: AnchorPosition;
  anchorSize: number;
  isMaximized: boolean;
}

export interface CanvasState {
  primaryGroup: EditorGroupState;
  secondaryGroup: EditorGroupState;
  tertiaryGroup: EditorGroupState;
  activeGroupId: EditorGroupId;
  layout: LayoutState;
  isMissionControlOpen: boolean;
}

export interface CanvasPersistState {
  primaryGroup: EditorGroupState;
  secondaryGroup: EditorGroupState;
  tertiaryGroup: EditorGroupState;
  activeGroupId: EditorGroupId;
  layout: LayoutState;
}

/**
 * Layout configuration constants.
 */
export const LAYOUT_CONFIG = {
  /** Min split ratio */
  MIN_SPLIT_RATIO: 0.2,
  /** Max split ratio */
  MAX_SPLIT_RATIO: 0.8,
  /** Default split ratio */
  DEFAULT_SPLIT_RATIO: 0.5,
  /** Min anchor size */
  MIN_ANCHOR_SIZE: 100,
  /** Max anchor size */
  MAX_ANCHOR_SIZE: 500,
  /** Default anchor size */
  DEFAULT_ANCHOR_SIZE: 200,
  /** Resizer width */
  RESIZER_WIDTH: 4,
  /** Snap range */
  SNAP_RANGE: 15,
  /** Transition duration */
  TRANSITION_DURATION: 200,
} as const;

/**
 * Create initial editor group state.
 */
export const createEditorGroupState = (): EditorGroupState => ({
  tabs: [],
  activeTabId: null,
});

export const createLayoutState = (): LayoutState => ({
  splitMode: 'none',
  splitRatio: LAYOUT_CONFIG.DEFAULT_SPLIT_RATIO,
  splitRatio2: LAYOUT_CONFIG.DEFAULT_SPLIT_RATIO,
  anchorPosition: 'hidden',
  anchorSize: LAYOUT_CONFIG.DEFAULT_ANCHOR_SIZE,
  isMaximized: false,
});

export const createCanvasState = (): CanvasState => ({
  primaryGroup: createEditorGroupState(),
  secondaryGroup: createEditorGroupState(),
  tertiaryGroup: createEditorGroupState(),
  activeGroupId: 'primary',
  layout: createLayoutState(),
  isMissionControlOpen: false,
});

/**
 * Clamp split ratio to valid range.
 */
export const clampSplitRatio = (ratio: number): number => {
  return Math.max(
    LAYOUT_CONFIG.MIN_SPLIT_RATIO,
    Math.min(LAYOUT_CONFIG.MAX_SPLIT_RATIO, ratio)
  );
};

/**
 * Clamp anchor size to valid range.
 */
export const clampAnchorSize = (size: number): number => {
  return Math.max(
    LAYOUT_CONFIG.MIN_ANCHOR_SIZE,
    Math.min(LAYOUT_CONFIG.MAX_ANCHOR_SIZE, size)
  );
};
