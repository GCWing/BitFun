/**
 * Content-related type definitions.
 * Reuses existing FlexiblePanel content types.
 */

/**
 * Panel content types.
 */
export type PanelContentType =
  | 'empty'
  | 'code-preview'
  | 'code-viewer'
  | 'code-editor'
  | 'markdown-viewer'
  | 'markdown-editor'
  | 'mermaid-editor'
  | 'text-viewer'
  | 'file-viewer'
  | 'image-viewer'
  | 'diff-code-editor'
  | 'git-diff'
  | 'git-settings'
  | 'git-graph'
  | 'git-branch-history'
  | 'ai-session'
  | 'planner'
  | 'task-detail'
  | 'plan-viewer'
  | 'terminal';

/**
 * Panel content interface.
 */
export interface PanelContent {
  /** Content type */
  type: PanelContentType;
  /** Display title */
  title: string;
  /** Content data */
  data?: any;
  /** Metadata (dedupe, navigation, etc.) */
  metadata?: Record<string, any>;
}

/**
 * File viewer types (code, markdown, images, etc.).
 */
export const FILE_VIEWER_TYPES: PanelContentType[] = [
  'code-preview',
  'code-viewer',
  'code-editor',
  'markdown-viewer',
  'markdown-editor',
  'mermaid-editor',
  'text-viewer',
  'file-viewer',
  'image-viewer',
  'diff-code-editor',
  'plan-viewer',
];

/**
 * Check whether a content type is a file viewer.
 */
export const isFileViewerType = (type: PanelContentType): boolean => {
  return FILE_VIEWER_TYPES.includes(type);
};

/**
 * Options for creating a tab.
 */
export interface CreateTabOptions {
  /** Content type */
  type: PanelContentType;
  /** Title */
  title: string;
  /** Data */
  data?: any;
  /** Metadata */
  metadata?: Record<string, any>;
  /** Whether to check duplicates */
  checkDuplicate?: boolean;
  /** Duplicate check key */
  duplicateCheckKey?: string;
  /** Whether to replace existing tab */
  replaceExisting?: boolean;
  /** Target editor group */
  targetGroup?: 'primary' | 'secondary';
  /** Enable split view (auto-switch to horizontal split) */
  enableSplitView?: boolean;
}

/**
 * Create-tab event detail.
 */
export interface CreateTabEventDetail extends CreateTabOptions {
  /** App mode / target canvas */
  mode?: 'agent' | 'project' | 'git';
}

/**
 * Tab event names.
 */
export const TAB_EVENTS = {
  /** Create tab in agent mode */
  AGENT_CREATE_TAB: 'agent-create-tab',
  /** Create tab in project mode */
  PROJECT_CREATE_TAB: 'project-create-tab',
  /** Create tab in Git scene canvas */
  GIT_CREATE_TAB: 'git-create-tab',
  /** Expand right panel */
  EXPAND_RIGHT_PANEL: 'expand-right-panel',
} as const;
