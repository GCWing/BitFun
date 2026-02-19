/**
 * Type definitions for panel components.
 * Defines content types, interfaces, and configuration for the panel system.
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
  | 'config-center'
  | 'workflow-editor'
  | 'planner'
  | 'task-detail'
  | 'plan-viewer'
  | 'terminal';

export interface PanelContent {
  type: PanelContentType;
  title: string;
  data?: any;
  metadata?: Record<string, any>;
}

export interface TabData {
  id: string;
  title: string;
  content: PanelContent;
  isDirty?: boolean; // Has unsaved changes
}

export interface FlexiblePanelProps {
  content: PanelContent | null;
  onContentChange?: (content: PanelContent | null) => void;
  className?: string;
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
  workspacePath?: string;
  onBeforeClose?: (content: PanelContent | null) => Promise<boolean>;
}

export interface TabbedFlexiblePanelProps {
  className?: string;
  onTabsChange?: (tabs: TabData[]) => void;
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
  workspacePath?: string;
  onBeforeClose?: (content: PanelContent | null) => Promise<boolean>;
}

export interface TabbedFlexiblePanelRef {
  addTab: (content: PanelContent) => void;
  switchToTab: (tabId: string) => void;
  findTabByMetadata: (metadata: Record<string, any>) => TabData | null;
  updateTabContent: (tabId: string, content: PanelContent) => void;
  closeAllTabs: () => void;
}

export interface PanelContentConfig {
  type: PanelContentType;
  displayName: string;
  icon: React.ComponentType<{ size?: number }>;
  supportsCopy: boolean;
  supportsDownload: boolean;
  showHeader: boolean;
}

