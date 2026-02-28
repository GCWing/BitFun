/**
 * Utility functions for panel content type configuration and management.
 * Provides helpers for icon resolution, capability checking, and file generation.
 */

import React from 'react';
import { 
  Code, 
  FileText, 
  GitBranch, 
  Eye,
  Edit3,
  Terminal as TerminalIcon,
  BookOpen,
  Settings,
  ClipboardList,
  Image,
  Network
} from 'lucide-react';
import { PanelContentType, PanelContentConfig } from './types';

// Configuration mapping for each panel content type
export const PANEL_CONTENT_CONFIGS: Record<PanelContentType, PanelContentConfig> = {
  'empty': {
    type: 'empty',
    displayName: 'Empty',
    icon: FileText,
    supportsCopy: false,
    supportsDownload: false,
    showHeader: false
  },
  'code-preview': {
    type: 'code-preview',
    displayName: 'Code Preview',
    icon: Code,
    supportsCopy: true,
    supportsDownload: true,
    showHeader: true
  },
  'code-viewer': {
    type: 'code-viewer',
    displayName: 'Code Viewer',
    icon: Code,
    supportsCopy: true,
    supportsDownload: true,
    showHeader: false
  },
  'code-editor': {
    type: 'code-editor',
    displayName: 'Code Editor',
    icon: Code,
    supportsCopy: true,
    supportsDownload: true,
    showHeader: false
  },
  'markdown-viewer': {
    type: 'markdown-viewer',
    displayName: 'Markdown Viewer',
    icon: FileText,
    supportsCopy: true,
    supportsDownload: true,
    showHeader: true
  },
  'markdown-editor': {
    type: 'markdown-editor',
    displayName: 'Markdown Editor',
    icon: FileText,
    supportsCopy: true,
    supportsDownload: true,
    showHeader: false
  },
  'mermaid-editor': {
    type: 'mermaid-editor',
    displayName: 'Mermaid Editor',
    icon: Edit3,
    supportsCopy: true,
    supportsDownload: true,
    showHeader: false
  },
  'text-viewer': {
    type: 'text-viewer',
    displayName: 'Text Viewer',
    icon: Eye,
    supportsCopy: true,
    supportsDownload: true,
    showHeader: true
  },
  'file-viewer': {
    type: 'file-viewer',
    displayName: 'File Viewer',
    icon: Code,
    supportsCopy: true,
    supportsDownload: true,
    showHeader: false
  },
  'image-viewer': {
    type: 'image-viewer',
    displayName: 'Image Viewer',
    icon: Image,
    supportsCopy: false,
    supportsDownload: false,
    showHeader: false
  },
  'diff-code-editor': {
    type: 'diff-code-editor',
    displayName: 'Diff Editor',
    icon: Code,
    supportsCopy: true,
    supportsDownload: true,
    showHeader: false
  },
  'git-diff': {
    type: 'git-diff',
    displayName: 'Git Diff',
    icon: GitBranch,
    supportsCopy: true,
    supportsDownload: true,
    showHeader: false
  },
  'git-settings': {
    type: 'git-settings',
    displayName: 'Git Settings',
    icon: GitBranch,
    supportsCopy: false,
    supportsDownload: false,
    showHeader: false
  },
  'ai-session': {
    type: 'ai-session',
    displayName: 'AI Session',
    icon: BookOpen,
    supportsCopy: false,
    supportsDownload: false,
    showHeader: false
  },
  'planner': {
    type: 'planner',
    displayName: 'Planner',
    icon: ClipboardList,
    supportsCopy: false,
    supportsDownload: false,
    showHeader: false
  },
  'ui-editor': {
    type: 'ui-editor',
    displayName: 'UI Editor',
    icon: Edit3,
    supportsCopy: false,
    supportsDownload: false,
    showHeader: false
  },
  'ui-relation-graph': {
    type: 'ui-relation-graph',
    displayName: 'UI Relation Graph',
    icon: Network,
    supportsCopy: false,
    supportsDownload: false,
    showHeader: false
  },
  'design-tokens': {
    type: 'design-tokens',
    displayName: 'Design Tokens',
    icon: Settings,
    supportsCopy: false,
    supportsDownload: false,
    showHeader: false
  },
  'task-detail': {
    type: 'task-detail',
    displayName: 'Task Detail',
    icon: ClipboardList,
    supportsCopy: false,
    supportsDownload: false,
    showHeader: false
  },
  'plan-viewer': {
    type: 'plan-viewer',
    displayName: 'Plan Viewer',
    icon: ClipboardList,
    supportsCopy: false,
    supportsDownload: false,
    showHeader: false
  }
};

/**
 * Gets the icon component for a given panel content type.
 * 
 * @param type - The panel content type
 * @returns React element with the appropriate icon (size: 16px)
 */
export const getContentIcon = (type: PanelContentType): React.ReactElement => {
  if (!type || !PANEL_CONTENT_CONFIGS[type]) {
    const DefaultIcon = PANEL_CONTENT_CONFIGS.empty.icon;
    return React.createElement(DefaultIcon, { size: 16 });
  }
  const config = PANEL_CONTENT_CONFIGS[type];
  const IconComponent = config.icon;
  return React.createElement(IconComponent, { size: 16 });
};

/**
 * Gets the display name for a given panel content type.
 * 
 * @param type - The panel content type
 * @returns Human-readable display name
 */
export const getContentTypeName = (type: PanelContentType): string => {
  if (!type || !PANEL_CONTENT_CONFIGS[type]) {
    return PANEL_CONTENT_CONFIGS.empty.displayName;
  }
  return PANEL_CONTENT_CONFIGS[type].displayName;
};

/**
 * Checks whether a content type supports copy functionality.
 * 
 * @param type - The panel content type
 * @returns True if the content type supports copying
 */
export const supportsContentCopy = (type: PanelContentType): boolean => {
  if (!type || !PANEL_CONTENT_CONFIGS[type]) {
    return false;
  }
  return PANEL_CONTENT_CONFIGS[type].supportsCopy;
};

/**
 * Checks whether a content type supports download functionality.
 * 
 * @param type - The panel content type
 * @returns True if the content type supports downloading
 */
export const supportsContentDownload = (type: PanelContentType): boolean => {
  if (!type || !PANEL_CONTENT_CONFIGS[type]) {
    return false;
  }
  return PANEL_CONTENT_CONFIGS[type].supportsDownload;
};

/**
 * Checks whether a content type should display a panel header.
 * 
 * @param type - The panel content type
 * @returns True if the content type should show the default header
 */
export const shouldShowHeader = (type: PanelContentType): boolean => {
  if (!type || !PANEL_CONTENT_CONFIGS[type]) {
    return false;
  }
  return PANEL_CONTENT_CONFIGS[type].showHeader;
};

/**
 * Generates a unique identifier for a panel tab.
 * 
 * @returns Unique tab ID in the format "tab_{timestamp}_{random}"
 */
export const generateTabId = (): string => {
  return `tab_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
};

/**
 * Generates an appropriate filename based on content type and title.
 * 
 * @param type - The panel content type
 * @param title - The content title (used as base filename)
 * @returns Filename with appropriate extension for the content type
 */
export const generateFileName = (type: PanelContentType, title: string): string => {
  const baseName = title || 'content';
  
  switch (type) {
    case 'markdown-viewer':
    case 'markdown-editor':
      return `${baseName}.md`;
    case 'code-preview':
      return `${baseName}.txt`;
    case 'mermaid-editor':
      return `${baseName}.mmd`;
    default:
      return `${baseName}.txt`;
  }
};

