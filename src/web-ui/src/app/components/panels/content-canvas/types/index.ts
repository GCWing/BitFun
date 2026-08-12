/**
 * Unified exports for ContentCanvas types.
 */

// Tab-related types
export type {
  TabState,
  CanvasTab,
  EditorGroupState,
  TabDragPayload,
  TabActions,
  TabVisualProps,
  ClosedTabRecord,
} from './tab';

export { generateTabId, createTab } from './tab';

// Layout-related types
export type {
  SplitMode,
  AnchorPosition,
  DropPosition,
  EditorGroupId,
  LayoutState,
  CanvasState,
  CanvasPersistState,
} from './layout';

export {
  LAYOUT_CONFIG,
  EDITOR_GROUP_IDS,
  EDITOR_GROUP_COL,
  EDITOR_GROUP_ROW,
  GRID_MAX_DIM,
  GRID9_RATIO_CONFIG,
  createEditorGroupState,
  createLayoutState,
  createCanvasState,
  clampSplitRatio,
  clampGrid9Ratio,
  clampAnchorSize,
} from './layout';

// Content-related types
export type {
  PanelContentType,
  PanelContent,
  CreateTabOptions,
  CreateTabEventDetail,
} from './content';

export {
  FILE_VIEWER_TYPES,
  isFileViewerType,
  TAB_EVENTS,
} from './content';
